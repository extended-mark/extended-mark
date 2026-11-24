//! The memo never changes what is drawn: a page is run with it and without it, through the same
//! events, and comes to the same tree and Markdown after each. What reaches a slot only through the
//! caller's own values is what a memo can forget (#14005).

mod common;

use common::build_with;
use xtmd_runtime::ir::Expr;
use xtmd_runtime::{Drawn, Page, PageOptions};

const SLOTS: &[(&str, &str)] = &[
    ("a let, two slots deep", "@column {\n @row {\n @label(value = \"on \\{on}\");\n }\n }"),
    ("a let's branch", "@column {\n if (on) {\n @label(value = \"yes\");\n } else {\n @label(value = \"no\");\n }\n }"),
    ("a match on a let", "@frame(title = \"f\") {\n body {\n match (on) {\n case true {\n @label(value = \"yes\");\n }\n case false {\n @label(value = \"no\");\n }\n }\n }\n }"),
    ("a let in a loop", "@column {\n for (i in 0..2) {\n let mine: text = on ? \"yes \\{i}\" : \"no \\{i}\";\n @row {\n @label(value = mine);\n }\n }\n }"),
    ("a widget that places its slot twice", "@twice {\n @label(value = \"on \\{on}\");\n }"),
];

fn page(view: &str) -> String {
    format!(
        "widget twice() {{\n slot body: view;\n view {{\n @column {{\n @body;\n }}\n @column {{\n @body;\n }}\n }}\n markdown {{\n x\n }}\n}}\n\nwidget w() {{\n mut turned: bool = false;\n fn odd(): bool {{\n return turned;\n }}\n view {{\n let on: bool = odd();\n @press(label = \"turn\", click = fn() {{ turned = !turned; }});\n {}\n }}\n markdown {{\n \\{{turned}}\n }}\n}}\n@@@\n@w;\n",
        view
    )
}

#[test]
fn a_slot_keeps_up_with_the_callers_values() {
    for (name, view) in SLOTS {
        let m = build_with(&page(view), &[]);
        let open = |memo: bool| Page::new(xtmd_runtime::ir::File { ir: m.clone() }, PageOptions { memo, ..PageOptions::default() }).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let (mut on, mut off) = (open(true), open(false));
        for turn in 1..4 {
            for p in [&mut on, &mut off] {
                p.event("s0/s0", "click", Vec::new()).unwrap();
            }
            assert_eq!(format!("{:?}", on.observe_tree()), format!("{:?}", off.observe_tree()), "{name}: turn {turn}");
            assert_eq!(on.markdown().unwrap(), off.markdown().unwrap(), "{name}: turn {turn}");
        }
    }
}

#[test]
fn an_optional_list_slot_left_out_empty_and_filled() {
    let m = build_with(
        "widget about(t: text) {\n view {\n @label(value = t);\n }\n markdown {\n \\{t}\n }\n}\n\nwidget box(name: text) {\n slot abouts: opt<list<about>>;\n view {\n if (let list: list<about> = abouts) {\n @label(value = \"\\{name} \\{list.len()}\");\n } else {\n @label(value = \"\\{name} none\");\n }\n }\n markdown {\n x\n }\n}\n\nwidget w() {\n view {\n @box(name = \"a\");\n @box(name = \"b\") {\n }\n @box(name = \"c\") {\n match (true) {\n case true {\n @about(t = \"1\");\n }\n case false {\n }\n }\n }\n }\n markdown {\n x\n }\n}\n@@@\n@w;\n",
        &[],
    );
    let p = Page::new(xtmd_runtime::ir::File { ir: m }, PageOptions::default()).unwrap();
    let tree = format!("{:?}", p.observe_tree());
    for want in ["a none", "b 0", "c 1"] {
        assert!(tree.contains(want), "{want} in {tree}");
    }
}

fn presses(nodes: &[Drawn], out: &mut Vec<Drawn>) {
    for n in nodes {
        if n.widget == "core/press" || n.widget == "core/field" || n.widget == "core/slider" {
            out.push(n.clone());
        }
        n.slots.values().for_each(|kids| presses(kids, out));
    }
}

/// What reads a part of a value, an item of a list, a field of one or what is inside an opt, keeps up with
/// it however it is changed: 240 changes, the same with the memo and without.
#[test]
fn what_reads_a_part_of_a_value_keeps_up_with_it() {
    let m = build_with(&std::fs::read_to_string(common::repo().join("checks/pages/parts.xtmd")).unwrap(), &[]);
    let mut seed: u64 = 0x9e37_79b9;
    let mut next = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    for run in 0..6 {
        let open = |memo: bool| Page::new(xtmd_runtime::ir::File { ir: m.clone() }, PageOptions { memo, ..PageOptions::default() }).unwrap();
        let (mut on, mut off) = (open(true), open(false));
        for step in 0..40 {
            let mut all = Vec::new();
            presses(off.tree(), &mut all);
            let d = all[next(all.len())].clone();
            let typed = Expr::Text(["", "a", "lemon"][next(3)].into());
            let slid = Expr::Num(next(5).to_string());
            for p in [&mut on, &mut off] {
                // by its place: the two number their items apart
                let id = p.at(&d.path).unwrap().id.clone();
                match d.widget.as_str() {
                    "core/press" => p.event(&id, "click", Vec::new()).unwrap(),
                    "core/field" => {
                        let v = p.value(&typed).unwrap();
                        p.write(&id, "value", v).unwrap()
                    }
                    _ => {
                        let v = p.value(&slid).unwrap();
                        p.write(&id, "value", v).unwrap()
                    }
                }
            }
            assert_eq!(format!("{:?}", on.observe_tree()), format!("{:?}", off.observe_tree()), "run {run}, change {step}: {} {}", d.widget, d.id);
            assert_eq!(on.observe_state(), off.observe_state(), "run {run}, change {step}");
            assert_eq!(on.markdown().unwrap(), off.markdown().unwrap(), "run {run}, change {step}");
        }
    }
}

#[test]
fn a_row_that_changes_is_worked_out_and_the_other_rows_are_not() {
    let worked = |rows: usize| {
        let list = (0..rows).map(|k| format!("task {{ title = \"{k}\" }}")).collect::<Vec<_>>().join(", ");
        let m = build_with(&format!("record task {{\n mut title: text;\n mut done: bool = false;\n}}\n\n$state {{\n tasks: list<task> = [{list}];\n}}\n\nwidget row(mut t: task) {{\n view {{\n @press(label = t.title, selected = t.done, click = fn() {{ t.done = !t.done; }});\n }}\n markdown {{\n \\{{t.title}}\n }}\n}}\n\nwidget board() {{\n view {{\n mut for (t in $state.tasks) {{\n if (mut x: task = t) {{\n @row(t = x);\n }}\n }}\n }}\n markdown {{\n x\n }}\n}}\n@@@\n@board;\n"), &[]);
        let mut p = Page::from_module(m, PageOptions::default()).unwrap();
        let mut before = Vec::new();
        presses(p.tree(), &mut before);
        p.event(&before[2].id.clone(), "click", Vec::new()).unwrap();
        let mut after = Vec::new();
        presses(p.tree(), &mut after);
        (before.iter().zip(&after).filter(|(a, b)| !Drawn::same(a, b)).count(), after.len())
    };
    assert_eq!(worked(5), (1, 5));
    assert_eq!(worked(60), (1, 60));
}
