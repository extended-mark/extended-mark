//! The `@@@` line that ends a document's definitions, and what is not one.

use xtmd::syntax::{has_divider, parse, Entry};

fn divides(src: &str) -> bool {
    let p = parse(src, Entry::Document);
    let file = p.file.unwrap_or_else(|| panic!("doesn't read: {:?}", p.diagnostics));
    assert_eq!(file.divider.is_some(), has_divider(src), "{src:?}");
    file.divider.is_some()
}

#[test]
fn a_line_of_its_own_divides() {
    assert!(divides("@@@\n# A\n"));
    assert!(divides("let n: int = 1;\n   @@@   \n# A\n"));
    assert!(!divides("# A\n"));
    assert!(!divides("    @@@\n"));
}

#[test]
fn a_fence_hides_it() {
    assert!(!divides("# A\n\n```xtmd\nlet n: int = 1;\n@@@\n```\n"));
    assert!(!divides("~~~\n@@@\n~~~\n"));
    // closed only by as many of the same character
    assert!(!divides("````\n```\n@@@\n````\n"));
    assert!(!divides("~~~\n```\n@@@\n~~~\n"));
    assert!(!divides("- a\n\n  ```\n  @@@\n  ```\n"));
}

#[test]
fn it_divides_after_a_fence() {
    let p = parse("@@@\n```\n@@@\n```\n", Entry::Document);
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    assert_eq!(p.file.unwrap().divider.unwrap().start, 0);
}

#[test]
fn a_fence_never_closed_hides_nothing() {
    assert!(has_divider("```\n@@@\n# A\n"));
}

#[test]
fn front_matter_is_not_searched() {
    assert!(!divides("---\na: 1\n---\n```\n@@@\n```\n"));
    assert!(divides("---\na: 1\n---\n@@@\n# A\n"));
}
