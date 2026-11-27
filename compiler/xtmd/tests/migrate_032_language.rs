//! The step from 0.3.2, its language part (src/migrate/rir/v1): a row of plan #2443's section 3 a test.
//! Each writes a tree for 0.3.2, runs `xtmd migrate` on it, and says what it becomes: the files the
//! command wrote, which it compiled back to what the step made, and what it left for a person, with
//! its exit code (3 for a TODO or a question open).

use std::path::{Path, PathBuf};
use std::process::Command;

/// A tree for 0.3.2 in a folder of its own, its xtmd.toml saying so.
fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtmd-language-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("xtmd.toml"), "xtmd = \"0.3.2\"\n").unwrap();
    for (path, text) in files {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

/// `xtmd migrate` in `dir`, writing: its exit code and what it said.
fn migrate(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_xtmd")).current_dir(dir).arg("migrate").args(args).arg("--yes").env("XTMD_MIGRATE_STEP", "1").output().unwrap();
    (out.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

fn read(dir: &Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

/// A tree of one document moved: its exit code, what was said, and the document as written.
fn one(name: &str, doc: &str) -> (i32, String, String) {
    let dir = tree(name, &[("doc.xtmd", doc)]);
    let (code, said) = migrate(&dir, &["."]);
    let text = read(&dir, "doc.xtmd");
    let _ = std::fs::remove_dir_all(&dir);
    (code, said, text)
}

#[test]
fn b1_a_let_that_reads_the_page_becomes_a_fn_and_each_read_a_call() {
    let (code, said, doc) = one("b1", "$state {\n    n: int = 2;\n}\n\nlet twice: int = $state.n * 2;\nlet two: int = 2;\n@@@\nTwice is \\{twice}, and \\{two}.\n");
    assert_eq!(code, 0, "{}", said);
    assert!(doc.contains("fn twice(): int {\n    return $state.n * 2;\n}"), "{}", doc);
    assert!(doc.contains("Twice is \\{twice()}, and \\{two}."), "a constant stays a let: {}", doc);
    assert!(doc.contains("let two: int = 2;"), "{}", doc);
}

#[test]
fn b2_a_field_written_is_mut_and_one_of_another_packs_record_is_left_for_a_person() {
    let (code, said, doc) = one(
        "b2",
        "record pt {\n    x: int = 0;\n    y: int = 0;\n}\n\n$state {\n    p: pt = pt { };\n    xs: list<pt> = [];\n}\n\nfn bump(mut n: int) {\n    n = n + 1;\n}\n\nfn go() {\n    $state.p.x = 3;\n    bump($state.p.y);\n}\n@@@\n@press(label = \"go\", click = go);\n",
    );
    assert_eq!(code, 0, "{}", said);
    assert!(doc.contains("mut x: int = 0;") && doc.contains("mut y: int = 0;"), "a field set, and one given to a `mut` parameter: {}", doc);

    let dir = tree("b2-pack", &[("kit/kit.xtmdpack", "package kit;\n\npack kit@1 {\n    uses core@1;\n    provides pt;\n}\n\nrecord pt {\n    x: int = 0;\n}\n"), ("doc.xtmd", "import kit;\n\n$state {\n    p: pt = pt { };\n}\n\nfn go() {\n    $state.p.x = 3;\n}\n@@@\n@press(label = \"go\", click = go);\n")]);
    let (code, said) = migrate(&dir, &["doc.xtmd", "--pack", "kit/kit.xtmdpack"]);
    assert_eq!(code, 3, "{}", said);
    let doc = read(&dir, "doc.xtmd");
    assert!(doc.contains("// TODO(xtmd migrate): `x` of `kit/pt` is written here, and 0.4.0 writes only a field declared `mut`: it is pack kit's"), "{}", doc);
    assert!(said.contains("it doesn't compile yet, and is written with its TODOs"), "what it doesn't compile with yet is said: {}", said);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a1_a_plain_block_for_a_widget_with_several_slots_fills_its_first_by_name() {
    let (code, said, doc) = one("a1", "@@@\n@frame(title = \"Build\") {\n    It passed.\n}\n");
    assert_eq!(code, 0, "{}", said);
    assert!(doc.contains("@frame(title = \"Build\") {\n    @slot body {\n        It passed.\n    }\n}"), "{}", doc);
}

#[test]
fn a4_an_extern_widget_that_said_no_markdown_writes_none() {
    let dir = tree("a4", &[("kit/kit.xtmdpack", "package kit;\n\nextern widget plot(values: list<number>);\n")]);
    let (code, said) = migrate(&dir, &["kit/kit.xtmdpack"]);
    assert_eq!(code, 0, "{}", said);
    assert!(read(&dir, "kit/kit.xtmdpack").contains("extern widget plot(values: list<number>) {\n    markdown { }\n}"), "{}", read(&dir, "kit/kit.xtmdpack"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a5_b3_a_name_that_is_a_keyword_now_is_renamed_everywhere_and_a_hosts_one_is_said() {
    let (code, said, doc) = one(
        "a5",
        "record r {\n    uses: int = 1;\n}\n\n$state {\n    warn: bool = false;\n}\n\nlet execute: int = 3;\n\nfn provides(migration: int): int {\n    return migration + 1;\n}\n@@@\nA \\{r { uses = 2 }.uses} \\{execute} \\{provides(1)} \\{$state.warn}\n",
    );
    assert_eq!(code, 3, "{}", said);
    for now in ["uses_: int = 1;", "warn_: bool = false;", "let execute_: int = 3;", "fn provides_(migration_: int): int {", "return migration_ + 1;", "r { uses_ = 2 }.uses_", "\\{execute_}", "\\{provides_(1)}", "\\{$state.warn_}"] {
        assert!(doc.contains(now), "{:?} in\n{}", now, doc);
    }
    assert!(doc.contains("// TODO(xtmd migrate): `$state.warn` is `warn_` now"), "a host reads $state by name: {}", doc);
}

#[test]
fn b5_several_widgets_in_a_slot_that_holds_one_is_a_question_and_each_answer_does_what_it_says() {
    let doc = "widget tab(t: text) {\n    view {\n        @label(value = t);\n    }\n    markdown {\n        \\{t}\n    }\n}\n\nwidget one() {\n    slot body: tab;\n    view {\n        @body;\n    }\n    markdown {\n        x\n    }\n}\n\nwidget u() {\n    view {\n        @one {\n            @tab(t = \"a\");\n            @tab(t = \"b\");\n        }\n    }\n    markdown {\n        x\n    }\n}\n@@@\n@u;\n";
    let dir = tree("b5", &[("doc.xtmd", doc)]);
    let (code, said) = migrate(&dir, &["."]);
    assert_eq!(code, 3, "a question waits for a person: {}", said);
    assert!(said.contains("slot-holds-one:doc/one.body:1"), "{}", said);
    assert_eq!(read(&dir, "doc.xtmd"), doc, "nothing is written while it is open");

    std::fs::write(dir.join("answers.toml"), "[steps]\n\"slot-holds-one:doc/one.body:1\" = \"first\"\n").unwrap();
    let (code, said) = migrate(&dir, &[".", "--answers", "answers.toml"]);
    assert_eq!(code, 0, "{}", said);
    let now = read(&dir, "doc.xtmd");
    assert!(now.contains("@tab(t = \"a\");\n\n            // `one`'s slot `body` holds one tab; these were given to it as well:\n            //     @tab(t = \"b\");"), "{}", now);

    std::fs::write(dir.join("doc.xtmd"), doc).unwrap();
    std::fs::write(dir.join("xtmd.toml"), "xtmd = \"0.3.2\"\n").unwrap();
    std::fs::write(dir.join("answers.toml"), "[steps]\n\"slot-holds-one:doc/one.body:1\" = \"list\"\n").unwrap();
    let (code, said) = migrate(&dir, &[".", "--answers", "answers.toml"]);
    assert_eq!(code, 3, "{}", said);
    let now = read(&dir, "doc.xtmd");
    assert!(now.contains("slot body: list<tab>;") && now.contains("// TODO(xtmd migrate): the slot `body` is a list now"), "{}", now);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn b6_a_block_given_to_a_view_held_in_a_value_is_kept_as_a_comment_and_left_for_a_person() {
    let (code, said, doc) = one("b6", "widget a() {\n    view {\n        let v: view = @label(value = \"x\");\n        @v {\n            @label(value = \"lost\");\n        }\n    }\n    markdown {\n        x\n    }\n}\n@@@\n@a;\n");
    assert_eq!(code, 3, "{}", said);
    assert!(doc.contains("// a block given to `v`, which 0.3.2 never drew:\n        //     @label(value = \"lost\");\n        // TODO(xtmd migrate): `v` is a view held in a value"), "{}", doc);
    assert!(doc.contains("        @v;\n"), "{}", doc);
}

#[test]
fn b4_show_of_what_was_made_from_a_text_is_that_text_and_anything_else_is_left_for_a_person() {
    let (code, said, doc) = one("b4", "widget a(t: text) {\n    view {\n        @label(value = rich_text.plain(t).show());\n    }\n    markdown {\n        \\{t}\n    }\n}\n\nfn same(x: rich_text, y: rich_text): bool {\n    return x.show() == y.show();\n}\n@@@\n@a(t = \"x\");\n");
    assert_eq!(code, 3, "{}", said);
    assert!(doc.contains("@label(value = t);"), "{}", doc);
    assert!(doc.contains("// TODO(xtmd migrate): `.show()` is gone in 0.4.0"), "{}", doc);
}

#[test]
fn b7_a_rich_text_in_state_keeps_its_text_and_each_read_makes_it() {
    let (code, said, doc) = one("b7", "$state {\n    note: rich_text = \"a **b**\";\n}\n@@@\n@paragraph(value = $state.note);\n");
    assert_eq!(code, 3, "the host is told: {}", said);
    assert!(doc.contains("note: text = \"a **b**\";"), "{}", doc);
    assert!(doc.contains("@paragraph(value = rich_text.from(template.of($state.note)).or(rich_text.plain($state.note)));"), "{}", doc);
    assert!(doc.contains("// TODO(xtmd migrate): `$state.note` holds a text now"), "{}", doc);
}

#[test]
fn b8_reading_a_rich_texts_parts_is_left_for_a_person() {
    let (code, said, doc) = one("b8", "fn f(x: rich_text): rich_text {\n    return rich_text { segments = [] };\n}\n@@@\nx\n");
    assert_eq!(code, 3, "{}", said);
    assert!(doc.contains("// TODO(xtmd migrate): a rich_text is Markdown drawn where it is placed in 0.4.0, and nothing reads its parts"), "{}", doc);
}

#[test]
fn b9_c1_what_0_3_2_read_as_a_sentence_keeps_its_marks_as_words() {
    let (code, said, doc) = one("b9", "let a: rich_text = \"# not a heading\";\nlet b: rich_text = \"- not a list, **bold**\";\n\n$state {\n    d: text = \"x\";\n}\n@@@\n@paragraph(value = a);\n\n@paragraph(value = b);\n\n@paragraph(value = rich_text.from(template.of($state.d)).or(\"bad\"));\n");
    assert_eq!(code, 3, "{}", said);
    assert!(doc.contains("let a: rich_text = \"\\\\# not a heading\";"), "{}", doc);
    assert!(doc.contains("let b: rich_text = \"\\\\- not a list, **bold**\";"), "{}", doc);
    assert!(doc.contains("// TODO(xtmd migrate): 0.4.0's `rich_text.from` reads blocks too"), "a text read while the page runs: {}", doc);
}
