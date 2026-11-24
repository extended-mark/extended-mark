//! What the IR tells of a page beside what it is: what each place reads, what each handler writes, what
//! is worked out once for all, and which numbers are proved whole (page 6, The IR).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use xtmd_check::{compile_core, compile_on, Checker, Source};
use xtmd_ir::ir::Module;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn base() -> Checker {
    let mut files: Vec<PathBuf> = std::fs::read_dir(repo().join("core")).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    let core: Vec<Source> = files.iter().map(|p| Source { path: p.to_string_lossy().into_owned(), text: std::fs::read_to_string(p).unwrap() }).collect();
    compile_core(&core)
}

fn module(text: &str) -> Module {
    let c = compile_on(&base(), &[], &[Source { path: "page.xtmd".into(), text: text.into() }], "doc");
    let errors: Vec<String> = c.errors().map(|d| d.diagnostic.message.clone()).collect();
    assert!(errors.is_empty(), "{:?}", errors);
    let mut m = c.module.unwrap();
    xtmd_lower::lower(&mut m);
    m
}

fn page(text: &str) -> Value {
    serde_json::to_value(module(text)).unwrap()
}

/// Every value under this key, in the order written.
fn all<'a>(v: &'a Value, key: &str, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if k == key {
                    out.push(x);
                }
                all(x, key, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| all(x, key, out)),
        _ => {}
    }
}

fn template<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["templates"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap()
}

/// The one node of a kind in a template whose name it has, or any.
fn node<'a>(t: &'a Value, kind: &str, name: Option<&str>) -> &'a Value {
    let mut out = Vec::new();
    all(&t["view"], kind, &mut out);
    out.into_iter().find(|n| name.map_or(true, |x| n["name"] == x)).unwrap()
}

fn handler<'a>(v: &'a Value, t: &'a Value, widget: &str) -> &'a Value {
    let id = node(t, "extern", Some(widget))["events"].as_object().unwrap().values().next().unwrap();
    v["handlers"].as_array().unwrap().iter().find(|h| h["id"] == *id).unwrap()
}

const PAGE: &str = r#"
record row {
    name: text;
    mut done: bool = false;
}

$state {
    n: number = 1;
    rows: list<row> = [row { name = "a" }];
    open: bool = false;
}

let start: number = 2;

fn twice(): number {
    return $state.n * 2;
}

fn add(by: number) {
    $state.n = $state.n + by;
}

widget shown(value: number, mut on: bool) {
    mut seen: number = 0;

    view {
        @label(value = "\{value} \{seen} \{start}");
        @press(label = "Show", click = fn() {
            seen = seen + value;
            on = !on;
        });
    }
    markdown {
        \{value}
    }
}
@@@
@label(value = "fixed \{start + 1}");
@label(value = "\{twice()}");
@shown(value = twice(), on = $state.open);
@press(label = "Add", click = fn() { add(start); });
@if (twice() > 3) {
    @for (r in $state.rows) {
        @label(value = r.name);
        @label(value = "\{page.width == width.NARROW ? "narrow" : r.name}");
    }
}
"#;

#[test]
fn each_input_says_what_it_reads() {
    let v = page(PAGE);
    let doc = template(&v, "doc");
    let mut labels = Vec::new();
    all(&doc["view"], "extern", &mut labels);
    let labels: Vec<&Value> = labels.into_iter().filter(|x| x["name"] == "core/label").collect();
    // the same on every page: worked out, and nothing is read
    assert_eq!(labels[0]["props"]["value"], json!({"text": "fixed 3"}));
    assert!(labels[0].get("reads").is_none());
    // through the fn it calls
    assert_eq!(labels[1]["reads"], json!({"value": [{"$state": "n"}]}));
    // a loop's variable, and the page's own width
    assert_eq!(labels[2]["reads"], json!({"value": [{"local": "r"}]}));
    assert_eq!(labels[3]["reads"], json!({"value": [{"local": "r"}, {"extern": "core/page.width"}]}));
    assert_eq!(node(doc, "use", None)["reads"], json!({"value": [{"$state": "n"}], "on": [{"$state": "open"}]}));
    assert_eq!(node(doc, "for", None)["reads"], json!([{"$state": "rows"}]));
    assert_eq!(node(doc, "if", None)["branches"][0]["reads"], json!([{"$state": "n"}]));
    // a widget's own: its input and its variable, and a constant is none
    let shown = template(&v, "doc/shown");
    assert_eq!(node(shown, "extern", Some("core/label"))["reads"], json!({"value": [{"input": "value"}, {"cell": "seen"}]}));
    assert!(node(shown, "extern", Some("core/press")).get("reads").is_none());
}

#[test]
fn each_handler_and_fn_says_what_it_reads_and_writes() {
    let v = page(PAGE);
    let h = handler(&v, template(&v, "doc"), "core/press");
    assert_eq!((&h["reads"], &h["writes"]), (&json!([{"$state": "n"}]), &json!([{"$state": "n"}])));
    let h = handler(&v, template(&v, "doc/shown"), "core/press");
    assert_eq!(h["reads"], json!([{"input": "on"}, {"input": "value"}, {"cell": "seen"}]));
    assert_eq!(h["writes"], json!([{"input": "on"}, {"cell": "seen"}]));
    let add = v["declarations"].as_array().unwrap().iter().find(|d| d["declare"]["fn"]["name"] == "doc/add").unwrap();
    assert_eq!((&add["declare"]["fn"]["reads"], &add["declare"]["fn"]["writes"]), (&json!([{"$state": "n"}]), &json!([{"$state": "n"}])));
}

#[test]
fn a_write_through_an_item_is_a_write_of_the_list() {
    let v = page(
        r#"
record row {
    mut done: bool = false;
}

$state {
    rows: list<row> = [row { }];
}

fn finish(mut r: row) {
    r.done = true;
}

widget all() {
    view {
        @press(label = "All", click = fn() {
            mut for (r in $state.rows) {
                if (mut x: row = r) {
                    x.done = true;
                }
            }
        });
        for (r in $state.rows) {
            @label(value = "\{r.done}");
        }
    }
    markdown {
    }
}
@@@
@all;
"#,
    );
    let t = template(&v, "doc/all");
    let h = handler(&v, t, "core/press");
    assert_eq!(h["writes"], json!([{"$state": "rows"}]));
}

#[test]
fn a_fn_kept_in_a_variable_reads_anything() {
    let kept = r#"
$state {
    n: number = 1;
}

widget w() {
    mut f: fn(): number = fn() { return 1; };

    view {
        @label(value = "\{f()}");
        @press(label = "Take", click = fn() { f = fn() { return $state.n; }; });
    }
    markdown {
    }
}
@@@
@w;
"#;
    let v = page(kept);
    let mut reads = Vec::new();
    all(template(&v, "doc/w"), "reads", &mut reads);
    assert!(reads.iter().any(|r| r["value"].as_array().is_some_and(|a| a.contains(&json!({"any": {}})))), "{reads:?}");
    // one given as an input was read where it was written
    let v = page(
        r#"
$state {
    n: number = 1;
}

widget w(f: fn(): number) {
    view {
        @label(value = "\{f()}");
    }
    markdown {
    }
}
@@@
@w(f = fn() { return $state.n; });
"#,
    );
    assert_eq!(node(template(&v, "doc"), "use", None)["reads"], json!({"f": [{"$state": "n"}]}));
    assert_eq!(node(template(&v, "doc/w"), "extern", None)["reads"], json!({"value": [{"input": "f"}]}));
}

#[test]
fn rules_say_what_they_read() {
    let v = page(
        r#"
$state {
    low: number = 1;
    high: number = 5;
    other: number = 0;

    require "low is under high" {
        return $state.low <= $state.high;
    }
}
@@@
\{$state.low} \{$state.high} \{$state.other}
"#,
    );
    let state = v["declarations"].as_array().unwrap().iter().find(|d| d["declare"].get("$state").is_some()).unwrap();
    assert_eq!(state["declare"]["$state"]["requires"][0]["reads"], json!([{"$state": "high"}, {"$state": "low"}]));
}

/// Each operator of a page, with whether it is proved whole.
fn ops(text: &str) -> Vec<(String, bool)> {
    let v = page(text);
    let mut out = Vec::new();
    all(&v, "op", &mut out);
    out.into_iter().map(|o| (o["name"].as_str().unwrap().to_string(), o["whole"] == true)).collect()
}

fn one(of: &str, code: &str) -> bool {
    let page = format!("$state {{\n    n: {of} = 5;\n}}\n@@@\n@label(value = \"\\{{{code}}}\");\n");
    ops(&page)[0].1
}

#[test]
fn what_a_contract_holds_whole_is_proved() {
    assert!(one("int 0..100", "$state.n + 1"));
    assert!(one("int 0..100", "$state.n * $state.n - 7"));
    assert!(one("int 0..100", "$state.n % 3"));
    assert!(one("int 0..100", "$state.n > 3"));
    assert!(one("number 0..100 step 5", "$state.n + 5"));
    // not whole, not bounded, too large, or divided
    assert!(!one("number 0..100", "$state.n + 1"));
    assert!(!one("int", "$state.n + 1"));
    assert!(!one("int 0..9007199254740991", "$state.n + 1"));
    assert!(!one("int 0..100", "$state.n / 2"));
    assert!(!one("int 0..100", "$state.n + 0.5"));
    assert!(!one("int 0..100", "3 % $state.n"));
    assert!(!one("number 0..100 step 0.5", "$state.n + 1"));
}

#[test]
fn what_is_the_same_on_every_page_is_worked_out() {
    let v = page(
        r#"
let width: number = 12;
let names: list<text> = ["a", "b"];

fn area(w: number): number {
    return w * width;
}
@@@
@label(value = "\{area(3)} of \{names.len()}, \{1 / 4}");
"#,
    );
    assert_eq!(node(template(&v, "doc"), "extern", None)["props"]["value"], json!({"text": "36 of 2, 0.25"}));
    // and what was worked out is gone, with what it used
    let mut names = Vec::new();
    all(&v["declarations"], "fn", &mut names);
    assert!(names.is_empty(), "{names:?}");
}

#[test]
fn what_an_engine_or_a_host_may_supply_is_left_to_it() {
    // a fn with a default: called plainly, through a fn, and in an expression
    let dir = repo().join("checks/pages/pick/pick.xtmdpack");
    let pack = vec![Source { path: "pick.xtmdpack".into(), text: std::fs::read_to_string(dir).unwrap() }];
    for code in ["dial(1)", "turned()", "dial(1) + 1"] {
        let text = format!("import pick;\n\nfn turned(): int {{\n    return dial(1);\n}}\n@@@\n@label(value = \"\\{{{code}}}\");\n");
        let c = compile_on(&base(), &[pack.clone()], &[Source { path: "page.xtmd".into(), text }], "doc");
        let errors: Vec<String> = c.errors().map(|d| d.diagnostic.message.clone()).collect();
        assert!(errors.is_empty(), "{:?}", errors);
        let mut m = c.module.unwrap();
        xtmd_lower::lower(&mut m);
        let v = serde_json::to_value(&m).unwrap();
        let value = &node(template(&v, "doc"), "extern", None)["props"]["value"];
        assert!(value.get("text").is_none(), "{code}: {value}");
        assert!(serde_json::to_string(&v["needs"]).unwrap().contains("pick/dial"), "{code}: {}", v["needs"]);
    }
}

#[test]
fn what_would_fail_is_left_to_fail_as_the_page_runs() {
    let v = page("widget w(n: number) {\n    view {\n        @label(value = \"\\{n}\");\n    }\n    markdown {\n    }\n}\n@@@\n@w(n = 1 / 0);\n");
    assert!(node(template(&v, "doc"), "use", None)["args"]["n"].get("op").is_some());
}

#[test]
fn telling_again_says_the_same() {
    let mut files = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "xtmd") {
                out.push(p);
            }
        }
    }
    walk(&repo().join("checks/cases"), &mut files);
    walk(&repo().join("engines/published/runtime/test/cases"), &mut files);
    let base = base();
    let mut n = 0;
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let Some(mut m) = compile_on(&base, &[], &[Source { path: "page.xtmd".into(), text }], "doc").module else { continue };
        xtmd_lower::lower(&mut m);
        let reads = |m: &Module| {
            let v = serde_json::to_value(m).unwrap();
            let mut out = Vec::new();
            ["reads", "writes"].iter().for_each(|k| all(&v, k, &mut out));
            out.into_iter().cloned().collect::<Vec<_>>()
        };
        let first = reads(&m);
        xtmd_lower::tell(&mut m);
        assert_eq!(first, reads(&m), "{}", f.display());
        n += 1;
    }
    assert!(n > 50, "{n}");
}
