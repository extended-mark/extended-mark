//! What comes to a page from outside is one of its type, or is refused and nothing changes: a host's writes
//! and calls, its extern values, what a widget writes and the arguments of its events (page 6). The changes
//! are checks/pages/outside.json's, which the TypeScript runtime is given too.

mod common;

use std::collections::BTreeMap;
use std::rc::Rc;

use common::*;
use serde_json::{json, Value as Json};
use xtmd_runtime::ir::{self, Expr};
use xtmd_runtime::{Broken, Page, PageOptions};

fn module() -> ir::Module {
    let pages = repo().join("checks/pages");
    build_with(&std::fs::read_to_string(pages.join("outside.xtmd")).unwrap(), &[pages.join("pick")])
}

/// A value as a host writes it, in the IR's form.
fn expr(x: &Json) -> Expr {
    match x["token"].as_str().and_then(|t| t.rsplit_once('.')) {
        Some((ty, name)) => serde_json::from_value(json!({"case": {"enum": ty, "name": name}})).unwrap(),
        None => serde_json::from_value(x.clone()).unwrap(),
    }
}

fn fields(x: &Json) -> Vec<(String, Expr)> {
    x.as_object().unwrap().iter().map(|(k, v)| (k.clone(), expr(v))).collect()
}

/// What the page holds: `$state`, the channel's values, and the overlay's own.
fn holds(p: &Page) -> BTreeMap<String, Expr> {
    let mut out = p.observe_state();
    for name in ["n", "s", "p"] {
        out.insert(format!("host.{}", name), p.given_extern(&format!("doc/host.{}", name)).unwrap().to_ir());
    }
    out.insert("s3.open".into(), p.find("s3").unwrap().prop("open").unwrap().to_ir());
    out
}

#[test]
fn what_comes_from_outside_is_of_its_type_or_refused() {
    let m = module();
    let cases: Json = serde_json::from_str(&std::fs::read_to_string(repo().join("checks/pages/outside.json")).unwrap()).unwrap();
    let changes = cases["changes"].as_array().unwrap();
    assert!(changes.len() > 40);
    for c in changes {
        let name = c["name"].as_str().unwrap();
        let mut p = Page::from_module(m.clone(), PageOptions::default()).unwrap();
        let mut want = holds(&p);
        let d = &c["do"];
        let args = |x: &Json| x["args"].as_array().unwrap().iter().map(expr).collect::<Vec<_>>();
        if d["set"].is_object() {
            p.set_given(&fields(&d["set"])).unwrap();
        }
        if d["extern"].is_object() {
            p.set_extern_given("doc/host", &fields(&d["extern"])).unwrap();
        }
        if d["expose"].is_object() {
            let args = args(&d["expose"]).iter().map(|x| p.value(x).unwrap()).collect();
            p.expose("doc/host", d["expose"]["name"].as_str().unwrap(), args).unwrap();
        }
        if d["write"].is_object() {
            let v = p.value(&expr(&d["write"]["value"])).unwrap();
            p.write(d["write"]["id"].as_str().unwrap(), d["write"]["input"].as_str().unwrap(), v).unwrap();
        }
        if d["event"].is_object() {
            let args = args(&d["event"]).iter().map(|x| p.value(x).unwrap()).collect();
            p.event(d["event"]["id"].as_str().unwrap(), d["event"]["name"].as_str().unwrap(), args).unwrap();
        }
        let out = p.take_outcome();
        let told = |k: &str| c.get(k).cloned().unwrap_or(json!([]));
        assert_eq!(json!({"refused": out.refused, "failures": out.failures}), json!({"refused": told("refused"), "failures": told("failures")}), "{}", name);
        if c["state"].is_object() {
            want.extend(fields(&c["state"]));
        }
        assert_eq!(holds(&p), want, "{}", name);
    }
}

/// A change of outside.json's, as actions.
fn actions(d: &Json) -> Vec<Json> {
    let mut out = Vec::new();
    if d["set"].is_object() {
        out.push(json!({"set": {"fields": d["set"]}}));
    }
    if d["extern"].is_object() {
        out.push(json!({"extern": {"channel": "doc/host", "values": d["extern"]}}));
    }
    if d["expose"].is_object() {
        out.push(json!({"expose": {"channel": "doc/host", "name": d["expose"]["name"], "args": d["expose"]["args"]}}));
    }
    out.extend(["write", "event"].iter().filter(|k| d[**k].is_object()).map(|k| json!({*k: d[*k]})));
    out
}

#[test]
fn each_change_is_the_same_as_an_action_and_what_it_led_to_is_told_once() {
    let m = module();
    let cases: Json = serde_json::from_str(&std::fs::read_to_string(repo().join("checks/pages/outside.json")).unwrap()).unwrap();
    for c in cases["changes"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let mut p = Page::from_module(m.clone(), PageOptions::default()).unwrap();
        let mut want = holds(&p);
        for a in actions(&c["do"]) {
            let a: xtmd_runtime::actions::Action = serde_json::from_value(a.clone()).unwrap_or_else(|e| panic!("{name}: {a}: {e}"));
            p.act(&a).unwrap();
        }
        let mut told = serde_json::Map::new();
        for k in ["refused", "failures"] {
            if let Some(x) = c.get(k) {
                told.insert(k.into(), x.clone());
            }
        }
        assert_eq!(serde_json::to_value(p.told()).unwrap(), Json::Object(told), "{name}");
        assert_eq!(serde_json::to_value(p.told()).unwrap(), json!({}), "{name}");
        if c["state"].is_object() {
            want.extend(fields(&c["state"]));
        }
        assert_eq!(holds(&p), want, "{name}");
    }
}

#[test]
fn the_pages_values_and_its_data_are_actions_too() {
    let mut p = Page::from_module(module(), PageOptions::default()).unwrap();
    let data = p.data().unwrap();
    let act = |p: &mut Page, a: Json| p.act(&serde_json::from_value(a).unwrap()).unwrap();
    act(&mut p, json!({"set": {"fields": {"n": {"num": "7"}}}}));
    act(&mut p, json!({"page": {"values": {"theme": {"case": {"enum": "core/theme", "name": "DARK"}}}}}));
    assert_eq!(serde_json::to_value(p.given_extern("core/page.theme").unwrap().to_ir()).unwrap()["case"]["name"], "DARK");
    act(&mut p, json!({"restore": {"data": data}}));
    assert_eq!(p.data().unwrap(), data);
}

#[test]
fn what_a_page_starts_from_is_checked_as_what_comes_later_is() {
    let mut opts = PageOptions::default();
    opts.externs = vec![("doc/host".to_string(), vec![("n".to_string(), text("x")), ("s".to_string(), text("b"))])].into_iter().collect();
    opts.state = vec![("n".to_string(), text("x")), ("t".to_string(), text("b"))];
    let mut p = Page::from_module(module(), opts).unwrap();
    let mut sites: Vec<String> = p.take_outcome().refused.into_iter().filter_map(|b| b.site).collect();
    sites.sort();
    assert_eq!(sites, vec!["$state.n", "doc/host.n"]);
    let now = holds(&p);
    assert_eq!([&now["host.n"], &now["host.s"], &now["n"], &now["t"]], [&num("3"), &text("b"), &num("3"), &text("b")]);
}

#[test]
fn a_fn_the_engine_supplies_gives_a_value_of_its_type() {
    let given = |x: Expr| {
        let mut opts = PageOptions::default();
        opts.fns.insert("pick/dial".into(), Rc::new(move |m, _| m.eval(&x).map(Some)));
        Page::from_module(module(), opts).unwrap().take_outcome().failures
    };
    assert_eq!(given(num("4")), vec![]);
    for x in [num("99"), text("x")] {
        assert_eq!(given(x).iter().map(|b| b.code.clone()).collect::<Vec<_>>(), vec![Some("failure.rule".to_string())]);
    }
}

#[test]
fn a_failure_in_the_documents_own_code_stays_where_it_is() {
    let mut p = page("$state {\n    n: int = 1;\n}\n@@@\nOne \\{10 / $state.n} two.\n\nThe rest.\n\n@slider(value = $state.n, label = \"N\");\n");
    let zero = v(&p, num("0"));
    p.set(vec![("n".into(), zero)]).unwrap();
    assert_eq!(p.observe_state()["n"], num("0"));
    assert_eq!(p.take_outcome().failures, vec![Broken { code: Some("failure.number".into()), rule: None, site: Some("m0".into()) }]);
    assert_eq!(p.find("m0").unwrap().failed.as_deref(), Some("a division by zero"));
    assert_eq!(p.markdown().unwrap(), "> paragraph failed: a division by zero\n\nThe rest.\n\nN: 0\n");
    let two = v(&p, num("2"));
    p.write("s2", "value", two).unwrap();
    assert_eq!(p.markdown().unwrap(), "One 5 two.\n\nThe rest.\n\nN: 2\n");
}

const TASKS: &str = r#"record task {
    mut title: text;
}

$state {
    xs: list<task> = [task { title = "a" }, task { title = "b" }];
}

widget row(x: task) {
    mut votes: int = 0;
    view {
        @press(label = "\{x.title}: \{votes}", click = fn() { votes = votes + 1; });
    }
    markdown { }
}

widget board() {
    view {
        @press(label = "Add", click = fn() { $state.xs.add(task { title = "new" }); });
        mut for (t in $state.xs) {
            if (mut x: task = t) {
                @row(x = x);
                @press(label = "Mark \{x.title}", click = fn() { x.title = "\{x.title}!"; });
            }
        }
    }
    markdown { }
}
@@@
@board;
"#;

fn labels(p: &Page) -> Vec<(String, String)> {
    p.tree().iter().map(|d| (d.id.clone(), d.prop("label").and_then(|v| v.as_text()).unwrap_or_default().to_string())).collect()
}

fn said(p: &Page) -> Vec<String> {
    labels(p).into_iter().map(|l| l.1).collect()
}

fn press(p: &mut Page, label: &str) {
    let id = labels(p).into_iter().find(|l| l.1 == label).unwrap().0;
    p.event(&id, "click", vec![]).unwrap();
}

fn tasks(of: &[&str]) -> Json {
    json!({"list": of.iter().map(|x| json!({"record": {"type": "doc/task", "fields": {"title": {"text": x}}}})).collect::<Vec<_>>()})
}

fn give(p: &mut Page, of: &[&str]) {
    let xs = v(p, serde_json::from_value(tasks(of)).unwrap());
    p.set(vec![("xs".into(), xs)]).unwrap();
}

#[test]
fn an_item_a_handler_adds_is_another_than_every_item_there() {
    for given in [false, true] {
        // with no widget of its own in a row, so nothing else takes a number
        let mut p = page(&TASKS.replace("                @row(x = x);\n", ""));
        if given {
            give(&mut p, &["a", "b"]);
        }
        press(&mut p, "Add");
        assert_eq!(said(&p), vec!["Add", "Mark a", "Mark b", "Mark new"]);
        press(&mut p, "Mark new");
        assert_eq!(serde_json::to_value(&p.observe_state()["xs"]).unwrap(), tasks(&["a", "b", "new!"]));
    }
}

#[test]
fn a_list_the_host_replaces_has_its_rows_made_again() {
    let mut p = page(TASKS);
    press(&mut p, "a: 0");
    press(&mut p, "b: 0");
    assert_eq!(said(&p).into_iter().filter(|x| x.contains(':')).collect::<Vec<_>>(), vec!["a: 1", "b: 1"]);
    give(&mut p, &["c", "d"]);
    assert_eq!(said(&p).into_iter().filter(|x| x.contains(':')).collect::<Vec<_>>(), vec!["c: 0", "d: 0"]);
}
