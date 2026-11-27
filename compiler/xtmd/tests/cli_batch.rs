//! `xtmd compile` and `xtmd md` over several files and folders, and `xtmd export --engine`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn xtmd_in(dir: &Path, args: &[&str]) -> (i32, Vec<u8>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_xtmd")).current_dir(dir).args(args).output().unwrap();
    (out.status.code().unwrap_or(-1), out.stdout, String::from_utf8_lossy(&out.stderr).into_owned())
}

fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtmd-batch-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, text) in files {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

const KIT: &str = "package kit;\n\npack kit@1 {\n    provides badge;\n}\n\nwidget badge(of: text) {\n    view {\n        @label(value = of);\n    }\n    markdown {\n        \\{of}\n    }\n}\n";

fn tree(name: &str) -> PathBuf {
    scratch(
        name,
        &[
            ("docs/one.xtmd", "# One\n\n**Bold** words.\n"),
            ("docs/deep/two.xtmd", "import kit;\n@@@\n# Two\n\n@badge(of = \"Kit\");\n"),
            ("docs/deep/xtmd.toml", "packs = [\"kit\"]\n"),
            ("docs/deep/kit/kit.xtmdpack", KIT),
            ("loose.xtmd", "# Loose\n"),
        ],
    )
}

#[test]
fn a_folder_is_built_into_another_each_document_at_its_place() {
    let dir = tree("folder");
    let (code, out, err) = xtmd_in(&dir, &["compile", "docs", "loose.xtmd", "--out", "dist"]);
    assert_eq!((code, out.is_empty()), (0, true), "{}", err);
    assert!(err.ends_with("3 documents: 3 written\n"), "{}", err);
    for (file, alone) in [("one.ir.json", "docs/one.xtmd"), ("deep/two.ir.json", "docs/deep/two.xtmd"), ("loose.ir.json", "loose.xtmd")] {
        // what building it alone writes, each with the packs of its own xtmd.toml
        let (_, want, _) = xtmd_in(&dir, &["compile", alone]);
        let got = std::fs::read_to_string(dir.join("dist").join(file)).unwrap();
        assert_eq!(got.trim_end(), String::from_utf8(want).unwrap().trim_end(), "{}", file);
    }
    // a pack's files are compiled with, not built
    assert!(!dir.join("dist/deep/kit").exists());

    let (code, _, err) = xtmd_in(&dir, &["md", "docs", "--out", "text"]);
    assert_eq!(code, 0, "{}", err);
    assert_eq!(std::fs::read_to_string(dir.join("text/deep/two.md")).unwrap(), "# Two\n\nKit\n");
    let (code, _, _) = xtmd_in(&dir, &["md", "docs/one.xtmd", "--format", "json", "--out", "blocks/"]);
    assert_eq!(code, 0);
    let blocks: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("blocks/one.blocks.json")).unwrap()).unwrap();
    assert_eq!(blocks.as_array().unwrap().len(), 2);
    // the resolved IR too
    let (code, _, err) = xtmd_in(&dir, &["compile", "docs", "--rir", "--out", "tools"]);
    assert!(code == 0 && dir.join("tools/one.rir.json").is_file(), "{}", err);
}

#[test]
fn one_that_doesnt_compile_is_left_out_and_the_rest_are_written() {
    let dir = scratch("errors", &[("a.xtmd", "# A\n"), ("b.xtmd", "@nothing;\n"), ("c.xtmd", "# C\n")]);
    let (code, out, err) = xtmd_in(&dir, &["compile", ".", "--out", "dist", "--format", "json"]);
    assert_eq!(code, 1);
    let lines: Vec<serde_json::Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.iter().map(|l| l["result"].as_str().unwrap_or("")).collect::<Vec<_>>(), ["written", "errors", "written", ""]);
    assert_eq!((lines[3]["written"].as_u64(), lines[3]["with_errors"].as_u64(), lines[3]["exit"].as_u64()), (Some(2), Some(1), Some(1)));
    assert!(err.contains("names.unknown-widget"), "{}", err);
    assert!(dir.join("dist/a.ir.json").is_file() && dir.join("dist/c.ir.json").is_file() && !dir.join("dist/b.ir.json").exists());
}

#[test]
fn several_need_a_folder_to_go_into_and_never_the_same_place() {
    let dir = scratch("usage", &[("x/page.xtmd", "# X\n"), ("y/page.xtmd", "# Y\n"), ("file", ""), ("none/readme.txt", "")]);
    for (args, why) in [
        (&["compile", "x/page.xtmd", "y/page.xtmd"][..], "needs --out"),
        (&["compile", "x"], "needs --out"),
        (&["md", "x", "y", "--out", "file"], "a file, and xtmd md"),
        (&["compile", "x", "y", "--out", "dist"], "would both be written to"),
        (&["compile", "none", "--out", "dist"], "no Extended Mark documents"),
    ] {
        let (code, _, err) = xtmd_in(&dir, args);
        assert_eq!(code, 2, "{:?}", args);
        assert!(err.contains(why), "{:?}: {}", args, err);
    }
    assert!(!dir.join("dist").exists());
    // a file named twice, or in a folder named too, is written once
    let (code, _, err) = xtmd_in(&dir, &["compile", "x/page.xtmd", "x/page.xtmd", "--out", "dist"]);
    assert!(code == 0 && err.contains("1 document: 1 written"), "{}", err);
}

#[test]
fn a_flag_takes_its_value_after_an_equals_sign_too() {
    let dir = tree("equals");
    let (code, out, err) = xtmd_in(&dir, &["compile", "loose.xtmd", "--out=dist/loose.ir.json"]);
    assert!(code == 0 && out.is_empty() && dir.join("dist/loose.ir.json").is_file(), "{}", err);
}

#[test]
fn an_engine_that_cant_be_run_is_said() {
    let dir = scratch("engine-usage", &[("page.xtmd", "# Hi\n"), ("taken", "a file"), ("has/xtmd.toml", "[engines]\nweb = [\"node\", \"engine.mjs\"]\nnone = []\nlost = [\"no-such-program-xtmd\"]\n"), ("has/page.xtmd", "# Hi\n")]);
    for (args, why) in [
        (&["export", "page.xtmd", "--engine", "web"][..], "needs --out"),
        (&["export", "page.xtmd", "--out", "taken", "--engine", "web"], "builds into a folder"),
        (&["export", "page.xtmd", "--out", "d", "--engine", "web"], "no engine `web`: there is no xtmd.toml"),
        (&["export", "has/page.xtmd", "--out", "d", "--engine", "egui"], "has lost, none, web"),
        (&["export", "has/page.xtmd", "--out", "d", "--engine", "none"], "an empty list"),
        (&["export", "has/page.xtmd", "--out", "d", "--engine", "lost"], "no-such-program-xtmd can't be run"),
        (&["export", "page.xtmd", "--out", "d", "--engine", "./nowhere"], "no such file"),
        (&["export", "page.xtmd", "--rir", "--out", "d", "--engine", "./nowhere"], "xtmd compile writes the resolved IR"),
    ] {
        let (got, _, err) = xtmd_in(&dir, args);
        assert!(got == 2 && err.contains(why), "{:?}: {} {}", args, got, err);
        assert!(!dir.join("d").exists());
    }
    // the commands are the ones there are
    let (code, _, err) = xtmd_in(&dir, &["render", "page.xtmd"]);
    assert!(code == 2 && err.contains("no command render"), "{}", err);
}

#[cfg(unix)]
mod engines {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn program(dir: &Path, name: &str, text: &str) {
        let p = dir.join(name);
        std::fs::write(&p, text).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn read(p: PathBuf) -> String {
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {}", p.display(), e))
    }

    /// An engine that keeps what it was given, and gives these files.
    fn keeping(files: &str) -> String {
        format!("#!/bin/sh\nfor a in \"$@\"; do printf '<%s>' \"$a\"; done > args.txt\ncat > job.json\nprintf '%s' '{{\"version\":1,\"files\":[{}]}}'\n", files)
    }

    #[test]
    fn it_is_given_every_document_at_once_and_its_arguments_untouched() {
        let dir = tree("engine-job");
        std::fs::write(dir.join("xtmd.toml"), "[engines]\nweb = [\"sh\", \"tools/web.sh\", \"first\"]\n").unwrap();
        std::fs::create_dir_all(dir.join("tools")).unwrap();
        std::fs::write(dir.join("tools/web.sh"), keeping(r#"{"path":"one.html","text":"1"},{"path":"deep/two.html","text":"2"},{"path":"assets/shared.css","text":"c"}"#)).unwrap();
        let (code, out, err) = xtmd_in(&dir, &["export", "docs", "loose.xtmd", "--out=dist", "--engine", "web", "--out", "theirs", "--pack", "x y", "--help", "--engine=again", "--", ""]);
        assert!(code == 0 && out.is_empty(), "{}", err);
        assert_eq!(read(dir.join("args.txt")), "<first><--out><theirs><--pack><x y><--help><--engine=again><--><>");
        let job: serde_json::Value = serde_json::from_str(&read(dir.join("job.json"))).unwrap();
        let names: Vec<&str> = job["documents"].as_array().unwrap().iter().map(|d| d["name"].as_str().unwrap()).collect();
        assert_eq!((job["version"].as_i64(), names), (Some(1), vec!["deep/two", "one", "loose"]));
        // each IR is the one `xtmd compile` writes
        for (name, file) in [("one", "docs/one.xtmd"), ("deep/two", "docs/deep/two.xtmd")] {
            let (_, ir, _) = xtmd_in(&dir, &["compile", file]);
            let ir: serde_json::Value = serde_json::from_slice(&ir).unwrap();
            assert_eq!(job["documents"].as_array().unwrap().iter().find(|d| d["name"] == name).unwrap()["ir"], ir["ir"], "{name}");
        }
        assert_eq!((read(dir.join("dist/one.html")), read(dir.join("dist/deep/two.html")), read(dir.join("dist/assets/shared.css"))), ("1".into(), "2".into(), "c".into()));
        assert!(err.contains("3 documents: 3 built into 3 files"), "{}", err);
        // it runs in xtmd.toml's folder, wherever xtmd is
        std::fs::remove_file(dir.join("job.json")).unwrap();
        let (code, _, err) = xtmd_in(&dir.join("docs"), &["export", "../loose.xtmd", "--out", "here", "--engine=web"]);
        assert!(code == 0 && dir.join("job.json").is_file() && dir.join("docs/here/one.html").is_file(), "{}", err);
    }

    #[test]
    fn a_file_may_be_bytes_and_json_says_what_was_written() {
        let dir = scratch("engine-bytes", &[("page.xtmd", "# Hi\n")]);
        program(&dir, "png", &keeping(r#"{"path":"page.png","base64":"iVBORwD/"}"#));
        let (code, out, err) = xtmd_in(&dir, &["export", "page.xtmd", "--format", "json", "--out", "dist", "--engine", "./png"]);
        assert_eq!((code, std::fs::read(dir.join("dist/page.png")).unwrap()), (0, b"\x89PNG\x00\xff".to_vec()), "{}", err);
        let lines: Vec<serde_json::Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!((lines[0]["type"].as_str(), lines[0]["bytes"].as_i64()), (Some("file"), Some(6)));
        assert_eq!((lines[1]["built"].as_i64(), lines[1]["written"].as_i64(), lines[1]["exit"].as_i64()), (Some(1), Some(1), Some(0)));
    }

    #[test]
    fn one_that_fails_or_gives_what_cant_be_written_changes_nothing() {
        let dir = scratch("engine-fails", &[("page.xtmd", "# Hi\n"), ("bad.xtmd", "@nothing;\n"), ("dist/page.html", "as it was")]);
        program(&dir, "fails", "#!/bin/sh\necho half\necho 'no canvas' >&2\nexit 7\n");
        let (code, _, err) = xtmd_in(&dir, &["export", "page.xtmd", "--out", "dist", "--engine", "./fails"]);
        assert!(code == 7 && err.starts_with("no canvas\n"), "{} {}", code, err);
        program(&dir, "out", &keeping(r#"{"path":"page.html","text":"new"},{"path":"../up.html","text":"x"}"#));
        let (code, _, err) = xtmd_in(&dir, &["export", "page.xtmd", "--out", "dist", "--engine", "./out"]);
        assert!(code == 1 && err.contains("no path under") && err.contains("nothing is written") && !dir.join("up.html").exists(), "{} {}", code, err);
        assert_eq!(read(dir.join("dist/page.html")), "as it was");
        std::fs::remove_file(dir.join("job.json")).unwrap();
        // a document that doesn't compile never reaches it; the others do, and the run fails
        program(&dir, "ok", &keeping(r#"{"path":"page.html","text":"new"}"#));
        let (code, _, err) = xtmd_in(&dir, &["export", "bad.xtmd", "--out", "dist", "--engine", "./ok"]);
        assert!(code == 1 && err.contains("names.unknown-widget") && !dir.join("job.json").exists(), "{} {}", code, err);
        let (code, _, err) = xtmd_in(&dir, &["export", "bad.xtmd", "page.xtmd", "--out", "dist", "--engine", "./ok"]);
        assert!(code == 1 && err.contains("2 documents: 1 built into 1 file, 1 with errors"), "{} {}", code, err);
        assert_eq!(read(dir.join("dist/page.html")), "new");
    }
}
