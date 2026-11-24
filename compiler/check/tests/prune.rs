//! A page's IR holds only what the page can run, and everything it names.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;
use xtmd_check::{compile_core, compile_on, Checker, Source};
use xtmd_ir::ir::{Decl, Module};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_all(dir: &Path, ext: &str, out: &mut Vec<Source>) {
    if dir.is_file() {
        return out.push(Source { path: dir.to_string_lossy().into_owned(), text: std::fs::read_to_string(dir).unwrap() });
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    for f in files {
        if f.is_dir() {
            read_all(&f, ext, out);
        } else if f.to_str().unwrap().ends_with(ext) {
            out.push(Source { path: f.to_string_lossy().into_owned(), text: std::fs::read_to_string(&f).unwrap() });
        }
    }
}

fn base() -> Checker {
    let mut core = Vec::new();
    read_all(&repo().join("core"), ".xtmdpack", &mut core);
    compile_core(&core)
}

fn page_on(base: &Checker, packs: &[Vec<Source>], text: &str) -> Option<Module> {
    let mut m = compile_on(base, packs, &[Source { path: "page.xtmd".into(), text: text.into() }], "doc").module?;
    xtmd_lower::lower(&mut m);
    Some(m)
}

fn page(text: &str) -> Module {
    let base = base();
    let c = compile_on(&base, &[], &[Source { path: "page.xtmd".into(), text: text.into() }], "doc");
    let errors: Vec<String> = c.errors().map(|d| d.diagnostic.message.clone()).collect();
    assert!(errors.is_empty(), "{:?}", errors);
    page_on(&base, &[], text).unwrap()
}

/// Each declaration's name.
fn names(m: &Module) -> BTreeSet<String> {
    let v = serde_json::to_value(m).unwrap();
    v["declarations"].as_array().unwrap().iter().filter_map(declared).collect()
}

fn declared(d: &Value) -> Option<String> {
    let mut v = &d["declare"];
    for _ in 0..3 {
        if let Some(n) = v["name"].as_str() {
            return Some(n.to_string());
        }
        v = v.as_object()?.values().next()?;
    }
    None
}

fn record<'a>(m: &'a Module, name: &str) -> &'a xtmd_ir::ir::Record {
    m.declarations.iter().find_map(|d| if let Decl::Record(r) = &d.declare { Some(r).filter(|r| r.name == name) } else { None }).unwrap_or_else(|| panic!("no record {name}"))
}

fn cases(m: &Module, name: &str) -> Vec<String> {
    m.declarations
        .iter()
        .find_map(|d| match &d.declare {
            Decl::Enum(e) if e.name == name => Some(e.cases.iter().map(|c| c.name.clone()).collect()),
            Decl::Token(t) if t.name == name => Some(t.cases.iter().map(|c| c.name.clone()).collect()),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn a_page_keeps_what_it_calls_and_drops_the_rest_its_own_too() {
    let m = page("record used {\n    a: int;\n}\n\nrecord unused {\n    a: int;\n}\n\nfn called(x: used): int {\n    return x.a;\n}\n\nfn never(x: int): int {\n    return x;\n}\n\nwidget shown() {\n    fn own(): int {\n        return called(used { a = 1 });\n    }\n    fn idle(): int {\n        return never(2);\n    }\n    view {\n        @label(value = \"\\{own()}\");\n    }\n}\n\nwidget hidden() {\n    view {\n        @icon(name = glyph.STAR);\n    }\n}\n@@@\n@shown;\n");
    let n = names(&m);
    assert!(n.contains("doc/used") && n.contains("doc/called") && n.contains("core/label"), "{n:?}");
    assert!(!n.contains("doc/unused") && !n.contains("doc/never") && !n.contains("core/icon") && !n.contains("core/glyph"), "{n:?}");
    let shown = m.templates.iter().find(|t| t.name == "doc/shown").unwrap();
    assert_eq!(shown.fns.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["doc/shown.own"]);
    assert!(m.templates.iter().all(|t| t.name != "doc/hidden"));
    assert!(!m.needs.core.contains(&"core/icon".to_string()) && m.needs.core.contains(&"core/label".to_string()));
}

#[test]
fn a_cases_arm_is_kept_when_the_page_can_make_the_case() {
    let doc = |state: &str, at: &str| format!("enum stage {{\n    A,\n    B,\n    C(n: int),\n}}\n\n{state}\nwidget w(s: stage) {{\n    view {{\n        match (s) {{\n            case stage.A {{\n                @label(value = \"a\");\n            }}\n            case stage.B {{\n                @icon(name = glyph.STAR);\n            }}\n            case stage.C(n) {{\n                @label(value = \"\\{{n}}\");\n            }}\n        }}\n    }}\n}}\n@@@\n@w(s = {at});\n");
    let m = page(&doc("", "stage.A"));
    assert_eq!(cases(&m, "doc/stage"), ["A"]);
    assert!(!names(&m).contains("core/icon"));
    // a host can write it: whole
    let m = page(&doc("$state {\n    at: stage = stage.A;\n}\n", "$state.at"));
    assert_eq!(cases(&m, "doc/stage"), ["A", "B", "C"]);
    assert!(names(&m).contains("core/icon"));
}

#[test]
fn a_records_rules_are_kept_when_the_page_makes_one_as_it_runs() {
    let doc = |view: &str| format!("record even {{\n    n: int = 2;\n    require \"n is even\" {{\n        return is_even(n);\n    }}\n}}\n\nfn is_even(n: int): bool {{\n    return n % 2 == 0;\n}}\n\nlet four: even = even {{ n = 4 }};\nlet two: even = even {{ }};\n\nwidget w() {{\n    mut k: int = 2;\n    view {{\n        @press(label = \"\\{{k}}\", click = fn() {{ k = k + 2; }});\n        {view}\n    }}\n}}\n@@@\n@w;\n");
    let m = page(&doc("@label(value = \"\\{four.n}\");"));
    assert!(record(&m, "doc/even").requires.is_empty() && record(&m, "doc/even").fields[0].default.is_none());
    assert!(!names(&m).contains("doc/is_even"));
    // made when the page was compiled, with a field left out: its value is in the IR, or its default is
    let m = page(&doc("@label(value = \"\\{two.n}\");"));
    assert!(record(&m, "doc/even").requires.is_empty());
    assert!(record(&m, "doc/even").fields[0].default.is_some() || serde_json::to_string(&m).unwrap().contains("\"fields\":{\"n\":{\"num\":\"2\"}}"));
    let m = page(&doc("@label(value = \"\\{(even { n = k }).n}\");"));
    assert_eq!(record(&m, "doc/even").requires.len(), 1);
    assert!(names(&m).contains("doc/is_even"));
}

#[test]
fn tokens_are_the_ones_the_page_or_what_it_calls_names() {
    let m = page("@@@\n@label(value = \"x\", tone = color.OK);\n");
    let c = cases(&m, "core/color");
    // the page's own, the label's, and the one named
    assert!(c.contains(&"OK".to_string()) && c.contains(&"SURFACE".to_string()) && c.contains(&"INK".to_string()), "{c:?}");
    assert!(!c.contains(&"WARN".to_string()), "{c:?}");
    assert!(!names(&m).contains("core/stroke") && !names(&m).contains("core/space"));
    // a colour's rules run where the page makes one, and nowhere else
    assert!(record(&m, "core/color_value").requires.is_empty() && !names(&m).contains("core/hex"));
    // a host that sends one can send any
    let m = page("$state {\n    tone: color = color.OK;\n}\n@@@\n@label(value = \"x\", tone = $state.tone);\n");
    assert!(cases(&m, "core/color").contains(&"WARN".to_string()) || cases(&m, "core/color").len() > c.len());
}

#[test]
fn a_value_shown_or_compared_brings_its_types_fn_and_no_other() {
    let doc = |hole: &str| format!("record pair {{\n    a: int;\n}}\n\nfn pair.show(self): text {{\n    return \"pair\";\n}}\n\nfn pair.same(self, other: pair): bool {{\n    return self.a == other.a;\n}}\n\n$state {{\n    p: pair = pair {{ a = 1 }};\n    n: int = 1;\n}}\n@@@\n\\{{{hole}}}\n");
    let n = names(&page(&doc("$state.n")));
    assert!(!n.contains("doc/pair.show") && !n.contains("doc/pair.same"), "{n:?}");
    let n = names(&page(&doc("$state.p")));
    assert!(n.contains("doc/pair.show") && !n.contains("doc/pair.same"), "{n:?}");
    let m = page(&doc("$state.p == $state.p"));
    assert!(names(&m).contains("doc/pair.same") && record(&m, "doc/pair").same.is_some());
    // with no `==` in it, the record compares field by field
    assert!(record(&page(&doc("$state.n")), "doc/pair").same.is_none());
}

#[test]
fn needs_lists_every_value_and_fn_taken_from_the_host() {
    let m = page("extern scores {\n    best: int = 0;\n    worst: int = 0;\n    fn saved(n: int);\n    fn lost(n: int);\n}\n\nwidget w() {\n    view {\n        @press(label = \"\\{scores.best} \\{page.width == width.NARROW}\", click = fn() { scores.saved(1); });\n    }\n}\n@@@\n@w;\n");
    assert_eq!(m.needs.host, ["core/page.width", "doc/scores.best", "doc/scores.saved"]);
    let channel = m.declarations.iter().find_map(|d| if let Decl::Extern(xtmd_ir::ir::Extern::Channel(c)) = &d.declare { Some(c).filter(|c| c.name == "doc/scores") } else { None }).unwrap();
    assert_eq!((channel.values.len(), channel.fns.len()), (1, 1));
    assert!(page("@@@\nHi.\n").needs.host.is_empty());
}

#[test]
fn markdown_that_is_the_view_is_written_once() {
    let m = page("widget a() {\n    view {\n        @label(value = \"x\");\n    }\n}\n\nwidget b() {\n    view {\n        @label(value = \"x\");\n    }\n    markdown {\n        y\n    }\n}\n\nwidget c() {\n    view {\n        @label(value = \"x\");\n    }\n    markdown { }\n}\n@@@\n@a;\n\n@b;\n\n@c;\n");
    let t = |n: &str| m.templates.iter().find(|t| t.name == n).unwrap();
    assert!(t("doc").markdown.is_none() && t("doc/a").markdown.is_none());
    assert_eq!(t("doc/b").markdown.as_ref().unwrap().len(), 1);
    assert!(t("doc/c").markdown.as_ref().unwrap().is_empty());
}

/// The language's own types, which nothing declares.
const BUILT_IN: [&str; 14] = ["core/number", "core/text", "core/bool", "core/duration", "core/list", "core/opt", "core/range", "core/view", "core/rich_text", "core/widget", "core/any", "core/slot", "core/int", "core/fn"];

/// Every full name in a value: `core/press`, `doc/task.show`.
fn full_names(v: &Value, key: &str, out: &mut BTreeSet<String>) {
    match v {
        // what a page says, and not what it names
        Value::String(_) if matches!(key, "text" | "message" | "front_matter" | "site" | "bounds") => {}
        Value::String(s) => {
            let mut parts = s.split('/');
            let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next()) else { return };
            let word = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.');
            if word(a) && word(b) {
                out.insert(s.clone());
            }
        }
        Value::Array(xs) => xs.iter().for_each(|x| full_names(x, key, out)),
        Value::Object(o) => o.iter().for_each(|(k, x)| full_names(x, k, out)),
        _ => {}
    }
}

#[test]
fn every_name_in_every_cases_ir_is_in_it() {
    let base = base();
    let mut files = Vec::new();
    read_all(&repo().join("checks/cases"), ".json", &mut files);
    read_all(&repo().join("engines/published/runtime/test/cases"), ".json", &mut files);
    let mut checked = 0;
    let mut wrong = Vec::new();
    for f in files {
        let case: Value = serde_json::from_str(&f.text).unwrap();
        let dir = Path::new(&f.path).parent().unwrap();
        let src = dir.join(case["source"].as_str().unwrap());
        if !src.to_str().unwrap().ends_with(".xtmd") {
            continue;
        }
        let mut packs = Vec::new();
        for p in case["packs"].as_array().into_iter().flatten() {
            read_all(&dir.join(p.as_str().unwrap()), ".xtmdpack", &mut packs);
        }
        let Some(m) = page_on(&base, &[packs], &std::fs::read_to_string(&src).unwrap()) else { continue };
        checked += 1;
        let v = serde_json::to_value(&m).unwrap();
        let mut known: BTreeSet<String> = names(&m);
        known.extend(m.templates.iter().map(|t| t.name.clone()));
        known.extend(m.templates.iter().flat_map(|t| t.fns.iter().map(|f| f.name.clone())));
        known.extend(BUILT_IN.iter().map(|s| s.to_string()));
        for d in &m.declarations {
            match &d.declare {
                Decl::Token(t) => known.extend(t.cases.iter().map(|c| format!("{}.{}", t.name, c.name))),
                Decl::Interface(i) => known.extend(i.fns.iter().map(|f| format!("{}.{}", i.name, f))),
                Decl::Extern(xtmd_ir::ir::Extern::Channel(c)) => known.extend(c.values.iter().map(|x| &x.name).chain(c.fns.iter().map(|x| &x.name)).map(|n| format!("{}.{}", c.name, n))),
                _ => {}
            }
        }
        let mut used = BTreeSet::new();
        full_names(&v, "", &mut used);
        for u in used.difference(&known) {
            wrong.push(format!("{}: {}", case["id"], u));
        }
        // and nothing it holds goes unnamed
        for e in m.declarations.iter().filter_map(|d| if let Decl::Enum(e) = &d.declare { Some(e) } else { None }) {
            if e.name.starts_with("core/") && e.cases.is_empty() && !e.name.is_empty() {
                let text = v.to_string();
                assert!(text.matches(&format!("\"{}\"", e.name)).count() > 1, "{}: {} is in it for nothing", case["id"], e.name);
            }
        }
    }
    assert!(checked > 50, "{checked}");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
