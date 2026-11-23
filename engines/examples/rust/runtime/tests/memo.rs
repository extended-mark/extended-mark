//! The memo never changes what is drawn: a page is run with it and without it, through the same
//! events, and comes to the same tree and Markdown after each. What reaches a slot only through the
//! caller's own values is what a memo can forget (#14005).

mod common;

use common::build_with;
use xtmd_runtime::{Page, PageOptions};

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
