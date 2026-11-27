//! Readers: how a tree written for an earlier release becomes a resolved IR. The release that wrote a
//! file is the one that understands it, so each release reads its own files, and this xtmd reads
//! only what that release wrote: its resolved IR and each document's Markdown. A reader is anything
//! that gives those; this xtmd brings one for a release's own `xtmd` command, from 0.4.0 on, whose
//! `compile --rir` and `md` it runs. 0.3's command has no `compile`: its reader, from 0.3.2's
//! published crates, is set aside (#14715).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

/// What a release read of a tree: its resolved IR, every file in it, and each document's Markdown, by
/// its path, as that release writes it.
pub struct Read {
    pub release: String,
    pub rir: Value,
    pub markdown: BTreeMap<String, String>,
}

pub trait Reader {
    /// The tree's files, each one a document or a pack's file, with the arguments that build them
    /// (`--pack`, `--config`).
    fn read(&self, files: &[String], args: &[String]) -> Result<Read, Vec<String>>;
}

/// A release's own `xtmd` command, run once a file.
pub struct XtmdCommand {
    pub program: PathBuf,
    /// Where it writes what it gives: a folder of this run's.
    pub scratch: PathBuf,
}

impl XtmdCommand {
    fn run(&self, args: &[String]) -> Result<(), String> {
        let out = Command::new(&self.program).args(args).output().map_err(|e| format!("{} can't be run: {}", self.program.display(), e))?;
        if out.status.success() {
            return Ok(());
        }
        let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(format!("{} {} failed: {}", self.program.display(), args.join(" "), if said.is_empty() { format!("exit {}", out.status.code().unwrap_or(-1)) } else { said }))
    }
}

impl Reader for XtmdCommand {
    fn read(&self, files: &[String], args: &[String]) -> Result<Read, Vec<String>> {
        std::fs::create_dir_all(&self.scratch).map_err(|e| vec![format!("{}: {}", self.scratch.display(), e)])?;
        let (mut all, mut markdown, mut errors) = (Vec::new(), BTreeMap::new(), Vec::new());
        let (mut version, mut release): (Option<u64>, Option<String>) = (None, None);
        // the first one's own parts, the packs it was built with among them; the files are every one's
        let mut top: Option<Value> = None;
        for (k, f) in files.iter().enumerate() {
            let rir = self.scratch.join(format!("{}.rir.json", k));
            let mut a = vec!["compile".to_string(), f.clone(), "--rir".into(), "--out".into(), rir.display().to_string()];
            a.extend(args.iter().cloned());
            if let Err(e) = self.run(&a) {
                errors.push(e);
                continue;
            }
            let v: Value = match std::fs::read_to_string(&rir).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(format!("{}: what {} wrote isn't a resolved IR: {}", f, self.program.display(), e));
                    continue;
                }
            };
            // every file is read by the same release, into the same version
            let (vn, rel) = (v["rir"].as_u64(), v["xtmd"].as_str().map(String::from));
            if version.is_some() && (version != vn || release != rel) {
                errors.push(format!("{}: read as rir {:?} of {:?}, and the others as rir {:?} of {:?}", f, vn, rel, version, release));
                continue;
            }
            version = vn;
            release = rel;
            top.get_or_insert_with(|| v.clone());
            // the file as its resolved IR names it, which decompile writes it at
            let mut named = f.clone();
            for file in v["files"].as_array().into_iter().flatten() {
                if let Some(p) = file["path"].as_str().filter(|p| same(p, f)) {
                    named = p.to_string();
                    if !all.iter().any(|x: &Value| x["path"] == file["path"]) {
                        all.push(file.clone());
                    }
                }
            }
            if crate::migrate::is_doc(f) {
                let md = self.scratch.join(format!("{}.md", k));
                let mut a = vec!["md".to_string(), f.clone(), "--out".into(), md.display().to_string()];
                a.extend(args.iter().cloned());
                match self.run(&a).and_then(|_| std::fs::read_to_string(&md).map_err(|e| e.to_string())) {
                    Ok(t) => {
                        markdown.insert(named, t);
                    }
                    Err(e) => errors.push(e),
                }
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let release = release.ok_or_else(|| vec![format!("{} wrote no release (\"xtmd\") in its resolved IR", self.program.display())])?;
        let mut rir = top.unwrap_or_else(|| json!({ "rir": version.unwrap_or(0), "xtmd": release, "packs": [] }));
        rir["files"] = Value::Array(all);
        Ok(Read { release, rir, markdown })
    }
}

/// The same file, whether named as given or as a resolved IR names it.
fn same(a: &str, b: &str) -> bool {
    Path::new(a) == Path::new(b) || std::fs::canonicalize(a).ok().zip(std::fs::canonicalize(b).ok()).map_or(false, |(x, y)| x == y)
}
