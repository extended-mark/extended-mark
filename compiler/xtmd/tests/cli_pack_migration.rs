//! examples/pack-migration, run as its README says: the pack checks, its surface is the one kept, and its
//! document moves to the one beside it.

use std::path::{Path, PathBuf};
use std::process::Command;

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (a, b) = (e.path(), to.join(e.file_name()));
        if a.is_dir() {
            copy(&a, &b);
        } else {
            std::fs::copy(a, b).unwrap();
        }
    }
}

#[test]
fn the_example_moves_its_document_to_the_packs_new_version() {
    let example = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/pack-migration");
    let dir = std::env::temp_dir().join(format!("xtmd-pack-migration-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy(&example, &dir);
    let xtmd = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_xtmd")).current_dir(&dir).args(args).output().unwrap();
        assert!(out.status.success(), "xtmd {}: {}{}", args.join(" "), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let read = |p: &str| std::fs::read_to_string(dir.join(p)).unwrap();
    xtmd(&["check", "kit"]);
    xtmd(&["surface", "kit", "--check"]);
    xtmd(&["fmt", "--check", "kit", "docs"]);
    let said = xtmd(&["migrate", "docs", "--dry-run"]);
    for what in ["kit@1 → kit@2  5 changes; 1 deletion", "meter → gauge", "a gauge is in the ink's colour unless it says another", "loud = true: a note that isn't GOOD has a red line instead"] {
        assert!(said.contains(what), "{}: {}", what, said);
    }
    xtmd(&["migrate", "docs", "--yes", "--allow-dirty"]);
    assert_eq!(read("docs/release.xtmd"), read("moved/release.xtmd"));
    assert!(read("xtmd.toml").contains("kit = 2"));
    xtmd(&["check", "docs"]);
    assert!(xtmd(&["migrate", "docs", "--check"]).contains("up to date"));
}
