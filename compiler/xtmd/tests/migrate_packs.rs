//! What a pack's migration says is what each use becomes (page 6), applied to the resolved IR and
//! printed in `xtmd fmt`'s layout: values worked out where the use gives constants, conditions decided
//! or asked, the migration's own fns run, names that mean what the migration means by them, and
//! nothing left out without a word. Each of #14269, #14270 and #14273 to #14276 has its case here.

use std::path::PathBuf;
use std::process::Command;

/// A pack `kit` at its second version, a document written for the first, and what moves it.
struct Kit<'a> {
    old: &'a str,
    provides: &'a str,
    new: &'a str,
    /// What is above the migration in its file, and the migration's own body.
    beside: &'a str,
    migration: &'a str,
    doc: &'a str,
    other: Option<&'a str>,
    answers: Option<&'a str>,
}

const BAR: Kit = Kit {
    old: "extern widget bar(value: number, label: text = \"x\");\n",
    provides: "bar",
    new: "extern widget bar(percent: number, label: text = \"x\", big: bool = false) {\n    markdown { }\n}\n",
    beside: "",
    migration: "",
    doc: "",
    other: None,
    answers: None,
};

/// A value that becomes a percentage.
const PERCENT: &str = "    widget bar {\n        add(input = percent, value = (value * 100).round(), why = \"a share of 100\");\n        drop(input = value, why = \"a share of 100\");\n    }";

struct Ran {
    /// `xtmd check` of the migration's file.
    check: (i32, String),
    code: i32,
    said: String,
    doc: String,
    dir: PathBuf,
}

fn run(name: &str, k: &Kit, flags: &[&str]) -> Ran {
    let dir = std::env::temp_dir().join(format!("xtmd-packs-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let write = |p: &str, t: &str| {
        let p = dir.join(p);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, t).unwrap();
    };
    let (imports, uses) = if k.other.is_some() { ("import other;\n\n", ", other@1") } else { ("", "") };
    write("kit/surfaces/kit@1.xtmdpack", &format!("package kit@1;\n\n{}", k.old));
    write("kit/kit.xtmdpack", &format!("package kit;\n\n{}pack kit@2 {{\n    uses core@2{};\n    provides {};\n}}\n\n{}", imports, uses, k.provides, k.new));
    write("kit/migrations/kit@2.xtmdpack", &format!("package kit;\n\n{}{}migration kit@1 to kit@2 {{\n{}\n}}\n", imports, k.beside, k.migration));
    write("xtmd.toml", &format!("packs = [\"kit\"{}]\nxtmd = \"{}\"\n\n[uses]\nkit = 1\n", if k.other.is_some() { ", \"other\"" } else { "" }, env!("CARGO_PKG_VERSION")));
    write("docs/a.xtmd", k.doc);
    if let Some(o) = k.other {
        write("other/other.xtmdpack", o);
    }
    let xtmd = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_xtmd")).args(args).current_dir(&dir).output().unwrap();
        (out.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
    };
    let check = xtmd(&["check", "--pack", if k.other.is_some() { "kit,other" } else { "kit" }, "kit/migrations/kit@2.xtmdpack"]);
    let mut args = vec!["migrate", "docs", "--yes", "--allow-dirty"];
    if let Some(a) = k.answers {
        write("answers.toml", a);
        args.extend(["--answers", "answers.toml"]);
    }
    args.extend(flags);
    let (code, said) = xtmd(&args);
    Ran { check, code, said, doc: std::fs::read_to_string(dir.join("docs/a.xtmd")).unwrap(), dir }
}

/// The document as the migration leaves it, after a clean check.
fn moved(name: &str, k: &Kit) -> String {
    let r = run(name, k, &[]);
    assert_eq!(r.check.0, 0, "{}: {}", name, r.check.1);
    assert_eq!(r.code, 0, "{}: {}", name, r.said);
    let _ = std::fs::remove_dir_all(&r.dir);
    r.doc
}

/// Why the document is left as it was.
fn left(name: &str, k: &Kit) -> String {
    let r = run(name, k, &[]);
    assert_eq!(r.check.0, 0, "{}: {}", name, r.check.1);
    assert_eq!(r.code, 1, "{}: {}", name, r.said);
    assert_eq!(r.doc, k.doc, "{}: left as it was", name);
    let _ = std::fs::remove_dir_all(&r.dir);
    r.said
}

fn doc(body: &str) -> String {
    format!("import kit;\n@@@\n{}\n", body)
}

fn bar(block: &str) -> String {
    format!("    widget bar {{\n{}\n    }}", block)
}

const RENAME: &str = "        rename(input = value, to = percent);";

#[test]
fn a_value_is_worked_out_where_the_use_gives_a_constant() {
    let d = moved("constant", &Kit { migration: PERCENT, doc: &doc("@bar(value = 0.4);\n\n@bar(value = 1+2);\n\n@bar(value = (1) + (2));\n\n@bar(value = -1);\n\n@bar(value = true ? 1 : 2);"), ..BAR });
    assert_eq!(d, doc("@bar(percent = 40);\n\n@bar(percent = 300);\n\n@bar(percent = 300);\n\n@bar(percent = -100);\n\n@bar(percent = 100);"));
}

#[test]
fn anything_else_passes_through_as_the_code_it_is_in_brackets_where_it_needs_them() {
    let top = "import kit;\n\n$state {\n    n: number = 0.4;\n}\n\nlet limit: number = 5;\n@@@\n";
    let d = moved("code", &Kit { migration: PERCENT, doc: &format!("{top}@bar(value = $state.n);\n\n@bar(value = $state.n+1);\n\n@bar(value = limit);\n"), ..BAR });
    assert_eq!(d, format!("{top}@bar(percent = ($state.n * 100).round());\n\n@bar(percent = (($state.n + 1) * 100).round());\n\n@bar(percent = (limit * 100).round());\n"));
}

#[test]
fn a_list_of_records_is_written_without_the_fields_at_their_defaults() {
    let k = Kit {
        old: "extern widget bar(values: list<number>, factor: number);\n",
        provides: "bar, dot",
        new: "record dot {\n    x: number = 0;\n    y: number;\n}\n\nextern widget bar(values: list<dot>, factor: number) {\n    markdown { }\n}\n",
        migration: &bar("        set(input = values, value = values.map(fn(v) { return dot { y = v * factor }; }), why = \"w\");"),
        // the document's own `factor` isn't the use's
        doc: "import kit;\n\nlet factor: number = 100;\n\n$state {\n    xs: list<number> = [1];\n}\n@@@\n@bar(values = [1, 2], factor = 2);\n\n@bar(values = $state.xs, factor = 3);\n",
        ..BAR
    };
    let d = moved("list", &k);
    assert!(d.ends_with("@bar(values = [dot { y = 2 }, dot { y = 4 }], factor = 2);\n\n@bar(values = $state.xs.map(fn(v) { return dot { y = v * 3 }; }), factor = 3);\n"), "{d}");
}

#[test]
fn a_migrations_own_fn_runs_in_its_block_or_beside_it() {
    let f = "fn pct(v: number): number {\n    return (v * 100).round();\n}\n\n";
    let uses = "        add(input = percent, value = pct(value), why = \"w\");\n        drop(input = value, why = \"w\");\n        example(before = @bar(value = 0.4), after = @bar(percent = 40));";
    let d = "import kit;\n\nlet limit: number = 5;\n@@@\n@bar(value = 0.4);\n\n@bar(value = limit);\n";
    let want = "import kit;\n\nlet limit: number = 5;\n@@@\n@bar(percent = 40);\n\n@bar(percent = 500);\n";
    assert_eq!(moved("beside", &Kit { beside: f, migration: &bar(uses), doc: d, ..BAR }), want);
    let inside = format!("    fn pct(v: number): number {{\n        return (v * 100).round();\n    }}\n\n{}", bar(uses));
    assert_eq!(moved("inside", &Kit { migration: &inside, doc: d, ..BAR }), want);
    // a variable can't be written in
    let said = left("own-variable", &Kit { migration: &inside, doc: "import kit;\n\n$state {\n    n: number = 0.4;\n}\n@@@\n@bar(value = $state.n);\n", ..BAR });
    assert!(said.contains("kit@2 works this out with `pct`, which only runs when migrating"), "{said}");
    // unless the fn calls one the newer version provides
    let scale = format!("{}\nfn scale(v: number): number {{\n    return v * 100;\n}}\n", BAR.new);
    let inside = inside.replace("return (v * 100).round();", "return scale(v);");
    let d = moved("own-provided", &Kit { provides: "bar, scale", new: &scale, migration: &inside, doc: "import kit;\n\n$state {\n    n: number = 0.4;\n}\n@@@\n@bar(value = $state.n);\n\n@bar(value = 2);\n", ..BAR });
    assert!(d.ends_with("@bar(percent = scale($state.n));\n\n@bar(percent = 200);\n"), "{d}");
}

#[test]
fn a_migrations_own_fns_are_named_in_its_file_only() {
    let own = |name: &str, on: &str| format!("    fn {on}{name}(v: number): number {{\n        return v;\n    }}\n\n{}", bar(RENAME));
    let r = run("own-twice", &Kit { beside: "fn pct(v: number): number {\n    return v;\n}\n\n", migration: &own("pct", ""), doc: &doc("@bar(value = 1);"), ..BAR }, &[]);
    assert!(r.check.1.contains("`pct` is declared twice"), "{}", r.check.1);
    let r = run("own-on", &Kit { migration: "    fn number.pct(self): number {\n        return self;\n    }", doc: &doc("@bar(value = 1);"), ..BAR }, &[]);
    assert!(r.check.1.contains("a migration's own fn is called by its name"), "{}", r.check.1);
    // a document never sees one
    let d = "import kit;\n\nlet a: number = pct(1);\n@@@\n\\{a}\n";
    let r = run("own-unseen", &Kit { migration: &own("pct", ""), doc: d, ..BAR }, &[]);
    assert!(r.said.contains("doesn't compile against kit@1"), "{}", r.said);
}

#[test]
fn a_condition_over_constants_is_decided_by_their_values() {
    let m = bar(&format!("        set(input = big, value = true, when = value > 3, why = \"w\");\n{RENAME}"));
    assert_eq!(moved("when", &Kit { migration: &m, doc: &doc("@bar(value = 5);\n\n@bar(value = 1);"), ..BAR }), doc("@bar(percent = 5, big = true);\n\n@bar(percent = 1);"));
    // `1.0` is `1`, however it is written
    let m = bar(&format!("        set(input = big, value = true, when = value == 1, why = \"w\");\n{RENAME}"));
    assert_eq!(moved("when-same", &Kit { migration: &m, doc: &doc("@bar(value = 1.0);"), ..BAR }), doc("@bar(percent = 1.0, big = true);"));
}

#[test]
fn a_condition_the_use_doesnt_say_is_put_to_the_person() {
    let m = bar(&format!("        set(input = big, value = true, when = value > 3, why = \"w\");\n{RENAME}"));
    let d = "import kit;\n\n$state {\n    n: number = 0.4;\n}\n@@@\n@bar(value = $state.n);\n";
    let k = Kit { migration: &m, doc: d, ..BAR };
    let said = left("asked", &k);
    assert!(said.contains("does `value > 3` hold, which is `$state.n > 3` here") && said.contains("[split] \"docs/a.xtmd:7 bar: value > 3\" = \"yes\""), "{said}");
    let d = moved("answered", &Kit { answers: Some("[split]\n\"docs/a.xtmd:7 bar: value > 3\" = \"yes\"\n"), ..k });
    assert!(d.ends_with("@bar(percent = $state.n, big = true);\n"), "{d}");
}

#[test]
fn given_ask_and_confirm_are_answered_inside_a_value() {
    let m = bar(&format!("{RENAME}\n        set(input = big, value = given(label), why = \"w\");"));
    assert_eq!(moved("given", &Kit { migration: &m, doc: &doc("@bar(value = 1, label = \"a\");\n\n@bar(value = 2);"), ..BAR }), doc("@bar(percent = 1, label = \"a\", big = true);\n\n@bar(percent = 2, big = false);"));
    let m = bar(&format!("{RENAME}\n        set(input = label, value = ask(why = \"which\", options = [\"a\", label]), why = \"w\");"));
    let k = Kit { migration: &m, doc: &doc("@bar(value = 1, label = \"mine\");"), ..BAR };
    let said = left("ask", &k);
    assert!(said.contains("1 question no one has answered") && said.contains("\"docs/a.xtmd:3 bar: which\""), "{said}");
    assert_eq!(moved("ask-answered", &Kit { answers: Some("[split]\n\"docs/a.xtmd:3 bar: which\" = '\"mine\"'\n"), ..k }), doc("@bar(percent = 1, label = \"mine\");"));
    let m = bar(&format!("{RENAME}\n        set(input = big, value = confirm(why = \"sure?\"), why = \"w\");"));
    assert_eq!(moved("confirm", &Kit { migration: &m, doc: &doc("@bar(value = 1);"), answers: Some("[confirm]\n\"sure?\" = true\n"), ..BAR }), doc("@bar(percent = 1, big = true);"));
}

#[test]
fn a_migrations_names_are_the_uses_values_and_never_the_documents() {
    // a hole, a default after a rename, and a variable of an execute's, each with a name the document has
    let m = bar(&format!("{RENAME}\n        set(input = label, value = \"v: \\{{label}}\", why = \"w\");"));
    let d = moved("hole", &Kit { migration: &m, doc: "import kit;\n\nlet label: text = \"the document's own\";\n@@@\n@bar(value = 1, label = \"mine\");\n", ..BAR });
    assert!(d.ends_with("@bar(percent = 1, label = \"v: mine\");\n"), "{d}");
    let k = Kit {
        new: "extern widget bar(value: number, caption: text = \"x\", big: text = \"\") {\n    markdown { }\n}\n",
        migration: &bar("        rename(input = label, to = caption);\n        set(input = big, value = caption, why = \"w\");"),
        doc: "import kit;\n\nlet caption: text = \"the document's own\";\n@@@\n@bar(value = 1);\n",
        ..BAR
    };
    let d = moved("default", &k);
    assert!(d.ends_with("@bar(value = 1, big = \"x\");\n"), "{d}");
    let m = bar("        execute {\n            mut n: number = value;\n            for (k in [1, 2]) {\n                n = n + k;\n            }\n            return @bar(percent = n);\n        }");
    let d = moved("mut", &Kit { migration: &m, doc: "import kit;\n\nlet n: number = 99;\n@@@\n@bar(value = 1);\n", ..BAR });
    assert!(d.ends_with("@bar(percent = 4);\n"), "{d}");
}

#[test]
fn an_execute_runs_over_constants_and_says_what_it_cant_decide() {
    let m = bar("        execute {\n            if (value * 2 == 10) {\n                return @bar(percent = value, big = true);\n            }\n            return @bar(percent = value);\n        }");
    assert_eq!(moved("execute-if", &Kit { migration: &m, doc: &doc("@bar(value = 5);\n\n@bar(value = 6);"), ..BAR }), doc("@bar(percent = 5, big = true);\n\n@bar(percent = 6);"));
    let m = bar("        execute {\n            match (value) {\n                case 1 {\n                    return @bar(percent = 10);\n                }\n                case else {\n                    return;\n                }\n            }\n        }");
    let k = Kit { migration: &m, doc: &doc("@bar(value = 1.0);\n\n@bar(value = 6);"), ..BAR };
    assert_eq!(moved("execute-match", &k), doc("@bar(percent = 10);\n@// kit@2 dropped bar\n@// @bar(value = 6);"));
    let said = left("execute-asks", &Kit { doc: "import kit;\n\n$state {\n    n: number = 1;\n}\n@@@\n@bar(value = $state.n);\n", ..k });
    assert!(said.contains("which case is `value`, which is `$state.n` here (1 or else)"), "{said}");
    let m = bar("        execute {\n            mut n: number = value;\n            n = n + 1;\n            return @bar(percent = n);\n        }");
    let said = left("execute-cant", &Kit { migration: &m, doc: "import kit;\n\n$state {\n    n: number = 1;\n}\n@@@\n@bar(value = $state.n);\n", ..BAR });
    assert!(said.contains("kit@2's execute for bar can't decide this use"), "{said}");
}

#[test]
fn a_migration_fns_if_for_and_execute_are_run() {
    let f = "migration fn fix(to: number) {\n    for (x in [true]) {\n        set(input = big, value = x, why = \"w\");\n    }\n    if (given(label)) {\n        set(input = label, value = \"changed\", why = \"w\");\n    } else if (value > to) {\n        set(input = label, value = \"large\", why = \"w\");\n    } else {\n        set(input = label, value = \"small\", why = \"w\");\n    }\n}\n\n";
    let m = bar(&format!("        fix(to = 3);\n{RENAME}"));
    let d = moved("fn-if", &Kit { beside: f, migration: &m, doc: &doc("@bar(value = 1, label = \"a\");\n\n@bar(value = 5);\n\n@bar(value = 2);"), ..BAR });
    assert_eq!(d, doc("@bar(percent = 1, label = \"changed\", big = true);\n\n@bar(percent = 5, big = true, label = \"large\");\n\n@bar(percent = 2, big = true, label = \"small\");"));
    let f = "migration fn scaled(by: number) {\n    execute {\n        return @bar(percent = value * by);\n    }\n}\n\n";
    assert_eq!(moved("fn-execute", &Kit { beside: f, migration: &bar("        scaled(by = 100);"), doc: &doc("@bar(value = 0.4);"), ..BAR }), doc("@bar(percent = 40);"));
    let f = "migration fn no() {\n    let x: number = 1;\n}\n\n";
    let r = run("fn-let", &Kit { beside: f, migration: &bar(&format!("        no();\n{RENAME}")), doc: &doc("@bar(value = 1);"), ..BAR }, &[]);
    assert!(r.check.1.contains("a migration fn holds statements, `for`, `if`, `execute` and other migration fns"), "{}", r.check.1);
}

const CARD: &str = "extern widget card(title: text) {\n    slot body: view;\n}\n";
const CARD_2: &str = "extern widget card(title: text) {\n    slot body: view;\n\n    markdown { }\n}\n";

#[test]
fn a_name_a_migration_writes_is_imported_and_never_means_another_packs() {
    let old = format!("{CARD}extern widget quote() {{\n    slot body: view;\n}}\n");
    let new = format!("{CARD_2}\nextern widget quote() {{\n    slot body: view;\n\n    markdown {{ }}\n}}\n");
    let k = Kit { old: &old, provides: "card, quote", new: &new, migration: "    widget card {\n        wrap(parent = quote, why = \"w\");\n    }", ..BAR };
    // core has a quote too
    let d = moved("import", &Kit { doc: "---\ntitle: x\n---\nimport kit.card;\n@@@\n@card(title = \"A\") {\nplain\n}\n", ..k });
    assert_eq!(d, "---\ntitle: x\n---\nimport kit.card;\nimport kit.quote;\n@@@\n@quote {\n    @card(title = \"A\") {\n        plain\n    }\n}\n");
    let said = left("import-core", &Kit { doc: "import kit.card;\n@@@\n@card(title = \"A\") {\nplain\n}\n\n@quote {\ncore's\n}\n", ..k });
    assert!(said.contains("it uses core's `quote`, and once migrated `quote` would be kit's"), "{said}");
    let said = left("import-own", &Kit { doc: "import kit;\n\nwidget quote() {\n    view {\n        @label(value = \"mine\");\n    }\n    markdown {\n        mine\n    }\n}\n@@@\n@card(title = \"A\") {\nplain\n}\n", ..k });
    assert!(said.contains("in this file `quote` is the document's own"), "{said}");
    // a type that a value has, worked out or written
    let k = Kit {
        old: "extern widget bar(value: number);\n",
        provides: "bar, tone",
        new: "enum tone { A, B }\n\nextern widget bar(value: number, t: tone = tone.A) {\n    markdown { }\n}\n",
        migration: "    fn pick(n: number): tone {\n        return n > 1 ? tone.B : tone.A;\n    }\n\n    widget bar {\n        add(input = t, value = pick(value), why = \"w\");\n    }",
        doc: "import kit.bar;\n@@@\n@bar(value = 3);\n",
        ..BAR
    };
    assert_eq!(moved("import-type", &k), "import kit.bar;\nimport kit.tone;\n@@@\n@bar(value = 3, t = tone.B);\n");
}

#[test]
fn a_use_becomes_one_of_another_packs() {
    let other = "package other;\n\npack other@1 {\n    provides panel;\n}\n\nextern widget panel(value: number, of: text = \"\") {\n    markdown { }\n}\n";
    let k = Kit {
        old: "extern widget bar(value: number);\n",
        provides: "holder",
        new: "extern widget holder(value: number) {\n    markdown { }\n}\n",
        migration: &bar("        rename(to = panel);\n        set(input = of, value = \"x\", why = \"w\");\n        example(before = @bar(value = 1), after = @panel(value = 1, of = \"x\"));"),
        other: Some(other),
        ..BAR
    };
    assert_eq!(moved("other", &Kit { doc: &doc("@bar(value = 1);"), ..k }), "import kit;\nimport other;\n@@@\n@panel(value = 1, of = \"x\");\n");
    assert_eq!(moved("other-one", &Kit { doc: "import kit.bar as b;\n@@@\n@b(value = 1);\n", ..k }), "import other.panel as b;\n@@@\n@b(value = 1, of = \"x\");\n");
    assert_eq!(moved("other-single", &Kit { doc: "import kit.bar;\n@@@\n@bar(value = 1);\n", ..k }), "import other.panel;\n@@@\n@panel(value = 1, of = \"x\");\n");
}

#[test]
fn a_fns_arguments_are_added_dropped_and_given_a_value_in_their_places() {
    let w = "extern widget bar(value: text);\n";
    let w2 = "extern widget bar(value: text) {\n    markdown { }\n}\n";
    let case = |name: &str, old: &str, new: &str, block: &str, before: &str| {
        let (old, new) = (format!("{old}{w}"), format!("{new}\n{w2}"));
        let provides = if old.starts_with("extern fn text.") { "bar" } else { "bar, pad" };
        // an extern fn's value isn't a constant: it is read in a fn
        let d = format!("import kit;\n\nfn a(): text {{\n    return {before};\n}}\n@@@\n\\{{a()}}\n");
        let k = Kit { old: &old, provides, new: &new, migration: block, doc: &d, ..BAR };
        let d = moved(name, &k);
        d.split_once("return ").unwrap().1.split_once(";\n").unwrap().0.to_string()
    };
    // page 6's own
    let block = "    fn text.slug {\n        rename(to = slugify);\n        add(arg = sep, value = \"-\", why = \"w\");\n    }";
    assert_eq!(case("fn-spec", "extern fn text.slug(self): text;\n", "extern fn text.slugify(self, sep: text): text;\n", block, "\"a b\".slug().slug()"), "\"a b\".slugify(\"-\").slugify(\"-\")");
    let block = "    fn text.pad {\n        add(arg = fill, value = \"-\", why = \"w\");\n    }";
    assert_eq!(case("fn-first", "extern fn text.pad(self, n: number): text;\n", "extern fn text.pad(self, fill: text, n: number): text;\n", block, "\"a\".pad(3)"), "\"a\".pad(\"-\", 3)");
    let block = "    fn text.pad {\n        drop(arg = fill, why = \"w\");\n    }";
    assert_eq!(case("fn-drop", "extern fn text.pad(self, fill: text, n: number): text;\n", "extern fn text.pad(self, n: number): text;\n", block, "\"a\".pad(\"-\", 3)"), "\"a\".pad(3)");
    let block = "    fn pad {\n        add(arg = w, value = \"\\{n} px\", why = \"w\");\n        drop(arg = n, why = \"w\");\n    }";
    assert_eq!(case("fn-replace", "extern fn pad(t: text, n: number): text;\n", "extern fn pad(t: text, w: text): text;\n", block, "pad(\"a\", 3)"), "pad(\"a\", \"3 px\")");
    let block = "    fn pad {\n        set(arg = n, value = n * 2, why = \"w\");\n    }";
    assert_eq!(case("fn-order", "extern fn pad(t: text, n: number): text;\n", "extern fn pad(n: number, t: text): text;\n", block, "pad(\"a\", 1 + 2)"), "pad(6, \"a\")");
}

#[test]
fn a_fns_arguments_are_checked() {
    let w = "extern widget bar(value: text);\n";
    let w2 = "extern widget bar(value: text) {\n    markdown { }\n}\n";
    let check = |name: &str, new: &str, block: &str| {
        let (old, new) = (format!("extern fn pad(t: text): text;\n{w}"), format!("{new}\n{w2}"));
        let r = run(name, &Kit { old: &old, provides: "bar, pad", new: &new, migration: block, doc: "import kit;\n@@@\nx\n", ..BAR }, &[]);
        assert_eq!(r.check.0, 1, "{}", r.check.1);
        r.check.1
    };
    let two = "extern fn pad(t: text, n: number): text;\n";
    assert!(check("arg-type", two, "    fn pad {\n        add(arg = n, value = \"x\", why = \"w\");\n    }").contains("expected number, found text"));
    assert!(check("arg-name", two, "    fn pad {\n        add(arg = size, value = 1, why = \"w\");\n    }").contains("`pad` has no argument `size`; it has `t`, `n`"));
    assert!(check("arg-gone", two, "    fn pad {\n        drop(arg = size, why = \"w\");\n    }").contains("`pad` has no argument `size` at this point; it has `t`"));
    assert!(check("arg-unsaid", "extern fn pad(t: number, n: number): text;\n", "    fn pad {\n        add(arg = n, value = 1, why = \"w\");\n    }").contains("`pad` was `(text, number): text` and is now `(number, number): text`"));
    assert!(check("arg-widget", two, "    widget bar {\n        drop(arg = value, why = \"w\");\n    }").contains("`bar` isn't a fn, so it has no arguments"));
}

#[test]
fn a_fn_or_a_let_is_renamed_where_it_is_imported_and_given_as_a_value() {
    let old = "let unit: number = 2;\n\nextern fn pad(t: text): text;\n";
    let new = "let measure: number = 2;\n\nextern fn fit(t: text): text;\n";
    let m = "    fn pad {\n        rename(to = fit);\n    }\n\n    let unit {\n        rename(to = measure);\n    }";
    let d = moved("fn-import", &Kit { old, provides: "fit, measure", new, migration: m, doc: "import kit.pad;\nimport kit.unit;\n\nfn xs(): list<text> {\n    return [\"b\"].map(pad);\n}\n@@@\n\\{xs().len() * unit} \\{pad(\"a\")}\n", ..BAR });
    assert_eq!(d, "import kit.fit;\nimport kit.measure;\n\nfn xs(): list<text> {\n    return [\"b\"].map(fit);\n}\n@@@\n\\{xs().len() * measure} \\{fit(\"a\")}\n");
    // an argument more can't be given where the fn is a value
    let new = "let unit: number = 2;\n\nextern fn pad(t: text, n: number): text;\n";
    let m = "    fn pad {\n        add(arg = n, value = 2, why = \"w\");\n    }";
    let said = left("fn-value", &Kit { old, provides: "pad, unit", new, migration: m, doc: "import kit;\n\nfn xs(): list<text> {\n    return [\"b\"].map(pad);\n}\n@@@\n\\{xs().len()}\n", ..BAR });
    assert!(said.contains("kit@2 changes pad's arguments, and here it is given as a value"), "{said}");
}

#[test]
fn a_slot_moves_into_a_text_where_it_holds_plain_text() {
    let new = "extern widget card(title: text, text: text = \"none\") {\n    markdown { }\n}\n";
    let k = Kit { old: CARD, provides: "card", new, migration: "    widget card {\n        move(from = body, to = text, why = \"w\");\n    }", ..BAR };
    let of = |name: &str, body: &str| {
        let d = doc(&format!("@card(title = \"A\") {{\n{body}\n}}"));
        (moved(name, &Kit { doc: &d, ..k }), doc(""))
    };
    assert_eq!(of("slot-plain", "plain \"words\"\nover two").0, doc("@card(title = \"A\", text = \"plain \\\"words\\\" over two\");"));
    assert_eq!(of("slot-hole", "has \\{1 + 2} holes").0, doc("@card(title = \"A\", text = \"has \\{1 + 2} holes\");"));
    for (n, body) in ["*marked* words", "a\n\nb", "- a", "1. a", "@label(value = \"x\");", "a \\* b", "[a](b)"].iter().enumerate() {
        let d = doc(&format!("@card(title = \"A\") {{\n{body}\n}}"));
        let said = left(&format!("slot-more-{n}"), &Kit { doc: &d, ..k });
        assert!(said.contains("this use fills it with more than plain text"), "{body}: {said}");
    }
}

#[test]
fn a_slot_moves_to_another_slot_where_it_is_written_and_what_a_block_holds_beside_it_stays() {
    let old = "extern widget two(title: text) {\n    slot a: view;\n    slot b: view;\n}\n";
    let new = "extern widget two(title: text) {\n    slot left: view;\n    slot b: view;\n\n    markdown { }\n}\n";
    let k = Kit { old, provides: "two", new, migration: "    widget two {\n        move(from = a, to = left, why = \"w\");\n    }", ..BAR };
    let d = moved("slots", &Kit { doc: &doc("@two(title = \"A\") {\n@slot a {\none\n}\n@slot b {\ntwo\n}\n}"), ..k });
    assert_eq!(d, doc("@two(title = \"A\") {\n    @slot left {\n        one\n    }\n\n    @slot b {\n        two\n    }\n}"));
    let code = |block: &str| format!("import kit;\n\nwidget w() {{\n    view {{\n        @two(title = \"A\") {{ {block} }}\n    }}\n    markdown {{ }}\n}}\n@@@\n@w;\n");
    let d = moved("slots-code", &Kit { doc: &code("a { @label(value = \"x\"); } b { @label(value = \"y\"); }"), ..k });
    let laid = "import kit;\n\nwidget w() {\n    view {\n        @two(title = \"A\") {\n            left {\n                @label(value = \"x\");\n            }\n            b {\n                @label(value = \"y\");\n            }\n        }\n    }\n    markdown { }\n}\n@@@\n@w;\n";
    assert_eq!(d, laid);
    // into a widget that had one slot and has two
    let new = "extern widget card() {\n    slot body: view;\n    slot head: view;\n\n    markdown { }\n}\n";
    let d = moved("slots-second", &Kit { old: CARD, provides: "card", new, migration: "    widget card {\n        move(from = title, to = head, why = \"w\");\n    }", doc: &doc("@card(title = \"A\") {\nplain\n}"), ..BAR });
    assert_eq!(d, doc("@card {\n    @slot body {\n        plain\n    }\n\n    @slot head {\n        A\n    }\n}"));
}

#[test]
fn an_execute_places_a_uses_block_or_the_use_is_left() {
    let new = "extern widget card(head: text) {\n    slot body: opt<view>;\n\n    markdown { }\n}\nextern widget row(v: view) {\n    markdown { }\n}\n";
    let old = format!("{CARD}extern widget row(v: view);\n");
    let k = Kit { old: &old, provides: "card, row", new, ..BAR };
    let lost = "    widget card {\n        execute {\n            return @card(head = title);\n        }\n    }";
    for (n, body) in ["@card(title = \"A\") {\nplain\n}", "@row(v = @card(title = \"A\") { @label(value = \"kept\"); });"].iter().enumerate() {
        let said = left(&format!("block-{n}"), &Kit { migration: lost, doc: &doc(body), ..k });
        assert!(said.contains("kit@2's execute for card leaves out this use's block"), "{said}");
    }
    let places = "    widget card {\n        execute {\n            return @card(head = title) {\n                @body;\n            };\n        }\n    }";
    assert_eq!(moved("block-placed", &Kit { migration: places, doc: &doc("@card(title = \"A\") {\nplain\n}"), ..k }), doc("@card(head = \"A\") {\n    plain\n}"));
}

#[test]
fn a_dropped_cases_uses_are_left_to_the_person_with_the_reason() {
    let k = Kit {
        old: "enum state { A, B }\n\nextern widget bar(s: state);\n",
        provides: "bar, state",
        new: "enum state { A }\n\nextern widget bar(s: state) {\n    markdown { }\n}\n",
        migration: "    enum state {\n        drop(from = state.B, why = \"nothing is B now\");\n    }",
        doc: &doc("@bar(s = state.B);\n\n@bar(s = state.A);"),
        ..BAR
    };
    assert!(left("case", &k).contains("kit@2 dropped B: nothing is B now: write what it is now"));
    let r = run("case-todo", &k, &["--allow-todos"]);
    assert_eq!(r.code, 3, "{}", r.said);
    assert_eq!(r.doc, doc("@// TODO(xtmd migrate): kit@2 dropped B: nothing is B now: write what it is now\n@bar(s = state.B);\n\n@bar(s = state.A);"));
    // and it says nothing of another that is gone
    let r = run("case-unsaid", &Kit { old: "enum state { A, B, C }\n\nextern widget bar(s: state);\n", ..k }, &[]);
    assert!(r.check.1.contains("`state` has no case `C` now"), "{}", r.check.1);
}