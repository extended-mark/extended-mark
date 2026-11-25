//! `xtmd build` and `xtmd md` over several files and folders: core is compiled once, and each document is
//! written into the `--out` folder, at its place in the folder named.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::cli::{out, plural};

/// A document and where its output goes.
pub(crate) struct Job {
    pub file: String,
    pub to: PathBuf,
}

/// Whether what is named is many documents, or one to a folder.
pub(crate) fn wanted(named: &[String], out: Option<&str>) -> bool {
    named.len() > 1 || named.iter().any(|f| Path::new(f).is_dir()) || (!named.is_empty() && out.map_or(false, |o| o.ends_with(['/', '\\']) || Path::new(o).is_dir()))
}

/// The documents named, each with its place under `out`: a folder's keep theirs in it, and a file named
/// alone goes straight into `out`.
pub(crate) fn jobs(cmd: &str, named: &[String], out: Option<&str>, ext: &str) -> Result<Vec<Job>, String> {
    let Some(out) = out else {
        return Err(format!("xtmd {} with more than one file, or a folder, needs --out and the folder to write into", cmd));
    };
    if Path::new(out).is_file() {
        return Err(format!("--out {}: a file, and xtmd {} with more than one file, or a folder, writes into a folder", out, cmd));
    }
    let mut jobs: Vec<Job> = Vec::new();
    let mut add = |file: String, place: PathBuf| {
        // a document found twice (a folder, and a file in it) is written once
        if !jobs.iter().any(|j| j.file == file) {
            let name = place.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let stem = name.strip_suffix(".xtmd").or_else(|| name.strip_suffix(".xtmdpack")).unwrap_or(&name);
            jobs.push(Job { file, to: Path::new(out).join(place.with_file_name(format!("{}.{}", stem, ext))) });
        }
    };
    for n in named {
        if !Path::new(n).is_dir() {
            add(n.clone(), PathBuf::from(Path::new(n).file_name().unwrap_or_default()));
            continue;
        }
        let mut found = Vec::new();
        crate::cli::under(Path::new(n), &mut found);
        // a folder's documents: its packs' files are what they are compiled with
        for f in found.into_iter().filter(|f| f.ends_with(".xtmd")) {
            let place = Path::new(&f).strip_prefix(n).map(Path::to_path_buf).unwrap_or_else(|_| PathBuf::from(&f));
            add(f, place);
        }
    }
    if jobs.is_empty() {
        return Err(format!("{}: no Extended Mark documents (.xtmd) in it", named.join(", ")));
    }
    let mut at: BTreeMap<&Path, &str> = BTreeMap::new();
    for j in &jobs {
        if let Some(other) = at.insert(&j.to, &j.file) {
            return Err(format!("{} and {} would both be written to {}: name the folder they are in, or write them apart", other, j.file, j.to.display()));
        }
    }
    Ok(jobs)
}

/// What became of one document: what to write, or the exit code it alone would have had.
pub(crate) type Made = Result<String, u8>;

/// Makes each document with `one` and writes it, saying what was written; the packs are the `xtmd.toml`'s
/// of each document's own folder, then `--pack`'s.
pub(crate) fn run(jobs: &[Job], args: &[String], json: bool, mut one: impl FnMut(&str, &[crate::check::Source]) -> Made) -> ExitCode {
    let mut packs: HashMap<Option<PathBuf>, Vec<crate::check::Source>> = HashMap::new();
    let (mut written, mut errors, mut unread) = (0, 0, 0);
    for j in jobs {
        let key = if args.iter().any(|a| a == "--config" || a.starts_with("--config=")) { None } else { crate::cli::find_config(Some(&j.file)) };
        if !packs.contains_key(&key) {
            match crate::cli::packs_for(args, &j.file) {
                Ok(p) => {
                    packs.insert(key.clone(), p);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    return ExitCode::from(2);
                }
            }
        }
        let made = one(&j.file, &packs[&key]).and_then(|text| {
            out::write_file(&j.to.to_string_lossy(), text).map_err(|e| {
                eprintln!("{}", e);
                2
            })
        });
        let result = match made {
            Ok(()) => {
                written += 1;
                "written"
            }
            Err(1) => {
                errors += 1;
                "errors"
            }
            Err(_) => {
                unread += 1;
                "not read"
            }
        };
        if json {
            let mut v = serde_json::json!({ "type": "file", "file": j.file, "result": result });
            if result == "written" {
                v["out"] = serde_json::json!(j.to);
            }
            out::line(v);
        } else if result == "written" {
            out::note(&format!("{} -> {}", j.file, j.to.display()));
        }
    }
    let code: u8 = if unread > 0 { 2 } else if errors > 0 { 1 } else { 0 };
    if json {
        out::line(serde_json::json!({ "type": "summary", "files": jobs.len(), "written": written, "with_errors": errors, "unreadable": unread, "exit": code }));
    } else {
        let mut said = format!("{}: {} written", plural(jobs.len(), "document"), written);
        if errors > 0 {
            said += &format!(", {} with errors", errors);
        }
        if unread > 0 {
            said += &format!(", {} not read or written", unread);
        }
        out::note(&said);
    }
    ExitCode::from(code)
}
