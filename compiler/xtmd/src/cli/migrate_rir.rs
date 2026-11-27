//! `xtmd migrate` on a tree written for the release before this one: moved to this one through what
//! each release understands of it (migrate/pipeline.rs). The tree's release is xtmd.toml's `xtmd`, and
//! one that says none is taken as written for the release before, out loud. A tree written for any
//! other release is refused by name: each release in between moves it, in turn. Every file is planned
//! first; a plan with a file refused or a question open writes nothing, and one that can be written is
//! written whole, every file or none. What it leaves for a person is listed, and exits 3.

use std::collections::BTreeMap;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::check::Source;
use crate::migrate::pipeline::{self, Checked, Plan};
use crate::migrate::readers::{self, Reader, PREVIOUS};
use crate::migrate::{is_doc, release_in, release_of, with_release, RELEASE};

/// The exit code of a run that writes, or would write, what a person must look at.
pub const FOR_A_PERSON: u8 = 3;

/// The answers in an answers file: `[steps]`, each question's id and its answer.
fn answers_in(text: &str) -> Result<BTreeMap<String, String>, String> {
    let v: toml::Value = text.parse().map_err(|e: toml::de::Error| e.message().to_string())?;
    let mut out = BTreeMap::new();
    for (k, x) in v.get("steps").and_then(|t| t.as_table()).into_iter().flatten() {
        out.insert(k.clone(), x.as_str().ok_or_else(|| format!("[steps] \"{}\" is one of the answers its question lists, in quotes", k))?.to_string());
    }
    Ok(out)
}

/// The release a tree is written for, by its xtmd.toml, and whether it was taken to be the previous
/// one because the file says none.
fn tree_release(args: &[String]) -> Result<(String, bool), String> {
    let (files, _, config, _) = super::given(args)?;
    let path = config.map(PathBuf::from).or_else(|| super::find_config(files.first().map(String::as_str)));
    match path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| release_in(&t)) {
        Some(r) => Ok((r, false)),
        None => Ok((PREVIOUS.to_string(), true)),
    }
}

/// Where `xtmd migrate` goes for these arguments: here, for a tree written for the release before this
/// one; nowhere, with why, for one written for any other but this; or, for a tree written for this
/// release and an IR, the pack migrations and IR steps of cli/migrate.rs (none).
pub fn route(args: &[String]) -> Option<ExitCode> {
    if args.iter().any(|a| a.ends_with(".json")) {
        return None;
    }
    let (release, assumed) = match tree_release(args) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", e);
            return Some(ExitCode::from(2));
        }
    };
    if release == RELEASE {
        return None;
    }
    let (Some(r), Some(now), Some(before)) = (release_of(&release), release_of(RELEASE), release_of(PREVIOUS)) else {
        eprintln!("xtmd.toml: xtmd = \"{}\" isn't a release of xtmd, such as \"{}\"", release, PREVIOUS);
        return Some(ExitCode::from(2));
    };
    if r > now {
        eprintln!("xtmd.toml says the tree is written for xtmd {}, which is newer than this xtmd ({}): use xtmd {} or later", release, RELEASE, release);
        return Some(ExitCode::from(2));
    }
    if r != before {
        eprintln!("nothing written: the tree is written for xtmd {}, and this xtmd ({}) moves only a tree written for {}, the release before it. Migrate it with the xtmd of each release from {}'s next to {} first, in turn (tools/migrate/chain.py does).", release, RELEASE, PREVIOUS, release, PREVIOUS);
        return Some(ExitCode::from(1));
    }
    // until the step from the release before is whole, its trees still go the way they went
    if !crate::migrate::rir::READY {
        return None;
    }
    if assumed {
        eprintln!("xtmd.toml says no release (xtmd = \"…\"): the tree is read as written for {}, the release before this one", PREVIOUS);
    }
    Some(run(args, readers::previous(), crate::migrate::rir::STEP.as_ref()))
}

/// The tree the arguments name, read by `reader` and moved by `step` when it was written for the
/// release before.
pub fn run(args: &[String], reader: Option<Box<dyn Reader>>, step: Option<&crate::migrate::rir::Step>) -> ExitCode {
    let flag = |f: &str| args.iter().any(|a| a == f);
    let (dry, check, diff, yes) = (flag("--dry-run"), flag("--check"), flag("--diff"), flag("--yes"));
    let json = args.windows(2).any(|w| w[0] == "--format" && w[1] == "json");
    let value = |f: &str| args.windows(2).find(|w| w[0] == f).map(|w| w[1].clone()).or_else(|| args.iter().find_map(|a| a.strip_prefix(&format!("{}=", f)).map(String::from)));
    let Some(reader) = reader else {
        eprintln!("nothing written: this xtmd isn't built with {}'s front end (compiler/xtmd/previous/), so it can't read a tree written for it", PREVIOUS);
        return ExitCode::from(1);
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
            "--answers" | "--format" => k += 2,
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
    if files.is_empty() {
        eprintln!("nothing to move: {} holds no document or pack's file", targets.join(", "));
        return ExitCode::SUCCESS;
    }
    // the tree's own packs' files are read with it; the other packs are what it is built with
    let packs = match super::options(&[targets.clone(), build.clone()].concat()) {
        Ok((_, p, _)) => p,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(2);
        }
    };
    let mut sources = Vec::new();
    for f in &files {
        match std::fs::read_to_string(f) {
            Ok(text) => sources.push(Source { path: f.clone(), text }),
            Err(e) => {
                eprintln!("{}: {}", f, e);
                return ExitCode::from(2);
            }
        }
    }
    let others: Vec<Source> = packs.iter().filter(|p| !files.iter().any(|f| same(f, &p.path))).cloned().collect();
    let read = match reader.read(&sources, &others) {
        Ok(r) => r,
        Err(e) => {
            for e in e {
                eprintln!("error: {}", e);
            }
            eprintln!("nothing written: {} doesn't take the tree as it is, and a migration starts from a tree valid for the release it is written for", reader.release());
            return ExitCode::from(1);
        }
    };
    let base = crate::check::compile_core(&super::core_sources());
    let compiles = |printed: &[Source]| compile_all(&base, &others, printed);
    let step = if read.release == RELEASE { None } else { step };
    let plan = pipeline::plan(read, step, answers, &compiles);

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
    let done = ExitCode::from(if plan.for_a_person() { FOR_A_PERSON } else { 0 });
    if dry || diff || check {
        return if check && !writes.is_empty() && !plan.for_a_person() { ExitCode::from(1) } else { done };
    }
    if writes.is_empty() {
        return done;
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
                eprintln!("written: {} file{}{}", writes.len(), if writes.len() == 1 { "" } else { "s" }, if plan.for_a_person() { format!(", with {} TODO{} for a person: look at each before relying on the files", plan.todos.len(), if plan.todos.len() == 1 { "" } else { "s" }) } else { String::new() });
            }
            done
        }
        Err(e) => {
            eprintln!("error: {}; nothing written", e);
            ExitCode::from(1)
        }
    }
}

/// Each file printed, compiled by this xtmd with the packs printed beside it in place of their old
/// files: its resolved IR and a document's Markdown, or its errors.
pub fn compile_all(base: &crate::check::Checker, packs: &[Source], printed: &[Source]) -> BTreeMap<String, Result<Checked, Vec<String>>> {
    let new_packs: Vec<Source> = printed.iter().filter(|s| !is_doc(&s.path)).cloned().collect();
    let mut world: Vec<Source> = packs.iter().filter(|p| !new_packs.iter().any(|n| same(&n.path, &p.path))).cloned().collect();
    world.extend(new_packs);
    let mut out = BTreeMap::new();
    for file in printed {
        let doc = is_doc(&file.path);
        let others: Vec<Source> = world.iter().filter(|p| p.path != file.path).cloned().collect();
        let (c, rir) = crate::check::compile_rir(base, &[others], &[file.clone()], if doc { "doc" } else { "main" });
        let errors: Vec<String> = c.diagnostics.iter().filter(|d| d.diagnostic.is_error()).map(|d| d.diagnostic.message.to_string()).collect();
        let entry = rir.as_ref().and_then(|r| r["files"].as_array()?.iter().find(|f| f["path"] == file.path.as_str()).cloned());
        let result = match (c.module, entry) {
            (Some(mut m), Some(rir)) if errors.is_empty() => {
                let markdown = if doc {
                    crate::lower::lower(&mut m);
                    match crate::markdown::page(&m) {
                        Ok(b) => Some(crate::markdown::write_page(m.front_matter.as_deref(), &b)),
                        Err(e) => {
                            out.insert(file.path.clone(), Err(vec![format!("its Markdown can't be written: {}", e.0)]));
                            continue;
                        }
                    }
                } else {
                    None
                };
                Ok(Checked { rir, markdown })
            }
            _ => Err(if errors.is_empty() { vec!["it gave no resolved IR".into()] } else { errors }),
        };
        out.insert(file.path.clone(), result);
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
        for w in &plan.warnings {
            line(serde_json::json!({ "type": "warning", "message": w }));
        }
        for r in &plan.refused {
            line(serde_json::json!({ "type": "refused", "message": r }));
        }
        line(serde_json::json!({ "type": "summary", "from": plan.from_release, "from_rir": plan.from_rir, "to": RELEASE, "to_rir": crate::migrate::rir::VERSION, "files": writes.len(), "ready": plan.ready(), "todos": plan.todos.len(), "warnings": plan.warnings.len() }));
        return;
    }
    eprintln!("read as written for {} (rir {}), moved to {} (rir {})", plan.from_release, plan.from_rir, RELEASE, crate::migrate::rir::VERSION);
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
    for w in &plan.warnings {
        eprintln!("  warning: {}", w);
    }
    for r in &plan.refused {
        eprintln!("  refused: {}", r);
    }
    if plan.ready() && plan.for_a_person() {
        eprintln!("{} TODO{} left for a person: migrating moves what is plain, and these it couldn't", plan.todos.len(), if plan.todos.len() == 1 { "" } else { "s" });
    }
    if !plan.ready() {
        eprintln!("nothing written: {}", if plan.refused.is_empty() { "a question is open" } else { "every file is written, or none" });
    }
}
