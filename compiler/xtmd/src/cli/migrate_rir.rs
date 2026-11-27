//! `xtmd migrate --reader <xtmd of the old release>`: a tree moved from the release it was written for
//! to this one through what each release understands of it (migrate/pipeline.rs). Every file is
//! planned first; a plan with a file refused or a question open writes nothing, and one that can be
//! written is written whole, every file or none.

use std::collections::BTreeMap;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::check::Source;
use crate::migrate::pipeline::{self, Plan};
use crate::migrate::readers::{Reader, XtmdCommand};
use crate::migrate::{is_doc, with_release, RELEASE};

/// The answers in an answers file: `[steps]`, each question's id and its answer.
fn answers_in(text: &str) -> Result<BTreeMap<String, String>, String> {
    let v: toml::Value = text.parse().map_err(|e: toml::de::Error| e.message().to_string())?;
    let mut out = BTreeMap::new();
    for (k, x) in v.get("steps").and_then(|t| t.as_table()).into_iter().flatten() {
        out.insert(k.clone(), x.as_str().ok_or_else(|| format!("[steps] \"{}\" is one of the answers its question lists, in quotes", k))?.to_string());
    }
    Ok(out)
}

pub fn run(args: &[String]) -> ExitCode {
    let flag = |f: &str| args.iter().any(|a| a == f);
    let (dry, check, diff, yes) = (flag("--dry-run"), flag("--check"), flag("--diff"), flag("--yes"));
    let json = args.windows(2).any(|w| w[0] == "--format" && w[1] == "json");
    let value = |f: &str| args.windows(2).find(|w| w[0] == f).map(|w| w[1].clone()).or_else(|| args.iter().find_map(|a| a.strip_prefix(&format!("{}=", f)).map(String::from)));
    let Some(reader) = value("--reader") else {
        eprintln!("--reader takes the xtmd command of the release the tree is written for: --reader ~/xtmd-0.4.0/xtmd");
        return ExitCode::from(2);
    };
    let answers = match value("--answers") {
        None => BTreeMap::new(),
        Some(f) => match std::fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|t| answers_in(&t)) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("--answers {}: {}", f, e);
                return ExitCode::from(2);
            }
        },
    };
    // the files, and what builds them, which the old release is given as they are
    let mut targets = Vec::new();
    let mut build = Vec::new();
    let mut k = 0;
    while k < args.len() {
        let a = &args[k];
        match a.as_str() {
            "--pack" | "--config" => {
                build.extend(args[k..(k + 2).min(args.len())].iter().cloned());
                k += 2;
            }
            "--reader" | "--answers" | "--format" => k += 2,
            _ if a.starts_with("--pack=") || a.starts_with("--config=") => {
                build.push(a.clone());
                k += 1;
            }
            _ if a.starts_with("--") => k += 1,
            _ => {
                targets.push(a.clone());
                k += 1;
            }
        }
    }
    if targets.is_empty() {
        targets.push(".".into());
    }
    let mut files = Vec::new();
    for t in &targets {
        if Path::new(t).is_dir() {
            super::under(Path::new(t), &mut files);
        } else if Path::new(t).is_file() {
            files.push(t.clone());
        } else {
            eprintln!("{}: no such file or folder", t);
            return ExitCode::from(2);
        }
    }
    files.sort();
    files.dedup();
    let packs = match super::options(&[targets.clone(), build.clone()].concat()) {
        Ok((_, p, _)) => p,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(2);
        }
    };

    let scratch = std::env::temp_dir().join(format!("xtmd-migrate-{}", std::process::id()));
    let read = XtmdCommand { program: PathBuf::from(&reader), scratch: scratch.clone() }.read(&files, &build);
    let _ = std::fs::remove_dir_all(&scratch);
    let read = match read {
        Ok(r) => r,
        Err(e) => {
            for e in e {
                eprintln!("error: {}", e);
            }
            eprintln!("nothing written: the release the tree is written for couldn't read it");
            return ExitCode::from(1);
        }
    };
    let base = crate::check::compile_core(&super::core_sources());
    let compiles = |printed: &[Source]| compile_all(&base, &packs, printed);
    let plan = pipeline::plan(read, crate::migrate::rir::STEPS, answers, &compiles);

    // what is written: the files that change, and xtmd.toml's release, which says the tree is moved
    let mut writes: Vec<(String, Option<String>, String)> = Vec::new();
    for p in &plan.files {
        let old = std::fs::read_to_string(&p.path).ok();
        if old.as_deref() != Some(p.text.as_str()) {
            writes.push((p.path.clone(), old, p.text.clone()));
        }
    }
    for toml in configs(&targets, &build) {
        if let Ok(text) = std::fs::read_to_string(&toml) {
            let now = with_release(&text, RELEASE);
            if now != text {
                writes.push((toml.display().to_string(), Some(text), now));
            }
        }
    }
    say(&plan, &writes, json, diff);
    if !plan.ready() {
        return ExitCode::from(1);
    }
    if dry || diff || check {
        return ExitCode::from(if check && !writes.is_empty() { 1 } else { 0 });
    }
    if writes.is_empty() {
        return ExitCode::SUCCESS;
    }
    if !yes {
        if !std::io::stdin().is_terminal() || json {
            eprintln!("nothing written: --yes writes it");
            return ExitCode::from(1);
        }
        eprint!("Write {} file{}? [y/N] ", writes.len(), if writes.len() == 1 { "" } else { "s" });
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        if !matches!(line.trim(), "y" | "Y" | "yes") {
            eprintln!("nothing written");
            return ExitCode::from(1);
        }
    }
    match write_all(&writes) {
        Ok(()) => {
            if !json {
                eprintln!("written: {} file{}", writes.len(), if writes.len() == 1 { "" } else { "s" });
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {}; nothing written", e);
            ExitCode::from(1)
        }
    }
}

/// Each document printed, compiled by this xtmd with the packs printed beside it in place of their old
/// files: its Markdown, or its errors.
pub(crate) fn compile_all(base: &crate::check::Checker, packs: &[Source], printed: &[Source]) -> BTreeMap<String, Result<String, Vec<String>>> {
    let new_packs: Vec<Source> = printed.iter().filter(|s| !is_doc(&s.path)).cloned().collect();
    let mut world: Vec<Source> = packs.iter().filter(|p| !new_packs.iter().any(|n| same(&n.path, &p.path))).cloned().collect();
    world.extend(new_packs);
    let mut out = BTreeMap::new();
    for doc in printed.iter().filter(|s| is_doc(&s.path)) {
        let c = crate::check::compile_on(base, &[world.clone()], &[doc.clone()], "doc");
        let errors: Vec<String> = c.diagnostics.iter().filter(|d| d.diagnostic.is_error()).map(|d| d.diagnostic.message.to_string()).collect();
        let result = match c.module {
            Some(mut m) if errors.is_empty() => {
                crate::lower::lower(&mut m);
                crate::markdown::page(&m).map(|b| crate::markdown::write_page(m.front_matter.as_deref(), &b)).map_err(|e| vec![format!("its Markdown can't be written: {}", e.0)])
            }
            _ => Err(errors),
        };
        out.insert(doc.path.clone(), result);
    }
    out
}

fn same(a: &str, b: &str) -> bool {
    Path::new(a) == Path::new(b) || std::fs::canonicalize(a).ok().zip(std::fs::canonicalize(b).ok()).map_or(false, |(x, y)| x == y)
}

/// The xtmd.toml files that say the tree's release: `--config`'s, or a folder's own.
fn configs(targets: &[String], build: &[String]) -> Vec<PathBuf> {
    let given = build.windows(2).find(|w| w[0] == "--config").map(|w| PathBuf::from(&w[1])).or_else(|| build.iter().find_map(|a| a.strip_prefix("--config=").map(PathBuf::from)));
    if let Some(c) = given {
        return vec![c];
    }
    targets.iter().map(|t| Path::new(t).join("xtmd.toml")).filter(|p| p.is_file()).collect()
}

/// Every file, or none: each new text is written beside its file first, and only when all of them are
/// does each take its file's place.
fn write_all(writes: &[(String, Option<String>, String)]) -> Result<(), String> {
    let beside = |p: &str| format!("{}.xtmd-migrate", p);
    for (k, (path, _, text)) in writes.iter().enumerate() {
        if let Err(e) = std::fs::write(beside(path), text) {
            for (p, _, _) in &writes[..=k] {
                let _ = std::fs::remove_file(beside(p));
            }
            return Err(format!("{}: {}", path, e));
        }
    }
    for (path, _, _) in writes {
        std::fs::rename(beside(path), path).map_err(|e| format!("{}: {}", path, e))?;
    }
    Ok(())
}

fn say(plan: &Plan, writes: &[(String, Option<String>, String)], json: bool, diff: bool) {
    if json {
        let line = |v: serde_json::Value| println!("{}", v);
        for (path, _, _) in writes {
            line(serde_json::json!({ "type": "file", "path": path, "change": "changed" }));
        }
        for t in &plan.todos {
            line(serde_json::json!({ "type": "todo", "message": t }));
        }
        for q in &plan.open {
            line(serde_json::json!({ "type": "question", "id": q.id, "question": q.question, "options": q.options }));
        }
        for r in &plan.refused {
            line(serde_json::json!({ "type": "refused", "message": r }));
        }
        line(serde_json::json!({ "type": "summary", "from": plan.from_release, "from_rir": plan.from_rir, "to": RELEASE, "to_rir": crate::migrate::rir::VERSION, "files": writes.len(), "ready": plan.ready() }));
        return;
    }
    eprintln!("read by {}'s xtmd as rir {}, moved to {} (rir {})", plan.from_release, plan.from_rir, RELEASE, crate::migrate::rir::VERSION);
    for (path, old, new) in writes {
        if diff {
            print!("{}", crate::migrate::unified(path, old.as_deref().unwrap_or(""), new));
        } else {
            eprintln!("  {}: changes", path);
        }
    }
    if writes.is_empty() && plan.ready() {
        eprintln!("  nothing to change");
    }
    for t in &plan.todos {
        eprintln!("  TODO {}", t);
    }
    for q in &plan.open {
        let options = if q.options.is_empty() { String::new() } else { format!(" ({})", q.options.join(" | ")) };
        let example = q.options.first().cloned().unwrap_or_else(|| "…".into());
        eprintln!("  question {}: {}{}: answer it in --answers answers.toml as [steps] \"{}\" = \"{}\"", q.id, q.question, options, q.id, example);
    }
    for r in &plan.refused {
        eprintln!("  refused: {}", r);
    }
    if !plan.ready() {
        eprintln!("nothing written: {}", if plan.refused.is_empty() { "a question is open" } else { "every file is written, or none" });
    }
}
