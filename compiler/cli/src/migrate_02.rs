//! The 0.2 → 0.3 step of `xtmd migrate`, run: the frozen JS file (tools/migrate-0.2/frozen), found
//! and checked, run with Node on one document. What it writes is read by xtmd-migrate, which also
//! tells a 0.2 document apart. Only Extended Mark 0.3 carries it: a later release sends a 0.2
//! document to `npx extended-mark@0.3 migrate`.

use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) use xtmd_migrate::v02::{fill, looks_02, speaks_03, stamped, Failed, Migrated};

/// The frozen file's SHA-256, from the lock this binary was built beside; empty when it carries no
/// 0.2 step.
const SHA256: &str = env!("XTMD_MIGRATE_02_SHA256");
/// Where the frozen file is in the repository this binary was built from.
const IN_REPO: &str = env!("XTMD_MIGRATE_02_REPO");

/// A 0.2 pack: a folder with a `manifest.mkp`, or a pack's `.zip`.
pub(crate) fn is_pack(p: &Path) -> bool {
    (p.is_dir() && p.join("manifest.mkp").is_file()) || p.extension().map_or(false, |x| x == "zip")
}

/// The frozen file, checked against the SHA-256 this binary was built with.
fn frozen() -> Result<PathBuf, String> {
    if SHA256.is_empty() {
        return Err("this xtmd doesn't carry the 0.2 → 0.3 step: run `npx extended-mark@0.3 migrate` on it first".into());
    }
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("migrate-0.2.cjs")));
    let found = std::env::var_os("XTMD_MIGRATE_02").map(PathBuf::from).into_iter().chain(beside).chain((!IN_REPO.is_empty()).then(|| PathBuf::from(IN_REPO))).find(|p| p.is_file());
    let Some(path) = found else {
        return Err("the 0.2 → 0.3 step (migrate-0.2.cjs) isn't beside this xtmd; set XTMD_MIGRATE_02 to it".into());
    };
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    if xtmd_check::sha256_hex(&bytes) != SHA256 {
        return Err(format!("{} isn't the 0.2 → 0.3 step this xtmd was built with (its SHA-256 differs): not run", path.display()));
    }
    Ok(path)
}

/// Runs the frozen step on one document, with the 0.2 packs it is built with. One that 0.2.10 refuses only
/// for summaries it lacks is run again with a placeholder in each.
pub(crate) fn run(doc: &Path, packs: &[PathBuf]) -> Result<Migrated, Failed> {
    let source = std::fs::read_to_string(doc).unwrap_or_default();
    let v = run_step(doc, packs)?;
    if let Some(filled) = fill(&source, &v) {
        if let Ok(m) = run_text(doc, &filled.text, packs).and_then(|again| Migrated::read_filled(&again, &source, &filled)) {
            return Ok(m);
        }
    }
    Migrated::read(&v, &source)
}

/// Runs the step on a document with another text: a file of its name, with its definitions beside it.
fn run_text(doc: &Path, text: &str, packs: &[PathBuf]) -> Result<serde_json::Value, Failed> {
    let not_run = |e: std::io::Error| Failed::NotRun(format!("the 0.2 → 0.3 step: {}", e));
    let dir = std::env::temp_dir().join(format!("xtmd-migrate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(not_run)?;
    let copy = dir.join(doc.file_name().unwrap_or_default());
    let made = std::fs::write(&copy, text).and_then(|_| {
        // the document's own definitions, name.mkp beside name.mkp.md
        let own = doc.with_extension("");
        if own.is_file() {
            std::fs::copy(&own, dir.join(own.file_name().unwrap_or_default()))?;
        }
        Ok(())
    });
    let out = made.map_err(not_run).and_then(|_| run_step(&copy, packs));
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// What the frozen step prints for a document.
fn run_step(doc: &Path, packs: &[PathBuf]) -> Result<serde_json::Value, Failed> {
    let file = frozen().map_err(Failed::NotRun)?;
    let node = std::env::var_os("XTMD_NODE").unwrap_or_else(|| "node".into());
    let mut cmd = Command::new(&node);
    cmd.arg(&file).arg(doc);
    for p in packs {
        cmd.arg("--pack").arg(p);
    }
    let out = cmd.output().map_err(|e| Failed::NotRun(format!("the 0.2 → 0.3 step runs with Node 20 or later, and `{}` can't be run: {}", node.to_string_lossy(), e)))?;
    serde_json::from_slice(&out.stdout).map_err(|_| Failed::NotRun(format!("the 0.2 → 0.3 step failed: {}", String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("no output"))))
}

