//! A keyword is never a name (page 1), whatever it would name and wherever the language reads it.

use xtmd_syntax::lexer::KEYWORDS;
use xtmd_syntax::{parse, Entry};

const NAMED: &[(&str, &str)] = &[
    ("a widget", "package t;\n\nwidget NAME() {\n    view {\n        @label(value = \"a\");\n    }\n    markdown {\n    }\n}\n"),
    ("an extern widget", "package t;\n\nextern widget NAME() {\n    markdown { }\n}\n"),
    ("an input", "package t;\n\nextern widget w(NAME: text) {\n    markdown { }\n}\n"),
    ("a slot", "package t;\n\nextern widget w() {\n    slot NAME: view;\n\n    markdown { }\n}\n"),
    ("an event", "package t;\n\nextern widget w() {\n    event NAME();\n\n    markdown { }\n}\n"),
    ("a record", "package t;\n\nrecord NAME {\n    a: text;\n}\n"),
    ("a field", "package t;\n\nrecord r {\n    NAME: text;\n}\n"),
    ("an enum", "package t;\n\nenum NAME { A }\n"),
    ("a fn", "package t;\n\nfn NAME(): number {\n    return 1;\n}\n"),
    ("a parameter", "package t;\n\nfn f(NAME: number): number {\n    return 1;\n}\n"),
    ("a let", "package t;\n\nlet NAME: number = 1;\n"),
    ("a name provided", "package t;\n\npack t@1 {\n    provides NAME;\n}\n"),
];

#[test]
fn no_keyword_names_anything() {
    for (what, src) in NAMED {
        assert!(parse(&src.replace("NAME", "ok"), Entry::Xtmd).diagnostics.is_empty(), "{what}");
        // `self` is what a fn's first parameter is called
        for (word, _) in KEYWORDS.iter().filter(|(w, _)| !(*w == "self" && *what == "a parameter")) {
            let d = parse(&src.replace("NAME", word), Entry::Xtmd).diagnostics;
            assert!(d.iter().any(|d| d.is_error()), "`{word}` is {what}'s name");
        }
    }
}

#[test]
fn a_documents_names_are_held_to_it_too() {
    for (word, _) in KEYWORDS.iter().filter(|(w, _)| !w.starts_with('$')) {
        let d = parse(&format!("$state {{\n    {word}: number = 1;\n}}\n@@@\nHi\n"), Entry::Document).diagnostics;
        assert!(d.iter().any(|d| d.is_error()), "`{word}` is a field of $state's");
    }
}
