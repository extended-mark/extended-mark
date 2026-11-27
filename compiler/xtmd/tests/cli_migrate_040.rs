//! `xtmd migrate` from 0.3.2 to 0.4.0, end to end through the step (src/migrate/rir/v1), on trees with a pack:
//! what is moved, and what is written with a TODO and said, with exit 3.

use std::path::{Path, PathBuf};
use std::process::Command;

fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtmd-migrate-040-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&dir);
    for (p, text) in files {
        let p = dir.join(p);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

fn xtmd(dir: &Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_xtmd")).args(args).current_dir(dir).output().unwrap();
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

const TOML: &str = "xtmd = \"0.3.2\"\npacks = [\"kit\"]\n";

const KIT: &str = r#"package kit;

pack kit@1 {
    uses core@1;
    provides card, two, inner_use;
}

widget card(title: text) {
    slot body: view;
    view {
        @column {
            @label(value = title);
            @body;
        }
    }
    markdown {
        **\{title}**

        \{body}
    }
}

widget two(title: text) {
    slot head: view;
    slot foot: opt<view>;
    view {
        @head;
    }
    markdown {
        \{title}

        \{head}
    }
}

widget inner_use() {
    view {
        @card(title = "in the pack");
    }
    markdown {
        x
    }
}
"#;

const DOC: &str = r#"import kit;

widget about(t: text) {
    view {
        @label(value = t);
    }
    markdown {
        \{t}
    }
}

widget box(name: text) {
    slot abouts: list<about>;
    slot extra: view;
    view {
        @label(value = "\{name} \{abouts.len()}");
        for (a in abouts) {
            @a;
        }
        @extra;
    }
    markdown {
        @for (a in abouts) {
            \{a}
        }

        \{extra}
    }
}

widget uses_it() {
    view {
        @box(name = "in code");
        @two(title = "left out in code");
    }
    markdown {
        x
    }
}
@@@
@box(name = "a");

@two(title = "plain") {
    Some **words**.

    - a list
}

@frame(title = "flush") {
Flush text.
}

@two(title = "left out");

@two(title = "half") {
@slot foot {
    Foot.
}
}

@uses_it;
"#;

#[test]
fn a_left_out_slot_is_given_an_empty_block_and_a_plain_block_is_named() {
    let dir = tree("slots", &[("xtmd.toml", TOML), ("kit/kit.xtmdpack", KIT), ("doc.xtmd", DOC)]);
    let (code, before) = xtmd(&dir, &["check", "."]);
    assert_eq!(code, 1, "{before}");
    let (code, out) = xtmd(&dir, &["migrate", ".", "--yes"]);
    assert_eq!(code, 0, "{out}");
    let (code, after) = xtmd(&dir, &["check", "."]);
    assert_eq!(code, 0, "{after}");
    let doc = std::fs::read_to_string(dir.join("doc.xtmd")).unwrap();
    for want in [
        // A1: a slot that may be empty is given an empty block where a call leaves it out; the widget stays
        "    slot abouts: list<about>;\n    slot extra: view;\n",
        "@box(name = \"a\") {\n    @slot abouts {\n    }\n\n    @slot extra {\n    }\n}\n",
        "        @box(name = \"in code\") {\n            abouts { }\n            extra { }\n        }\n",
        "@two(title = \"plain\") {\n    @slot head {\n        Some **words**.\n\n        - a list\n    }\n}\n",
        "@frame(title = \"flush\") {\n    @slot body {\n        Flush text.\n    }\n}\n",
        "@two(title = \"left out\") {\n    @slot head {\n    }\n}\n",
        "        @two(title = \"left out in code\") {\n            head { }\n        }\n",
        "@two(title = \"half\") {\n    @slot head {\n    }\n\n    @slot foot {\n        Foot.\n    }\n}\n",
    ] {
        assert!(doc.contains(want), "{want}\nin\n{doc}");
    }
    let kit = std::fs::read_to_string(dir.join("kit/kit.xtmdpack")).unwrap();
    assert!(kit.contains("    slot body: view;\n") && kit.contains("@card(title = \"in the pack\") {\n            body { }\n        }"), "{kit}");
    assert_eq!(std::fs::read_to_string(dir.join("xtmd.toml")).unwrap(), TOML.replace("0.3.2", env!("CARGO_PKG_VERSION")));
    // what was drawn is what is drawn: nothing given is still nothing
    let (code, md) = xtmd(&dir, &["md", "doc.xtmd"]);
    assert!(code == 0 && md.contains("Some **words**.") && md.contains("Flush text."), "{md}");
    // and a second run changes nothing
    let (_, again) = xtmd(&dir, &["migrate", "."]);
    assert!(again.contains("0 to migrate"), "{again}");
}

const HARD: &str = r#"widget tab(t: text) {
    view {
        @label(value = t);
    }
    markdown {
        \{t}
    }
}

widget tabs() {
    slot panels: list<tab>[1...];
    slot head: tab;
    view {
        @head;
    }
    markdown {
        x
    }
}
@@@
@tabs;
"#;

#[test]
fn a_slot_that_must_hold_something_is_left_for_a_person() {
    let dir = tree("hard", &[("hard.xtmd", HARD)]);
    let (code, out) = xtmd(&dir, &["migrate", "hard.xtmd", "--yes"]);
    // said out loud: no xtmd.toml is the release before, and each TODO, with exit 3
    assert_eq!(code, 3, "{out}");
    assert!(out.contains("the tree is read as written for 0.3.2"), "{out}");
    assert!(out.contains("`tabs` needs its slot `panels`") && out.contains("`tabs` needs its slot `head`"), "{out}");
    let text = std::fs::read_to_string(dir.join("hard.xtmd")).unwrap();
    assert!(text.contains("@// TODO(xtmd migrate): `tabs` needs its slot `panels`"), "{out}\n{text}");
}

#[test]
fn a_tree_already_written_for_this_release_is_left_alone() {
    let dir = tree("now", &[("xtmd.toml", &TOML.replace("0.3.2", env!("CARGO_PKG_VERSION"))), ("kit/kit.xtmdpack", &KIT.replace("slot body: view;", "slot body: opt<view>;")), ("doc.xtmd", "# Hi\n")]);
    let (_, out) = xtmd(&dir, &["migrate", "."]);
    assert!(out.contains("0 to migrate"), "{out}");
}

const WARNED: &str = r#"import kit;

record alert {
    warn: text;
}

$state {
    warn: bool = false;
}

fn warn(warn: alert): text {
    return "\{warn.warn}!";
}

widget tell(warn: text) {
    view {
        @label(value = warn);
    }
    markdown {
        \{warn}, and the word warn stays
    }
}
@@@
I warn you: \{warn(alert { warn = "\{$state.warn}" })}

@tell(warn = "warn in a text stays");

@mark(warn = "kit");
"#;

const PLAIN_EXTERNS: &str = r#"package kit;

pack kit@1 {
    uses core@1;
    provides mark, box, gap;
}

// says nothing as Markdown yet
extern widget mark(warn: text);

extern widget box() {
    slot body: view;      // what it holds
}

extern widget gap() {
}
"#;

#[test]
fn a_name_spelled_warn_is_renamed_and_an_extern_says_what_it_is_as_markdown() {
    let dir = tree("warn", &[("xtmd.toml", TOML), ("kit/kit.xtmdpack", PLAIN_EXTERNS), ("doc.xtmd", WARNED)]);
    assert_eq!(xtmd(&dir, &["check", "."]).0, 1);
    let (code, out) = xtmd(&dir, &["migrate", ".", "--yes"]);
    // `$state.warn` is renamed too, with a TODO: a host reads it by its name
    assert_eq!(code, 3, "{out}");
    assert!(out.contains("`$state.warn` is `warn_` now, as `warn` is a keyword"), "{out}");
    let (code, after) = xtmd(&dir, &["check", "."]);
    assert_eq!(code, 0, "{after}");
    let doc = std::fs::read_to_string(dir.join("doc.xtmd")).unwrap();
    for want in [
        "    warn_: text;\n",
        "    warn_: bool = false;\n",
        "fn warn_(warn_: alert): text {\n    return \"\\{warn_.warn_}!\";\n",
        "\\{warn_}, and the word warn stays",
        "I warn you: \\{warn_(alert { warn_ = \"\\{$state.warn_}\" })}",
        "@tell(warn_ = \"warn in a text stays\");",
        "@mark(warn_ = \"kit\");",
    ] {
        assert!(doc.contains(want), "{want}\nin\n{doc}");
    }
    let kit = std::fs::read_to_string(dir.join("kit/kit.xtmdpack")).unwrap();
    for want in [
        "// says nothing as Markdown yet\nextern widget mark(warn_: text) {\n    markdown { }\n}\n",
        "    slot body: view; // what it holds\n\n    markdown { }\n}\n",
        "extern widget gap() {\n    markdown { }\n}\n",
    ] {
        assert!(kit.contains(want), "{want}\nin\n{kit}");
    }
    let (_, again) = xtmd(&dir, &["migrate", "."]);
    assert!(again.contains("0 to migrate"), "{again}");
}

const MD: &str = r##"record note {
    body: md;
    all: list<md>;
}

$state {
    draft: text = "a **b**";
}

fn read(t: text): opt<md> {
    return md.from(template.of(t));
}

let intro: md = "# Hi\n\nwords, (and more";

widget box(body: md) {
    view {
        @blocks(value = body);
        @blocks(value = read($state.draft).or("**bad**"));
    }
    markdown {
        @blocks(value = body);
    }
}
@@@
@box(body = "a");

The md in prose stays, and @blocks too.

@blocks(value = intro);

@blocks(value = md.from(template.of($state.draft)).or("**bad**"));
"##;

#[test]
fn md_is_rich_text_and_is_placed() {
    let dir = tree("md", &[("xtmd.toml", "xtmd = \"0.3.2\"\n"), ("doc.xtmd", MD)]);
    let (code, out) = xtmd(&dir, &["migrate", ".", "--yes"]);
    assert_eq!(code, 0, "{out}");
    let (code, after) = xtmd(&dir, &["check", "."]);
    assert_eq!(code, 0, "{after}");
    let doc = std::fs::read_to_string(dir.join("doc.xtmd")).unwrap();
    for want in [
        "    body: rich_text;\n    all: list<rich_text>;\n",
        "fn read(t: text): opt<rich_text> {\n    return rich_text.from(template.of(t));\n}",
        "let intro: rich_text = \"# Hi\\n\\nwords, (and more\";",
        "        @body;\n        let held_2: rich_text = read($state.draft).or(\"**bad**\");\n        @held_2;\n",
        "    markdown {\n        \\{body}\n    }",
        "The md in prose stays, and @blocks too.",
        "\n\\{intro}\n\n\\{rich_text.from(template.of($state.draft)).or(\"**bad**\")}\n",
    ] {
        assert!(doc.contains(want), "{want}\nin\n{doc}");
    }
    let (_, again) = xtmd(&dir, &["migrate", "."]);
    assert!(again.contains("0 to migrate"), "{again}");
}

#[test]
fn what_reads_a_rich_texts_parts_is_left_for_a_person() {
    let doc = "fn first(a: rich_text): int {\n    return a.segments.len();\n}\n\nfn same_words(a: rich_text, b: rich_text): bool {\n    return a == b;\n}\n@@@\nhi\n";
    let dir = tree("parts", &[("xtmd.toml", "xtmd = \"0.3.2\"\n"), ("doc.xtmd", doc)]);
    let (code, out) = xtmd(&dir, &["migrate", ".", "--yes"]);
    assert_eq!(code, 3, "{out}");
    assert!(out.contains("nothing reads its parts") && out.contains("a rich_text is drawn in 0.4.0, and two aren't compared"), "{out}");
    let text = std::fs::read_to_string(dir.join("doc.xtmd")).unwrap();
    assert!(text.contains("    // TODO(xtmd migrate): a rich_text is drawn in 0.4.0, and two aren't compared: compare the texts they are made from\n    return a == b;"), "{text}");
}
