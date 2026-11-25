//! The host's side (page 6): the tree every runtime gives, its changes, ids, fns and data, and that a change
//! works out only what reads it. The same as @extended-mark/runtime's host.test.mjs.

mod common;

use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

use common::*;
use xtmd_runtime::host::apply;
use xtmd_runtime::ir;
use xtmd_runtime::state::Data;
use xtmd_runtime::tree::{Change, Child, Input, Node, Tree};
use xtmd_runtime::{Page, PageData, PageOptions, PageTheme, PageValues, PageWidth};

fn every<'a>(nodes: &'a [Node], out: &mut Vec<&'a Node>) {
    fn inside<'a>(x: &'a Input, out: &mut Vec<&'a Node>) {
        match x {
            Input::Nodes(ns) => every(ns, out),
            Input::List(xs) => xs.iter().for_each(|x| inside(x, out)),
            Input::Record(r) => r.fields.values().for_each(|x| inside(x, out)),
            Input::Some(x) => inside(x, out),
            _ => {}
        }
    }
    for n in nodes {
        out.push(n);
        n.inputs.values().for_each(|x| inside(x, out));
    }
}

fn find<'a>(t: &'a Tree, widget: &str, k: usize) -> &'a Node {
    let mut all = Vec::new();
    every(&t.nodes, &mut all);
    all.into_iter().filter(|n| n.widget == widget).nth(k).unwrap_or_else(|| panic!("no {} {}", widget, k))
}

fn num_input(n: &str) -> Input {
    Input::Num(n.into())
}

fn text_of(n: &Node, input: &str) -> String {
    match &n.inputs[input] {
        Input::Text(t) => t.clone(),
        other => panic!("{:?}", other),
    }
}

#[test]
fn a_pages_tree_is_the_one_every_engine_gives() {
    for (page, tree) in [("schemas/examples/counter.xtmd", "schemas/examples/counter.tree.json"), ("apps/example-host/page.xtmd", "apps/example-host/test/page.tree.json")] {
        let opts = PageOptions { externs: vec![("doc/clock".into(), vec![("today".into(), text("Tuesday"))])], ..PageOptions::default() };
        let mut p = page_with(&std::fs::read_to_string(repo().join(page)).unwrap(), opts);
        let want: Tree = serde_json::from_str(&std::fs::read_to_string(repo().join(tree)).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(p.host_tree().unwrap()).unwrap(), serde_json::to_value(want).unwrap(), "{}", page);
    }
}

const TASKS: &str = "record task {\n    title: text;\n}\n\n$state {\n    tasks: list<task> = [task { title = \"a\" }, task { title = \"b\" }, task { title = \"c\" }];\n}\n\nwidget row(t: task) {\n    mut votes: int = 0;\n    view {\n        @press(label = \"\\{t.title}: \\{votes}\", click = fn() { votes = votes + 1; });\n    }\n}\n\nwidget board() {\n    view {\n        mut for (t in $state.tasks) {\n            if (mut x: task = t) {\n                @row(t = x);\n                @press(label = \"Drop\", click = fn() { $state.tasks.remove(x); });\n            }\n        }\n    }\n}\n@@@\n@board;\n";

#[test]
fn an_id_follows_its_item_and_what_changed_says_what_is_kept() {
    let mut p = page(TASKS);
    let first = p.host_tree().unwrap();
    let b = find(&first, "core/press", 2).clone();
    assert_eq!(text_of(&b, "label"), "b: 0");
    p.event(&b.id, "click", vec![]).unwrap();
    let (changes, now) = p.host_changes(&first).unwrap();
    assert_eq!(changes, vec![Change { id: b.id.clone(), inputs: BTreeMap::from([("label".into(), Input::Text("b: 1".into()))]), children: BTreeMap::new(), events: None, writes: None }]);
    // the first row goes: the rest are kept, by their ids
    let drop = find(&now, "core/press", 1).id.clone();
    p.event(&drop, "click", vec![]).unwrap();
    let (changes, last) = p.host_changes(&now).unwrap();
    assert_eq!(changes.len(), 1);
    assert!(changes[0].id.is_empty() && changes[0].children["nodes"].iter().all(|c| matches!(c, Child::Keep(_))));
    assert_eq!(text_of(find(&last, "core/press", 0), "label"), "b: 1");
    assert_eq!(apply(&now, &changes), last);
}

#[test]
fn a_widget_that_fails_is_a_node_that_says_why() {
    let mut p = page("$state {\n    parts: int 0... = 0;\n}\n\nwidget share() {\n    view {\n        @label(value = \"Each: \\{100 / $state.parts}\");\n    }\n}\n@@@\n@share;\n\nAfter.\n");
    let t = p.host_tree().unwrap();
    assert_eq!((t.nodes[0].widget.as_str(), t.nodes[0].failed.as_deref(), t.nodes[0].inputs.len()), ("doc/share", Some("a division by zero"), 0));
    p.set(vec![("parts".into(), v(&p, num("4")))]).unwrap();
    let (changes, now) = p.host_changes(&t).unwrap();
    assert!(matches!(&changes[0].children["nodes"][..], [Child::Node(_), Child::Keep(_)]), "{:?}", changes);
    assert_eq!(text_of(&now.nodes[0], "value"), "Each: 25");
}

#[test]
fn a_fn_moves_when_what_it_reads_does() {
    let mut p = page("$state {\n    red: number 0..1 = 1;\n    other: int = 0;\n}\n\nfn tint(at: point, px: number, args: number): pixel {\n    return pixel { r = $state.red, g = at.x.clamp(0, 1), b = 0, a = 1 };\n}\n\nwidget picture() {\n    view {\n        @canvas(label = \"Picture\") {\n            @layer {\n                @shade(label = \"Tint\", area = region { from = point { x = 0, y = 0 }, to = point { x = 1, y = 1 } }, shader = tint, args = 0);\n            }\n        }\n    }\n}\n@@@\n@picture;\n");
    let t = p.host_tree().unwrap();
    let id = find(&t, "core/shade", 0).id.clone();
    let at: Input = serde_json::from_value(serde_json::json!({ "record": { "type": "core/point", "fields": { "x": { "num": "0.5" }, "y": { "num": "0" } } } })).unwrap();
    let args = [at, num_input("0.01"), num_input("0")];
    let red = |x: Input| match x {
        Input::Record(r) => r.fields["r"].clone(),
        other => panic!("{:?}", other),
    };
    assert_eq!(red(p.call(&id, "shader", &args).unwrap()), num_input("1"));
    let version = |t: &Tree| match &find(t, "core/shade", 0).inputs["shader"] {
        Input::Fn(f) => f.version,
        other => panic!("{:?}", other),
    };
    let was = version(&t);
    p.set(vec![("other".into(), v(&p, num("1")))]).unwrap();
    p.set_page(PageValues { width: Some(PageWidth::Wide), theme: Some(PageTheme::Dark) }).unwrap();
    assert_eq!(version(&p.host_tree().unwrap()), was);
    p.set(vec![("red".into(), v(&p, num("0.25")))]).unwrap();
    let (changes, now) = p.host_changes(&t).unwrap();
    assert_eq!((version(&now), changes.len(), changes[0].id.as_str()), (was + 1, 1, id.as_str()));
    assert_eq!(red(p.call(&id, "shader", &args).unwrap()), num_input("0.25"));
}

#[test]
fn a_pages_data_takes_out_what_it_holds_and_puts_it_back() {
    let mut p = page(TASKS);
    let id = find(&p.host_tree().unwrap(), "core/press", 2).id.clone();
    p.event(&id, "click", vec![]).unwrap();
    let data = p.data().unwrap();
    let data: Data = serde_json::from_str(&serde_json::to_string(&data).unwrap()).unwrap();
    let mut q = page(TASKS);
    let state = data.state.iter().map(|(k, x)| (k.clone(), q.value_of(x).unwrap())).collect();
    q.set(state).unwrap();
    let own = data.own.iter().map(|(p, vs)| (p.clone(), vs.iter().map(|(k, x)| (k.clone(), serde_json::from_value::<ir::Expr>(serde_json::to_value(x).unwrap()).unwrap())).collect())).collect();
    assert_eq!(q.restore_own(&own).unwrap(), Vec::<String>::new());
    assert_eq!(q.data().unwrap(), data);
    assert_eq!(text_of(find(&q.host_tree().unwrap(), "core/press", 2), "label"), "b: 1");
}

#[test]
fn a_page_that_starts_from_its_data_is_drawn_once_with_it() {
    let text = "$state {\n    open: bool = false;\n    n: int 0..10 = 0;\n}\n\nwidget counter() {\n    mut k: int 0..5 = 0;\n    view {\n        @press(label = \"\\{k}\", click = fn() { k = (k + 1).clamp(0, 5); });\n    }\n    markdown {\n        x\n    }\n}\n@@@\n@if ($state.open) {\n@counter;\n\n@overlay(label = \"More\") {\n@slot anchor {\n@label(value = \"Open\");\n}\n@slot body {\nIn it.\n}\n}\n}\n\n\\{$state.n}\n";
    let file = common::build_with(text, &[]);
    let mut p = Page::new(ir::File { ir: file.clone() }, PageOptions::default()).unwrap();
    let on = p.value(&ir::Expr::Bool(true)).unwrap();
    let three = p.value(&num("3")).unwrap();
    p.set(vec![("open".into(), on), ("n".into(), three)]).unwrap();
    let t = p.host_tree().unwrap();
    p.event(&find(&t, "core/press", 0).id.clone(), "click", vec![]).unwrap();
    let open = p.value(&ir::Expr::Bool(true)).unwrap();
    p.write(&find(&t, "core/overlay", 0).id.clone(), "open", open).unwrap();
    let data = PageData { state: p.observe_state().into_iter().collect(), own: p.observe_own() };
    assert_eq!(data.own.len(), 2);
    let mut q = Page::new(ir::File { ir: file.clone() }, PageOptions { data: Some(data.clone()), ..PageOptions::default() }).unwrap();
    assert_eq!((q.revision(), q.left()), (1, Some(&[][..])));
    assert_eq!((q.observe_state(), q.observe_own()), (p.observe_state(), p.observe_own()));
    let t = q.host_tree().unwrap();
    assert_eq!((text_of(find(&t, "core/press", 0), "label"), &find(&t, "core/overlay", 0).inputs["open"]), ("1".to_string(), &Input::Bool(true)));
    // later, over what it holds: one draw more
    let mut r = Page::new(ir::File { ir: file.clone() }, PageOptions::default()).unwrap();
    assert_eq!(r.restore(&data).unwrap(), Some(vec![]));
    assert_eq!((r.revision(), r.observe_own()), (2, p.observe_own()));
    // what doesn't fit is left out, and a refused `$state` puts nothing back
    let mut odd = data.clone();
    odd.own.insert("s9".into(), [("k".to_string(), num("1"))].into_iter().collect());
    let counter = odd.own.iter_mut().find(|(_, vs)| vs.contains_key("k")).map(|(p, vs)| (p.clone(), vs)).unwrap();
    let wrong = format!("{}.k", counter.0);
    counter.1.insert("k".into(), ir::Expr::Text("many".into()));
    assert_eq!(Page::new(ir::File { ir: file.clone() }, PageOptions { data: Some(odd), ..PageOptions::default() }).unwrap().left(), Some(&[wrong, "s9.k".to_string()][..]));
    let bad = PageData { state: vec![("n".into(), num("11"))], own: data.own.clone() };
    let mut s = Page::new(ir::File { ir: file }, PageOptions { data: Some(bad.clone()), ..PageOptions::default() }).unwrap();
    assert_eq!((s.left(), s.revision()), (None, 1));
    assert_eq!(s.take_outcome().refused.len(), 1);
    assert_eq!(s.restore(&bad).unwrap(), None);
}

#[test]
fn what_a_widget_writes_has_to_fit_what_its_node_says() {
    let mut p = page("$state {\n    n: int 0..10 = 3;\n}\n@@@\n@slider(value = $state.n, label = \"N\");\n");
    let t = p.host_tree().unwrap();
    let s = find(&t, "core/slider", 0);
    assert_eq!(serde_json::to_value(&s.writes["value"]).unwrap(), serde_json::json!({ "range": { "low": { "num": "0" }, "high": { "num": "10" } }, "step": { "num": "1" } }));
}

/// Something a reader or a host could do now, the `k`th of them.
fn act(p: &mut Page, t: &Tree, k: usize) {
    let mut all = Vec::new();
    every(&t.nodes, &mut all);
    let mut acts: Vec<Box<dyn Fn(&mut Page)>> = vec![Box::new(move |p| p.set_page(PageValues { width: Some(if k % 2 == 0 { PageWidth::Narrow } else { PageWidth::Wide }), theme: Some(if k % 3 == 0 { PageTheme::Dark } else { PageTheme::Light }) }).unwrap())];
    for n in all {
        for e in &n.events {
            let (id, e) = (n.id.clone(), e.clone());
            acts.push(Box::new(move |p| p.event(&id, &e, vec![]).unwrap()));
        }
        if n.widget == "core/slider" {
            let id = n.id.clone();
            acts.push(Box::new(move |p| {
                let x = v(p, num(&(k % 12).to_string()));
                p.write(&id, "value", x).unwrap()
            }));
        }
    }
    let n = acts.len();
    acts.swap_remove((k * 7919) % n)(p);
}

#[test]
fn changes_make_the_next_tree_and_ids_are_one_each() {
    let mut changed = 0;
    for (text, packs) in [(std::fs::read_to_string(repo().join("apps/example-host/page.xtmd")).unwrap(), vec![]), (TASKS.to_string(), vec![]), (std::fs::read_to_string(repo().join("runtime/typescript/test/cases/graphs-release-flow.xtmd")).unwrap(), vec![repo().join("examples/graphs")])] {
        let opts = PageOptions { supplies: Some(Rc::new(|_: &str| true)), ..PageOptions::default() };
        let mut p = Page::from_module(build_with(&text, &packs), opts).unwrap();
        let mut last = p.host_tree().unwrap();
        for k in 0..20 {
            act(&mut p, &last, k + 1);
            let (changes, now) = p.host_changes(&last).unwrap();
            assert_eq!(apply(&last, &changes), now, "after change {}", k);
            let mut all = Vec::new();
            every(&now.nodes, &mut all);
            assert_eq!(all.iter().map(|n| n.id.as_str()).collect::<HashSet<_>>().len(), all.len());
            changed += changes.len();
            last = now;
        }
    }
    assert!(changed > 20, "{changed}");
}

#[test]
fn a_change_works_out_only_what_reads_it_whatever_else_is_on_the_page() {
    let doc = |rows: usize| {
        let list = (0..rows).map(|k| k.to_string()).collect::<Vec<_>>().join(", ");
        let prose = (0..rows).map(|k| format!("Paragraph {k} with **\\{{$state.other}}** in it.")).collect::<Vec<_>>().join("\n\n");
        format!("$state {{\n    n: int = 0;\n    other: int = 0;\n    unread: int = 0;\n    rows: list<int> = [{list}];\n}}\n\nwidget one(k: int) {{\n    view {{\n        if (k > 3) {{\n            @label(value = \"row \\{{k}} of \\{{$state.rows.len()}}\");\n        }} else {{\n            @label(value = \"small \\{{k}}\");\n        }}\n    }}\n}}\n\nwidget all() {{\n    view {{\n        for (r in $state.rows) {{\n            @one(k = r);\n        }}\n        @label(value = \"n is \\{{$state.n}}\");\n    }}\n}}\n@@@\n@all;\n\n{prose}\n")
    };
    let worked = |rows: usize, field: &str| {
        let mut p = page(&doc(rows));
        let before = p.worked();
        let x = v(&p, num("1"));
        p.set(vec![(field.into(), x)]).unwrap();
        p.worked() - before
    };
    let few = worked(8, "n");
    assert!(few <= 12, "{few} worked out for one label");
    assert_eq!(worked(80, "n"), few);
    assert!(worked(40, "unread") <= 1);
    let per = worked(21, "other") - worked(20, "other");
    assert_eq!(worked(40, "other") - worked(20, "other"), 20 * per);
}

#[test]
fn the_hosts_state_is_checked_before_the_page_is_drawn() {
    let text = "$state {\n    n: int 0..10 = 3;\n    m: int = 0;\n    require \"m is over n\" {\n        return m <= n;\n    }\n}\n@@@\nn is \\{$state.n}\n";
    let start = |state: &[(&str, &str)]| {
        let mut p = page_with(text, PageOptions { state: state.iter().map(|(k, x)| (k.to_string(), num(x))).collect(), ..PageOptions::default() });
        let told: Vec<String> = p.take_outcome().refused.into_iter().map(|b| b.rule.or(b.site).unwrap()).collect();
        (p.observe_state(), told, p.revision())
    };
    let is = |n: &str, m: &str| BTreeMap::from([("n".to_string(), num(n)), ("m".to_string(), num(m))]);
    // all together, where one alone would break a rule
    assert_eq!(start(&[("m", "5"), ("n", "6")]), (is("6", "5"), vec![], 1));
    // else each that fits, in order
    assert_eq!(start(&[("n", "40"), ("m", "2"), ("gone", "1")]), (is("3", "2"), vec!["$state.n".into(), "$state.gone".into()], 1));
    assert_eq!(start(&[("m", "5")]), (is("3", "0"), vec!["m is over n".into()], 1));
}
