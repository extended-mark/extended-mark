//! Every 0.3.2 file, migrated: the corpus in checks/migrate-0.3 (made by its make.py from tag v0.3.2,
//! with the probes of 25 Nov), each unit copied to a folder of its own whose xtmd.toml says 0.3.2, and
//! `xtmd migrate` run on it without writing. What each unit came to is checked against expected.tsv,
//! so a phase that moves more files says so there, and one that moves fewer fails. Nothing is skipped.
//!
//! Each unit goes through the step from 0.3.2 (src/migrate/rir/v1), XTMD_MIGRATE_STEP set while it isn't
//! yet what `xtmd migrate` takes by itself. `XTMD_BLESS=1 cargo test --test migrate_032` writes what this
//! xtmd does into expected.tsv. Where it goes: every unit valid in 0.3.2 `moved` or `todo`, and every
//! unit 0.3.2 refuses `refused`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../checks/migrate-0.3")
}

/// One unit of the corpus: the files moved together, and the packs they are built with.
struct Unit {
    id: String,
    root: PathBuf,
    files: Vec<String>,
    packs: Vec<String>,
    valid: bool,
}

fn units() -> Vec<Unit> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(corpus().join("units.json")).unwrap()).unwrap();
    let strings = |x: &Value| x.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect::<Vec<_>>();
    v["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| {
            let id = u["id"].as_str().unwrap().to_string();
            let root = match id.strip_prefix("probes/") {
                Some(p) => corpus().join("probes").join(p),
                None => corpus().join("v0.3.2"),
            };
            Unit { id, root, files: strings(&u["files"]), packs: strings(&u["packs"]), valid: u["valid"].as_bool().unwrap() }
        })
        .collect()
}

fn copy(from: &Path, to: &Path) {
    if from.is_dir() {
        for e in std::fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            copy(&e.path(), &to.join(e.file_name()));
        }
    } else {
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(from, to).unwrap();
    }
}

/// What migrating a unit came to: `moved`, `todo` (written, with what a person must look at),
/// `refused` (nothing written), or the exit code of anything else.
fn migrate(u: &Unit, scratch: &Path) -> String {
    let dir = scratch.join(u.id.replace('/', "~"));
    let _ = std::fs::remove_dir_all(&dir);
    // a probe is a tree already; a file of the tag goes in a tree whose xtmd.toml says 0.3.2
    if u.id.starts_with("probes/") {
        copy(&u.root, &dir);
    } else {
        for f in u.files.iter().chain(&u.packs) {
            copy(&u.root.join(f), &dir.join(f));
        }
        let own = u.files.iter().filter_map(|f| Path::new(f).ancestors().map(|a| a.join("xtmd.toml")).find(|t| u.root.join(t).is_file()));
        for t in own {
            copy(&u.root.join(&t), &dir.join(&t));
        }
        if !dir.join("xtmd.toml").is_file() {
            std::fs::write(dir.join("xtmd.toml"), "xtmd = \"0.3.2\"\n").unwrap();
        }
    }
    let mut args: Vec<String> = vec!["migrate".into()];
    if u.id.starts_with("probes/") {
        args.push(".".into());
    } else {
        args.extend(u.files.iter().cloned());
        if !u.packs.is_empty() {
            args.extend(["--pack".into(), u.packs.join(",")]);
        }
        args.extend(["--config".into(), "xtmd.toml".into()]);
    }
    args.extend(["--dry-run".into(), "--format".into(), "json".into()]);
    // through the step from 0.3.2, whole or not yet (rir::READY)
    let out = Command::new(env!("CARGO_BIN_EXE_xtmd")).current_dir(&dir).args(&args).env("XTMD_MIGRATE_STEP", "1").output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    match out.status.code() {
        Some(0) => "moved".into(),
        Some(3) => "todo".into(),
        Some(1) => "refused".into(),
        c => format!("exit-{}", c.unwrap_or(-1)),
    }
}

#[test]
fn every_0_3_2_file_is_migrated_as_expected() {
    let units = units();
    // the whole tag: its documents and its packs' files, but core's
    let docs = units.iter().filter(|u| !u.id.starts_with("probes/")).flat_map(|u| &u.files).filter(|f| f.ends_with(".xtmd")).count();
    assert!(docs >= 120 && units.len() >= 300, "the corpus is short: {} units, {} documents", units.len(), docs);
    let scratch = std::env::temp_dir().join(format!("xtmd-migrate-032-{}", std::process::id()));
    let results: Vec<(String, bool, String)> = std::thread::scope(|s| {
        let chunks: Vec<&[Unit]> = units.chunks(units.len().div_ceil(4)).collect();
        let handles: Vec<_> = chunks.into_iter().map(|c| s.spawn(|| c.iter().map(|u| (u.id.clone(), u.valid, migrate(u, &scratch))).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let _ = std::fs::remove_dir_all(&scratch);
    let now: String = results.iter().map(|(id, valid, s)| format!("{}\t{}\t{}\n", id, if *valid { "valid" } else { "invalid" }, s)).collect();
    let count = |valid: bool, s: &str| results.iter().filter(|(_, v, r)| *v == valid && r == s).count();
    eprintln!(
        "0.3.2 corpus: {} units valid in 0.3.2: {} moved, {} with TODOs, {} refused; {} invalid in 0.3.2: {} refused",
        results.iter().filter(|r| r.1).count(),
        count(true, "moved"),
        count(true, "todo"),
        count(true, "refused"),
        results.iter().filter(|r| !r.1).count(),
        count(false, "refused")
    );
    let expected_path = corpus().join("expected.tsv");
    if std::env::var("XTMD_BLESS").is_ok() {
        std::fs::write(&expected_path, &now).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&expected_path).unwrap_or_default();
    let differ: Vec<String> = now.lines().zip(expected.lines().chain(std::iter::repeat(""))).filter(|(a, b)| a != b).map(|(a, b)| format!("  was {:?}, is {:?}", b, a)).collect();
    assert!(differ.is_empty() && now.lines().count() == expected.lines().count(), "{} units changed (XTMD_BLESS=1 writes expected.tsv anew):\n{}", differ.len(), differ.join("\n"));
}
