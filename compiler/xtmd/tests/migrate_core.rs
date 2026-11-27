//! core is a pack like any other: 0.4.0's core is core@2, core@1's surface is what 0.3.2's core
//! provided (written from the copy of it in compiler/xtmd/previous/), and core's migration accounts
//! for every difference, as any pack's must. `XTMD_BLESS=1 cargo test --test migrate_core` writes the
//! surface anew.

use std::path::PathBuf;
use std::process::Command;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn core_at_1_is_the_surface_of_0_3_2s_core() {
    let sources: Vec<xtmd::check::Source> = xtmd::previous::core_files::SOURCES.iter().map(|(p, t)| xtmd::check::Source { path: p.to_string(), text: t.to_string() }).collect();
    let (name, version, text) = xtmd::cli::surface::surface_of(&sources).unwrap();
    assert_eq!((name.as_str(), version.as_str()), ("core", "1"));
    let kept = repo().join("core/surfaces/core@1.xtmdpack");
    if std::env::var("XTMD_BLESS").is_ok() {
        std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
        std::fs::write(&kept, &text).unwrap();
    }
    assert_eq!(std::fs::read_to_string(&kept).unwrap_or_default(), text, "core/surfaces/core@1.xtmdpack isn't 0.3.2's core's surface (XTMD_BLESS=1 writes it)");
}

#[test]
fn cores_migration_accounts_for_every_difference_and_its_examples_hold() {
    let core = repo().join("core");
    let pack = format!("{},{}", core.join("surfaces/core@1.xtmdpack").display(), core.join("migrations/core@2.xtmdpack").display());
    let out = Command::new(env!("CARGO_BIN_EXE_xtmd")).args(["check", "--pack", &pack, core.join("migrations/core@2.xtmdpack").to_str().unwrap()]).output().unwrap();
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{}", said);
}

/// A tree for 0.3.2, moved by `xtmd migrate`: its exit code, what was said, and each file as written.
fn moved(name: &str, files: &[(&str, &str)]) -> (i32, String, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("xtmd-core-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let packs: Vec<String> = files.iter().filter(|(p, _)| p.ends_with(".xtmdpack")).map(|(p, _)| format!("{:?}", p.split('/').next().unwrap())).collect();
    std::fs::write(dir.join("xtmd.toml"), format!("xtmd = \"0.3.2\"\npacks = [{}]\n", packs.join(", "))).unwrap();
    for (path, text) in files {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    let out = Command::new(env!("CARGO_BIN_EXE_xtmd")).current_dir(&dir).args(["migrate", ".", "--yes"]).env("XTMD_MIGRATE_STEP", "1").output().unwrap();
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let texts = files.iter().map(|(p, _)| std::fs::read_to_string(dir.join(p)).unwrap()).collect();
    let _ = std::fs::remove_dir_all(&dir);
    (out.status.code().unwrap_or(-1), said, texts)
}

#[test]
fn an_md_is_a_rich_text_and_blocks_places_its_value_where_it_stands() {
    let (code, said, docs) = moved("md", &[("doc.xtmd", "let a: md = \"# Hi\";\n\nwidget w(body: md) {\n    view {\n        @blocks(value = body);\n    }\n}\n@@@\nBefore.\n\n@blocks(value = a);\n\n@w(body = a);\n")]);
    assert_eq!(code, 0, "{}", said);
    let doc = &docs[0];
    assert!(doc.contains("let a: rich_text = \"# Hi\";") && doc.contains("widget w(body: rich_text) {"), "{}", doc);
    assert!(doc.contains("    view {\n        @body;\n    }"), "in a view, the value is placed by its name: {}", doc);
    assert!(doc.contains("Before.\n\n\\{a}\n"), "in prose, it is a hole: {}", doc);
}

#[test]
fn what_read_md_and_rich_texts_parts_is_left_for_a_person_with_why() {
    let (code, said, docs) = moved("parts", &[("doc.xtmd", "widget w(body: md) {\n    view {\n        for (b in body.blocks) {\n            @label(value = \"x\");\n        }\n    }\n}\n@@@\n@w(body = \"# Hi\");\n")]);
    assert_eq!(code, 3, "a TODO is said out loud: {}", said);
    assert!(said.contains("core@2 drops md.blocks: a rich_text holds Markdown's blocks itself"), "{}", said);
    assert!(docs[0].contains("// TODO(xtmd migrate): core@2 drops md.blocks"), "{}", docs[0]);
}

#[test]
fn a_packs_uses_line_moves_to_core_at_2() {
    let pack = "package kit;\n\npack kit@1 {\n    uses core@1;\n    provides tag;\n}\n\nwidget tag(t: text) {\n    view {\n        @label(value = t);\n    }\n    markdown {\n        \\{t}\n    }\n}\n";
    let (code, said, files) = moved("uses", &[("kit/kit.xtmdpack", pack), ("doc.xtmd", "import kit;\n@@@\n@tag(t = \"x\");\n")]);
    assert_eq!(code, 0, "{}", said);
    assert!(files[0].contains("    uses core@2;\n"), "{}", files[0]);
}
