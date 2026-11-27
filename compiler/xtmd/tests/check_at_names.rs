//! Names after `@` (page 3): a value is looked up first only when it can be placed, a slot, a view held
//! in a value or a fn that returns one; any other value is skipped, and the name calls the widget.

use xtmd::check::{compile_core, compile_on, Source};
use xtmd::formats::ir::{self, Node};
use std::path::PathBuf;

fn core_sources() -> Vec<Source> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../core");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().map_or(false, |x| x == "xtmdpack")).collect();
    files.sort();
    files.into_iter().map(|p| Source { path: format!("core/{}", p.file_name().unwrap().to_str().unwrap()), text: std::fs::read_to_string(&p).unwrap() }).collect()
}

// as XTMD Integration reported it on 0.3.2: an input named `icon` hid core's widget `icon`
const DOC: &str = r#"widget p2(text: text, icon: opt<glyph> = opt.NONE) {
    view {
        @row {
            if (let g: glyph = icon) {
                @icon(name = g);
            }
            @label(value = text);
        }
    }
}

widget p3(label: text, icon: glyph = glyph.UP) {
    view {
        @icon(name = icon);
        @label(value = label);
    }
}

widget holder(shown: view) {
    view {
        @shown;
    }
}

fn mark(): glyph {
    return glyph.DOWN;
}

widget p4() {
    view {
        @icon(name = mark());
    }
}
@@@
@p2(text = "x", icon = glyph.UP);
@p3(label = "y");
@holder(shown = @label(value = "held"));
@p4;
"#;

/// Every widget each node of a view calls, through conditions, loops and slots.
fn widgets(nodes: &[Node], out: &mut Vec<String>) {
    for n in nodes {
        match n {
            Node::Extern(e) => {
                out.push(e.name.clone());
                for s in e.slots.values() {
                    widgets(s, out);
                }
            }
            Node::Use(u) => {
                out.push(u.name.clone());
                for s in u.slots.values() {
                    widgets(s, out);
                }
            }
            Node::If(i) => {
                for b in &i.branches {
                    widgets(&b.then, out);
                }
                widgets(&i.else_, out);
            }
            _ => {}
        }
    }
}

#[test]
fn an_input_that_holds_no_view_never_hides_a_widget() {
    let base = compile_core(&core_sources());
    let c = compile_on(&base, &[], &[Source { path: "at.xtmd".into(), text: DOC.into() }], "doc");
    let errors: Vec<String> = c.errors().map(|d| format!("{}: {}", d.file, d.diagnostic.message)).collect();
    assert!(errors.is_empty(), "{:?}", errors);
    let m = c.module.unwrap();
    let called = |name: &str| -> Vec<String> {
        let t: &ir::Template = m.templates.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("no template {}", name));
        let mut out = Vec::new();
        widgets(&t.view, &mut out);
        out
    };
    // `icon: opt<glyph>`, read in an `if let`: `@icon(name = g)` is core's widget
    assert!(called("doc/p2").contains(&"core/icon".to_string()), "{:?}", called("doc/p2"));
    // `icon: glyph`, given to the widget of the same name
    assert!(called("doc/p3").contains(&"core/icon".to_string()), "{:?}", called("doc/p3"));
    // a glyph a fn returns, given to the widget
    assert!(called("doc/p4").contains(&"core/icon".to_string()), "{:?}", called("doc/p4"));
    // an input that holds a view is still placed by `@shown`, not taken for a widget
    assert!(!called("doc/holder").iter().any(|w| w.ends_with("/shown")), "{:?}", called("doc/holder"));
}
