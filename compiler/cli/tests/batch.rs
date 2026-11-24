//! `xtmd build` and `xtmd md` over several files and folders, and `xtmd render`.

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
    let (code, out, err) = xtmd_in(&dir, &["build", "docs", "loose.xtmd", "--out", "dist"]);
    assert_eq!((code, out.is_empty()), (0, true), "{}", err);
    assert!(err.ends_with("3 documents: 3 written\n"), "{}", err);
    for (file, alone) in [("one.ir.json", "docs/one.xtmd"), ("deep/two.ir.json", "docs/deep/two.xtmd"), ("loose.ir.json", "loose.xtmd")] {
        // what building it alone writes, each with the packs of its own xtmd.toml
        let (_, want, _) = xtmd_in(&dir, &["build", alone]);
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
    let (code, _, err) = xtmd_in(&dir, &["build", "docs", "--rir", "--out", "tools"]);
    assert!(code == 0 && dir.join("tools/one.rir.json").is_file(), "{}", err);
}

#[test]
fn one_that_doesnt_compile_is_left_out_and_the_rest_are_written() {
    let dir = scratch("errors", &[("a.xtmd", "# A\n"), ("b.xtmd", "@nothing;\n"), ("c.xtmd", "# C\n")]);
    let (code, out, err) = xtmd_in(&dir, &["build", ".", "--out", "dist", "--format", "json"]);
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
        (&["build", "x/page.xtmd", "y/page.xtmd"][..], "needs --out"),
        (&["build", "x"], "needs --out"),
        (&["md", "x", "y", "--out", "file"], "a file, and xtmd md"),
        (&["build", "x", "y", "--out", "dist"], "would both be written to"),
        (&["build", "none", "--out", "dist"], "no Extended Mark documents"),
    ] {
        let (code, _, err) = xtmd_in(&dir, args);
        assert_eq!(code, 2, "{:?}", args);
        assert!(err.contains(why), "{:?}: {}", args, err);
    }
    assert!(!dir.join("dist").exists());
    // a file named twice, or in a folder named too, is written once
    let (code, _, err) = xtmd_in(&dir, &["build", "x/page.xtmd", "x/page.xtmd", "--out", "dist"]);
    assert!(code == 0 && err.contains("1 document: 1 written"), "{}", err);
}

#[test]
fn a_flag_takes_its_value_after_an_equals_sign_too() {
    let dir = tree("equals");
    let (code, out, err) = xtmd_in(&dir, &["build", "loose.xtmd", "--out=dist/loose.ir.json"]);
    assert!(code == 0 && out.is_empty() && dir.join("dist/loose.ir.json").is_file(), "{}", err);
}

#[test]
fn render_says_what_is_missing() {
    let dir = scratch("render-usage", &[("page.xtmd", "# Hi\n"), ("bad.xtmd", "@nothing;\n"), ("has/xtmd.toml", "[engines]\nweb = [\"node\", \"render.mjs\"]\nnone = []\nlost = [\"no-such-program-xtmd\"]\n"), ("has/page.xtmd", "# Hi\n")]);
    for (args, code, why) in [
        (&["render", "page.xtmd"][..], 2, "needs --engine"),
        (&["render", "page.xtmd", "--engine", "web"], 2, "no engine `web`: there is no xtmd.toml"),
        (&["render", "has/page.xtmd", "--engine", "egui"], 2, "has lost, none, web"),
        (&["render", "has/page.xtmd", "--engine", "none"], 2, "an empty list"),
        (&["render", "has/page.xtmd", "--engine", "lost"], 2, "no-such-program-xtmd can't be run"),
        (&["render", "page.xtmd", "--engine", "./nowhere"], 2, "no such file"),
        (&["render", "page.xtmd", "bad.xtmd", "--engine", "./nowhere"], 2, "takes one file"),
    ] {
        let (got, _, err) = xtmd_in(&dir, args);
        assert!(got == code && err.contains(why), "{:?}: {} {}", args, got, err);
    }
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

    #[test]
    fn an_engine_reads_the_ir_and_gets_its_arguments_untouched() {
        let dir = scratch("render", &[("page.xtmd", "# Hi\n"), ("xtmd.toml", "[engines]\nweb = [\"sh\", \"tools/web.sh\", \"first\"]\n"), ("tools/web.sh", "for a in \"$@\"; do printf '<%s>' \"$a\"; done\ncat\n")]);
        let (code, ir, _) = xtmd_in(&dir, &["build", "page.xtmd"]);
        assert_eq!(code, 0);
        let ir = String::from_utf8(ir).unwrap();
        let ir = ir.trim_end();
        // every argument after the name, whatever it looks like to xtmd
        let (code, out, err) = xtmd_in(&dir, &["render", "page.xtmd", "--engine", "web", "--out", "theirs.png", "--pack", "x y", "--help", "--engine=again", "--", ""]);
        assert_eq!(code, 0, "{}", err);
        assert_eq!(String::from_utf8(out).unwrap(), format!("<first><--out><theirs.png><--pack><x y><--help><--engine=again><--><>{}", ir));
        assert!(err.is_empty() && !dir.join("theirs.png").exists(), "{}", err);
        // it runs in xtmd.toml's folder, wherever xtmd is
        std::fs::create_dir_all(dir.join("deep")).unwrap();
        let (code, out, err) = xtmd_in(&dir.join("deep"), &["render", "../page.xtmd", "--out=out/page.txt", "--engine=web"]);
        assert!(code == 0 && out.is_empty(), "{}", err);
        assert_eq!(std::fs::read_to_string(dir.join("deep/out/page.txt")).unwrap(), format!("<first>{}", ir));
    }

    #[test]
    fn what_it_draws_is_bytes_and_its_exit_code_is_its_own() {
        let dir = scratch("render-bytes", &[("page.xtmd", "# Hi\n"), ("bad.xtmd", "@nothing;\n")]);
        program(&dir, "png", "#!/bin/sh\nprintf '\\211PNG\\000\\377'\n");
        let (code, out, _) = xtmd_in(&dir, &["render", "page.xtmd", "--engine", "./png"]);
        assert_eq!((code, out), (0, b"\x89PNG\x00\xff".to_vec()));
        let (code, _, _) = xtmd_in(&dir, &["render", "page.xtmd", "--out", "page.png", "--engine", "./png"]);
        assert_eq!((code, std::fs::read(dir.join("page.png")).unwrap()), (0, b"\x89PNG\x00\xff".to_vec()));
        // one that fails says so, and the file that was there stays
        program(&dir, "fails", "#!/bin/sh\necho half\necho 'no canvas' >&2\nexit 7\n");
        let (code, out, err) = xtmd_in(&dir, &["render", "page.xtmd", "--out", "page.png", "--engine", "./fails"]);
        assert!(code == 7 && out.is_empty() && err.starts_with("no canvas\n"), "{} {}", code, err);
        assert_eq!(std::fs::read(dir.join("page.png")).unwrap(), b"\x89PNG\x00\xff");
        // a document that doesn't compile never reaches it
        let (code, out, err) = xtmd_in(&dir, &["render", "bad.xtmd", "--engine", "./png"]);
        assert!(code == 1 && out.is_empty() && err.contains("names.unknown-widget"), "{} {}", code, err);
    }
}
