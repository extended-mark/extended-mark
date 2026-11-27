//! The `xtmd` command.

use std::process::ExitCode;

mod migrate;
mod migrate_ir;
pub mod migrate_rir;
mod examples;
mod batch;
mod flags;
mod help;
mod out;
mod engine;
pub(crate) use crate::migrate::step as migrate_step;
mod surface;

use crate::syntax::{entry_for, format, parse, Diagnostic};

use crate::live::CORE;

pub(crate) fn core_sources() -> Vec<crate::check::Source> {
    CORE.iter().map(|(p, t)| crate::check::Source { path: p.to_string(), text: t.to_string() }).collect()
}

/// The packs `--pack` names: a comma-separated list of folders and `.xtmdpack` files. A folder gives
/// every `.xtmdpack` file under it; each file says its own pack with its `package` line.
fn pack_sources(list: &str) -> Result<Vec<crate::check::Source>, String> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
        let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir).map_err(|e| format!("{}: {}", dir.display(), e))?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out)?;
            } else if p.extension().map_or(false, |x| x == "xtmdpack") {
                out.push(p);
            }
        }
        Ok(())
    }
    let mut paths = Vec::new();
    for item in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let p = std::path::Path::new(item);
        if p.is_dir() {
            walk(p, &mut paths)?;
        } else if item.ends_with(".xtmdpack") {
            paths.push(p.to_path_buf());
        } else {
            return Err(format!("--pack {}: not a folder or a .xtmdpack file", item));
        }
    }
    paths
        .into_iter()
        .map(|p| {
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {}", p.display(), e))?;
            Ok(crate::check::Source { path: p.to_string_lossy().into_owned(), text })
        })
        .collect()
}

/// `--pack a,b` and `--out file` taken out of the arguments, the files left.
/// What an `xtmd.toml` holds: the packs a document uses, as `--pack` takes them, relative to it.
#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default)]
    packs: Vec<String>,
    /// The release of Extended Mark the tree is written for.
    #[serde(default)]
    #[allow(dead_code)]
    xtmd: Option<String>,
    /// The version of each pack the tree is written for, as `xtmd migrate` reads it.
    #[serde(default)]
    #[allow(dead_code)]
    uses: std::collections::BTreeMap<String, u32>,
    /// The engines `xtmd export --engine` runs, each a command: `web = ["node", "engine.mjs"]`.
    #[serde(default)]
    engines: std::collections::BTreeMap<String, Vec<String>>,
}

fn read_config(path: &std::path::Path) -> Result<Config, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    toml::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e.message()))
}

/// The `xtmd.toml` for a file or folder: the one in its folder (a folder's own, for a folder) or in a
/// folder above it; for nothing named, the current folder's.
pub(crate) fn find_config(file: Option<&str>) -> Option<std::path::PathBuf> {
    let start = std::fs::canonicalize(file.unwrap_or(".")).ok()?;
    let skip = usize::from(!start.is_dir());
    start.ancestors().skip(skip).map(|d| d.join("xtmd.toml")).find(|p| p.is_file())
}

/// The packs of the `xtmd.toml` named, or else of the one `find_config` finds.
fn config_for(named: Option<&str>, file: Option<&String>) -> Result<Vec<crate::check::Source>, String> {
    let path = match named {
        Some(p) => Some(std::path::PathBuf::from(p)),
        None => find_config(file.map(String::as_str)),
    };
    let Some(path) = path else { return Ok(Vec::new()) };
    let config = read_config(&path)?;
    let dir = path.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let mut out = Vec::new();
    for p in &config.packs {
        let packs = pack_sources(&dir.join(p).to_string_lossy()).map_err(|e| format!("{}: packs = [\"{}\"]: {}", path.display(), p, e.split_once(": ").map_or(e.as_str(), |(_, why)| why)))?;
        out.extend(packs);
    }
    Ok(out)
}

/// A flag's value, as the next argument or after `=`.
fn value_of(args: &[String], flag: &str) -> Option<String> {
    args.iter().enumerate().rev().find_map(|(k, a)| if a == flag { args.get(k + 1).cloned() } else { a.strip_prefix(flag).and_then(|r| r.strip_prefix('=')).map(str::to_string) })
}

fn options(args: &[String]) -> Result<(Vec<String>, Vec<crate::check::Source>, Option<String>), String> {
    let (files, packs, config, out) = given(args)?;
    let all = all_packs(config.as_deref(), files.first(), packs)?;
    Ok((files, all, out))
}

/// The packs one of the files named is compiled with.
fn packs_for(args: &[String], file: &str) -> Result<Vec<crate::check::Source>, String> {
    let (_, packs, config, _) = given(args)?;
    all_packs(config.as_deref(), Some(&file.to_string()), packs)
}

/// The config's packs, then `--pack`'s; a file named twice is read once.
fn all_packs(config: Option<&str>, file: Option<&String>, packs: Vec<crate::check::Source>) -> Result<Vec<crate::check::Source>, String> {
    let mut all = config_for(config, file)?;
    all.extend(packs);
    let mut seen = std::collections::HashSet::new();
    all.retain(|s| seen.insert(std::fs::canonicalize(&s.path).unwrap_or_else(|_| s.path.clone().into())));
    Ok(all)
}

/// The files, `--pack`'s packs, `--config` and `--out`, as given.
fn given(args: &[String]) -> Result<(Vec<String>, Vec<crate::check::Source>, Option<String>, Option<String>), String> {
    let (mut files, mut packs, mut out) = (Vec::new(), Vec::new(), None);
    let mut config: Option<String> = None;
    let mut k = 0;
    while k < args.len() {
        match args[k].as_str() {
            "--out" | "--pack" | "--config" | "--format" => {
                let Some(v) = args.get(k + 1) else { return Err(format!("{} needs a value", args[k])) };
                match args[k].as_str() {
                    "--out" => out = Some(v.clone()),
                    "--pack" => packs.extend(pack_sources(v)?),
                    "--format" => {}
                    _ => config = Some(v.clone()),
                }
                k += 2;
            }
            a if a.starts_with("--pack=") => {
                packs.extend(pack_sources(&a["--pack=".len()..])?);
                k += 1;
            }
            a if a.starts_with("--out=") => {
                out = Some(a["--out=".len()..].to_string());
                k += 1;
            }
            a if a.starts_with("--config=") => {
                config = Some(a["--config=".len()..].to_string());
                k += 1;
            }
            a if flags::is_flag(a) => k += 1,
            _ => {
                files.push(args[k].clone());
                k += 1;
            }
        }
    }
    Ok((files, packs, config, out))
}

/// The packs without the file itself, however each names it: a relative path given, and the
/// same file found through an xtmd.toml's absolute one, are one file.
fn without(packs: &[crate::check::Source], path: &str) -> Vec<crate::check::Source> {
    let me = std::fs::canonicalize(path).ok();
    packs.iter().filter(|p| std::path::Path::new(&p.path) != std::path::Path::new(path) && (me.is_none() || std::fs::canonicalize(&p.path).ok() != me)).cloned().collect()
}


/// The Extended Mark files a folder holds, documents and packs' files, at any depth.
fn under(dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<std::path::PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if p.is_dir() {
            if !name.starts_with('.') && !matches!(name, "node_modules" | "target" | "dist") {
                under(&p, out);
            }
        } else if entry_for(&p.to_string_lossy()).is_some() {
            out.push(p.to_string_lossy().into_owned());
        }
    }
}

/// The files named, each folder as the Extended Mark files under it.
fn expand(files: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for f in files {
        if std::path::Path::new(f).is_dir() {
            under(std::path::Path::new(f), &mut out);
        } else {
            out.push(f.clone());
        }
    }
    out
}

/// A command that takes one file, given more: the usage error it is.
fn one_file(cmd: &str, files: &[String]) -> Result<(), ExitCode> {
    if files.len() > 1 {
        eprintln!("xtmd {} takes one file, and was given {}: {}", cmd, files.len(), files.join(", "));
        return Err(ExitCode::from(2));
    }
    Ok(())
}

/// The surfaces a pack's file has released: `surfaces/` in its pack's folder (the file's, or the
/// one above `migrations/`), so a change that breaks one is an error.
fn released_surfaces(path: &str) -> Vec<crate::check::Source> {
    let file = std::path::Path::new(path);
    let Some(mut dir) = file.parent() else { return Vec::new() };
    if dir.file_name().map_or(false, |n| n == "migrations" || n == "surfaces") {
        dir = dir.parent().unwrap_or(dir);
    }
    let surfaces = dir.join("surfaces");
    if !surfaces.is_dir() {
        return Vec::new();
    }
    pack_sources(&surfaces.to_string_lossy()).unwrap_or_default()
}

/// The codes of the errors a missing pack brings with it: its names, looked for and not found.
const FOLLOWS_A_MISSING_PACK: &[&str] = &["names.unknown-type", "names.unknown-widget", "names.unknown-fn", "names.unknown-name", "names.unknown-case"];

/// A pack the build doesn't have: its error says how to add it, and the errors that follow from it
/// in the same file are left out until it is found, its message counting them.
fn missing_packs(c: &mut crate::check::Compiled) {
    let files: std::collections::HashSet<String> = c.diagnostics.iter().filter(|d| d.diagnostic.code == "names.unknown-pack").map(|d| d.file.clone()).collect();
    if files.is_empty() {
        return;
    }
    let mut left: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    c.diagnostics.retain(|d| {
        let follows = files.contains(&d.file) && FOLLOWS_A_MISSING_PACK.contains(&d.diagnostic.code);
        if follows {
            *left.entry(d.file.clone()).or_default() += 1;
        }
        !follows
    });
    for d in c.diagnostics.iter_mut().filter(|d| d.diagnostic.code == "names.unknown-pack") {
        let name = d.diagnostic.message.split('`').nth(1).unwrap_or("it").to_string();
        if d.diagnostic.fix.is_none() {
            d.diagnostic.fix = Some(format!("pass its folder with --pack (--pack {} for ./{}), or list it in xtmd.toml: packs = [\"{}\"]", name, name, name));
        }
        if let Some(n) = left.remove(&d.file) {
            d.diagnostic.message = format!("{} ({} more error{} that follow from it are left out until it is found)", d.diagnostic.message, n, if n == 1 { "" } else { "s" });
        }
    }
}

/// What `xtmd compile` writes: as small as it goes, or over several lines with `--pretty`.
fn json_text<T: serde::Serialize>(v: &T, pretty: bool) -> String {
    if pretty { serde_json::to_string_pretty(v) } else { serde_json::to_string(v) }.unwrap()
}

/// The levels a compiled document needs, by name.
fn levels_of(m: &crate::formats::ir::Module) -> Vec<String> {
    m.levels.iter().filter_map(|l| serde_json::to_value(l).ok()).filter_map(|v| v.as_str().map(str::to_string)).collect()
}

fn plural(n: usize, one: &str) -> String {
    format!("{} {}{}", n, one, if n == 1 { "" } else { "s" })
}

/// A file's IR, as `xtmd compile` writes it; its diagnostics are reported, and what stops it is its exit code.
fn build_ir(base: &crate::check::Checker, path: &str, packs: &[crate::check::Source], json: bool) -> Result<crate::formats::ir::Module, u8> {
    let mut m = compile_file(base, path, packs, json, true)?;
    crate::lower::lower(&mut m);
    Ok(m)
}

/// A file compiled, its errors (and its own warnings, when asked) reported.
fn compile_file(base: &crate::check::Checker, path: &str, packs: &[crate::check::Source], json: bool, warnings: bool) -> Result<crate::formats::ir::Module, u8> {
    let text = match read(path) {
        Ok((t, _)) => t,
        Err(e) => {
            eprintln!("{}", e);
            return Err(2);
        }
    };
    let module = if migrate::is_doc(path) { "doc" } else { "main" };
    let mut c = crate::check::compile_on(base, &[without(packs, path)], &[crate::check::Source { path: path.to_string(), text }], module);
    missing_packs(&mut c);
    report_found(&c, path, json, warnings);
    c.module.ok_or(1)
}

fn report_found(c: &crate::check::Compiled, path: &str, json: bool, warnings: bool) {
    for d in c.diagnostics.iter().filter(|d| d.diagnostic.is_error() || (warnings && d.file == path)) {
        if json {
            eprintln!("{}", out::diagnostic(&d.file, &c.texts[&d.file], &d.diagnostic));
        } else {
            eprintln!("{}", d.diagnostic.render(&d.file, &c.texts[&d.file]));
        }
    }
}

/// What `build` or `md` makes of one file, written where `--out` says or printed.
fn write_one(made: Result<String, u8>, out: Option<String>, print: impl Fn(&str)) -> ExitCode {
    match (made, out) {
        (Err(c), _) => ExitCode::from(c),
        (Ok(text), Some(o)) => match out::write_file(&o, text) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{}", e);
                ExitCode::from(1)
            }
        },
        (Ok(text), None) => {
            print(&text);
            ExitCode::SUCCESS
        }
    }
}

/// `xtmd export <files|folders>… --out dir --engine e`: the files an engine makes of the documents. Without
/// an engine there is nothing to export: the IR is `xtmd compile`'s.
fn export(args: &[String], engine: Option<(String, Vec<String>)>) -> ExitCode {
    if engine.is_none() {
        let named: Vec<&str> = positional(args).iter().map(|s| s.as_str()).collect();
        eprintln!("xtmd export runs an engine: --engine <name>, after xtmd's own flags. The IR a runtime runs is xtmd compile's: xtmd compile {}", if named.is_empty() { "page.xtmd".to_string() } else { named.join(" ") });
        return ExitCode::from(2);
    }
    compile(args, engine)
}

/// A command a release moved: its name, the release that moved it, and where it went. It runs nothing and
/// says where to go, for that release only; the next one removes it, by the table in
/// ".github/pre-commit/Pre Commit checks.md", which `.github/pre-commit/check.mjs` holds to this list.
pub(crate) const MOVED: &[(&str, &str, &str)] = &[(
    "build",
    "0.4.0",
    "xtmd build moved in 0.4.0, and 0.5.0 removes it. The files an engine makes: xtmd export <files> --out dir --engine <name>. The IR a runtime runs: xtmd compile <files>",
)];

/// `xtmd compile <files|folders>… [--pack a,b] [--rir [--core]] [--pretty] [--out out]`: each document as
/// one standalone IR, core and the packs in it; with `--rir`, the resolved IR of the document and its packs
/// instead. `xtmd export` comes here with its engine, for the IR it gives that engine.
fn compile(args: &[String], engine: Option<(String, Vec<String>)>) -> ExitCode {
    let (files, _, _, out) = match given(args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(2);
        }
    };
    if files.is_empty() {
        eprintln!("{}", help::of(if engine.is_some() { "export" } else { "compile" }).unwrap());
        return ExitCode::from(2);
    }
    let rir = args.iter().any(|a| a == "--rir");
    let pretty = args.iter().any(|a| a == "--pretty");
    let with_core = args.iter().any(|a| a == "--core");
    if with_core && !rir {
        eprintln!("--core adds core's files to the resolved IR: it goes with --rir");
        return ExitCode::from(2);
    }
    let json = match json_or_exit(args) {
        Ok(j) => j,
        Err(c) => return c,
    };
    // core compiled with what its names resolve to kept, so its files are written resolved too
    let base = std::cell::OnceCell::new();
    let base = || base.get_or_init(|| if with_core { crate::check::compile_core_logged(&core_sources()) } else { crate::check::compile_core(&core_sources()) });
    let one = |path: &str, packs: &[crate::check::Source]| -> Result<String, u8> {
        if !rir {
            return build_ir(base(), path, packs, json).map(|m| json_text(&crate::formats::ir::File { ir: m }, pretty));
        }
        let text = read(path).map(|x| x.0).map_err(|e| {
            eprintln!("{}", e);
            2
        })?;
        let module = if migrate::is_doc(path) { "doc" } else { "main" };
        let opts = crate::check::RirOptions { core: with_core, partial: false };
        let (mut c, r) = crate::check::compile_rir_with(base(), &[without(packs, path)], &[crate::check::Source { path: path.to_string(), text }], module, opts);
        missing_packs(&mut c);
        report_found(&c, path, json, true);
        // the resolved IR: what the checker understood, for tools, and what the document needs
        c.module.ok_or(1)?;
        r.map(|r| json_text(&r, pretty)).ok_or(1)
    };
    if let Some(e) = engine {
        return engine::build(&files, args, e, out, json, |path, packs| build_ir(base(), path, packs, json));
    }
    if batch::wanted(&files, out.as_deref()) {
        return match batch::jobs("compile", &files, out.as_deref(), if rir { "rir.json" } else { "ir.json" }) {
            Ok(jobs) => batch::run(&jobs, args, json, one),
            Err(e) => {
                eprintln!("{}", e);
                ExitCode::from(2)
            }
        };
    }
    match packs_for(args, &files[0]) {
        Ok(packs) => write_one(one(&files[0], &packs), out, out::text),
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::from(2)
        }
    }
}

/// `xtmd decompile <file.rir.json> [--out dir | --stdout | --diff | --check]`: each file of a resolved IR
/// printed back as source, in the canonical layout, at the path it names (under `--out`'s folder when
/// given); `--stdout` prints them, `--diff` and `--check` say how the files there would change.
fn decompile(args: &[String]) -> ExitCode {
    let named = positional(args);
    if named.len() != 1 {
        eprintln!("{}", help::of("decompile").unwrap());
        return ExitCode::from(2);
    }
    let path = named[0].as_str();
    let json = match json_or_exit(args) {
        Ok(j) => j,
        Err(c) => return c,
    };
    let has = |f: &str| args.iter().any(|a| a == f);
    let (stdout, diff, check) = (has("--stdout"), has("--diff"), has("--check"));
    let value: serde_json::Value = match std::fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}: {}", path, e);
            return ExitCode::from(2);
        }
    };
    // an IR, as xtmd compile writes it, in the resolved IR's shape first; what it has lost is said once
    let value = if value.get("ir").is_some() {
        let stem = std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or("page");
        let stem = stem.strip_suffix(".json").unwrap_or(stem);
        let stem = stem.strip_suffix(".ir").unwrap_or(stem);
        let module = value["ir"]["module"].as_str().unwrap_or("doc");
        let file = if module == "doc" { format!("{}.xtmd", stem) } else { format!("{}.xtmdpack", stem) };
        let dir = std::path::Path::new(path).parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        let file = if dir.is_empty() || out::out_path(args).is_some() { file } else { format!("{}/{}", dir, file) };
        match crate::decompile::ir::from_ir(&value, &file) {
            Ok(f) => {
                for l in &f.losses {
                    if json {
                        eprintln!("{}", serde_json::json!({ "type": "warning", "code": "decompile.lost", "message": l }));
                    } else {
                        eprintln!("warning: what an IR doesn't keep is written anew: {}", l);
                    }
                }
                f.rir
            }
            Err(e) => {
                eprintln!("error: {}: {}", path, e);
                return ExitCode::from(1);
            }
        }
    } else {
        value
    };
    let printed = match crate::decompile::decompile(&value) {
        Ok(p) => p,
        Err(errors) => {
            for e in errors {
                eprintln!("error: {}", e);
            }
            return ExitCode::from(1);
        }
    };
    let out_dir = out::out_path(args);
    let mut differ = false;
    for p in &printed {
        let target = match &out_dir {
            Some(d) => std::path::Path::new(d).join(p.path.trim_start_matches('/')).to_string_lossy().into_owned(),
            None => p.path.clone(),
        };
        if stdout {
            out::print(&p.text);
            continue;
        }
        let had = std::fs::read_to_string(&target).ok();
        let same = had.as_deref() == Some(p.text.as_str());
        differ |= !same;
        if diff || check {
            if diff && !same {
                out::print(&crate::migrate::unified(&target, had.as_deref().unwrap_or(""), &p.text));
            }
            if json {
                out::line(serde_json::json!({ "type": "file", "file": target, "result": if same { "same" } else { "would change" } }));
            } else if check && !same {
                eprintln!("{}: the resolved IR prints back differently", target);
            }
            continue;
        }
        if let Err(e) = out::write_file(&target, &p.text) {
            eprintln!("{}", e);
            return ExitCode::from(1);
        }
        if json {
            out::line(serde_json::json!({ "type": "file", "file": target, "result": "written" }));
        }
    }
    if check && differ {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn read(path: &str) -> Result<(String, crate::syntax::Entry), String> {
    if std::path::Path::new(path).is_dir() {
        return Err(format!("{}: a folder", path));
    }
    let entry = entry_for(path).ok_or_else(|| format!("{}: not an Extended Mark file (.xtmdpack or .xtmd)", path))?;
    let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path, e))?;
    Ok((src, entry))
}

fn report(path: &str, src: &str, diags: &[Diagnostic], json: bool) {
    for d in diags {
        if json {
            eprintln!("{}", out::diagnostic(path, src, d));
        } else {
            eprintln!("{}", d.render(path, src));
        }
    }
}

/// `--format`'s choice, or the usage error it is.
fn json_or_exit(args: &[String]) -> Result<bool, ExitCode> {
    out::wants_json(args).map_err(|e| {
        eprintln!("{}", e);
        ExitCode::from(2)
    })
}

/// The arguments that aren't flags, and not the value of one that takes a value.
fn positional(args: &[String]) -> Vec<&String> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < args.len() {
        match args[k].as_str() {
            "--format" | "--out" | "--pack" | "--config" => k += 2,
            a if a.starts_with('-') && a.len() > 1 => k += 1,
            _ => {
                out.push(&args[k]);
                k += 1;
            }
        }
    }
    out
}

/// `xtmd fmt --stdin path`: stdin's text in the canonical layout on stdout, or with --check and
/// --diff, whether and how it would change; the file at `path` is never read or written.
fn fmt_text(path: &str, check: bool, diff: bool, json: bool) -> ExitCode {
    let Some(entry) = entry_for(path) else {
        eprintln!("{}: not an Extended Mark file (.xtmdpack or .xtmd): --stdin's path says what the text is", path);
        return ExitCode::from(2);
    };
    let mut src = String::new();
    if let Err(e) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut src) {
        eprintln!("stdin: {}", e);
        return ExitCode::from(2);
    }
    let line = |result: &str, more: serde_json::Value| {
        let mut v = serde_json::json!({ "type": "file", "file": path, "result": result });
        if let serde_json::Value::Object(m) = more {
            v.as_object_mut().unwrap().extend(m);
        }
        out::line(v);
    };
    match format(&src, entry) {
        Ok(text) if check || diff => {
            let same = text == src;
            if diff && !same {
                let d = migrate::unified(path, &src, &text);
                if json {
                    line("would change", serde_json::json!({ "diff": d }));
                } else {
                    out::print(&d);
                }
            } else if json {
                line(if same { "canonical" } else { "would change" }, serde_json::json!({}));
            } else if !same {
                eprintln!("{}: not in the canonical layout", path);
            }
            if check && !same {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Ok(text) => {
            if json {
                line("printed", serde_json::json!({ "text": text }));
            } else {
                out::print(&text);
            }
            ExitCode::SUCCESS
        }
        Err(diags) => {
            report(path, &src, &diags, json);
            if json {
                line("failed", serde_json::json!({}));
            }
            ExitCode::from(1)
        }
    }
}

fn fmt(args: &[String]) -> ExitCode {
    let check = args.iter().any(|a| a == "--check");
    let diff = args.iter().any(|a| a == "--diff");
    // --out alone: one file's canonical text, written there, the file left as it was
    let copy = out::to_a_file() && !check && !diff && !args.iter().any(|a| a == "--stdout");
    let stdout = args.iter().any(|a| a == "--stdout") || copy;
    let json = match json_or_exit(args) {
        Ok(j) => j,
        Err(c) => return c,
    };
    // --stdin: the text comes from stdin, and the one path says what kind of file it is (it needn't
    // exist): for an editor, whose text isn't saved
    let stdin = args.iter().any(|a| a == "--stdin");
    if stdin {
        let named = positional(args);
        if named.len() != 1 || std::path::Path::new(named[0]).is_dir() {
            eprintln!("--stdin formats one text: name the file it is for (xtmd fmt --stdin page.xtmd < page.xtmd)");
            return ExitCode::from(2);
        }
        return fmt_text(named[0], check, diff, json);
    }
    let files = expand(&positional(args).into_iter().cloned().collect::<Vec<_>>());
    if files.is_empty() {
        eprintln!("{}", help::of("fmt").unwrap());
        return ExitCode::from(2);
    }
    if stdout && files.len() != 1 {
        if copy {
            eprintln!("--out takes one file's canonical text: name one file, or add --check or --diff for a report on several");
        } else {
            eprintln!("--stdout prints one file in the canonical layout: name one file");
        }
        return ExitCode::from(2);
    }
    // what fmt says of each file: on stderr, so --stdout and --diff print only the text; with --out
    // and --check, in the report
    let say = |line: &str| if diff || stdout { eprintln!("{}", line) } else { out::note(line) };
    let (mut failed, mut unread) = (false, false);
    // with --format json, what became of each file: canonical, formatted, would change, printed, or failed
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    let mut tell = |file: &str, result: &'static str, more: serde_json::Value| {
        *counts.entry(result).or_default() += 1;
        if json {
            let mut v = serde_json::json!({ "type": "file", "file": file, "result": result });
            if let serde_json::Value::Object(m) = more {
                v.as_object_mut().unwrap().extend(m);
            }
            out::line(v);
        }
    };
    for path in &files {
        let (src, entry) = match read(path) {
            Ok(x) => x,
            Err(e) => {
                if !json {
                    eprintln!("{}", e);
                }
                tell(path, "not read", serde_json::json!({ "why": e }));
                unread = true;
                continue;
            }
        };
        match format(&src, entry) {
            // --stdout: the file in the canonical layout, written nowhere (or to --out's file)
            Ok(text) if stdout => {
                if json {
                    tell(path, "printed", serde_json::json!({ "text": text }));
                } else {
                    out::print(&text);
                }
            }
            Ok(text) if text == src => tell(path, "canonical", serde_json::json!({})),
            Ok(text) if diff => {
                let d = migrate::unified(path, &src, &text);
                if !json {
                    out::print(&d);
                }
                tell(path, "would change", serde_json::json!({ "diff": d }));
                failed |= check;
            }
            Ok(text) => {
                if check {
                    if !json {
                        say(&format!("{}: not in the canonical layout (xtmd fmt {} rewrites it)", path, path));
                    }
                    tell(path, "would change", serde_json::json!({}));
                    failed = true;
                } else if let Err(e) = std::fs::write(path, text) {
                    if !json {
                        eprintln!("{}: {}", path, e);
                    }
                    tell(path, "not written", serde_json::json!({ "why": e.to_string() }));
                    failed = true;
                } else {
                    if !json {
                        say(&format!("{}: formatted", path));
                    }
                    tell(path, "formatted", serde_json::json!({}));
                }
            }
            Err(diags) => {
                if json {
                    for d in &diags {
                        out::line(out::diagnostic(path, &src, d));
                    }
                } else {
                    report(path, &src, &diags, false);
                }
                tell(path, "doesn't read", serde_json::json!({}));
                failed = true;
            }
        }
    }
    let code: u8 = if unread { 2 } else if failed { 1 } else { 0 };
    if json {
        let mut v = serde_json::json!({ "type": "summary", "files": files.len(), "exit": code });
        for (k, n) in &counts {
            v[k.replace(' ', "_").replace('\'', "")] = serde_json::json!(n);
        }
        out::line(v);
    } else if !stdout {
        let parts: Vec<String> = counts.iter().map(|(k, n)| format!("{} {}", n, if *k == "canonical" { "already canonical" } else { k })).collect();
        say(&format!("{}: {}", plural(files.len(), "file"), parts.join(", ")));
    }
    ExitCode::from(code)
}

fn parse_cmd(args: &[String]) -> ExitCode {
    // its syntax tree is JSON whatever --format says; --format json makes its diagnostics JSON too
    let json = match json_or_exit(args) {
        Ok(j) => j,
        Err(c) => return c,
    };
    let files: Vec<String> = positional(args).into_iter().cloned().collect();
    if let Err(c) = one_file("parse", &files) {
        return c;
    }
    let Some(path) = files.first() else {
        eprintln!("{}", help::of("parse").unwrap());
        return ExitCode::from(2);
    };
    let (src, entry) = match read(path) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(2);
        }
    };
    let parsed = parse(&src, entry);
    report(path, &src, &parsed.diagnostics, json);
    let errors = parsed.has_errors();
    match parsed.file {
        Some(f) => {
            out::text(&serde_json::to_string_pretty(&f).unwrap());
            if errors {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        None => ExitCode::from(1),
    }
}

fn check(args: &[String]) -> ExitCode {
    let json = match json_or_exit(args) {
        Ok(j) => j,
        Err(c) => return c,
    };
    let (named, packs, _) = match options(args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(2);
        }
    };
    let files = expand(&named);
    if files.is_empty() {
        if named.is_empty() {
            eprintln!("{}", help::of("check").unwrap());
        } else {
            eprintln!("{}: no Extended Mark files (.xtmdpack or .xtmd) in it", named.join(", "));
        }
        return ExitCode::from(2);
    }
    let (mut failed, mut unread) = (false, 0);
    let (mut errors, mut warnings, mut with_errors) = (0, 0, 0);
    let base = crate::check::compile_core(&core_sources());
    for path in &files {
        let (src, _) = match read(path) {
            Ok(x) => x,
            Err(e) => {
                if json {
                    out::line(serde_json::json!({ "type": "file", "file": path, "result": "not read", "why": e }));
                } else {
                    out::note(&e);
                }
                unread += 1;
                continue;
            }
        };
        // each file with core and the packs: a document, or a pack's file with the rest of its pack
        // and the surfaces it has released
        let module = if migrate::is_doc(path) { "doc" } else { "main" };
        let mut packs = without(&packs, path);
        if module == "main" {
            let mut seen: std::collections::HashSet<std::path::PathBuf> = packs.iter().filter_map(|s| std::fs::canonicalize(&s.path).ok()).collect();
            packs.extend(released_surfaces(path).into_iter().filter(|s| std::fs::canonicalize(&s.path).map_or(true, |p| seen.insert(p))));
            packs = without(&packs, path);
        }
        let mut c = crate::check::compile_on(&base, &[packs.clone()], &[crate::check::Source { path: path.to_string(), text: src.clone() }], module);
        missing_packs(&mut c);
        failed |= c.module.is_none();
        // a pack's migrations: each example run through its block, once the file compiles
        if c.module.is_some() && module == "main" {
            crate::cli::migrate_step::NO_ASKING.store(true, std::sync::atomic::Ordering::Relaxed);
            for d in examples::check(&base, path, &src, &packs) {
                failed = true;
                c.diagnostics.push(crate::check::Found { file: path.to_string(), diagnostic: d });
            }
        }
        // this file's errors and warnings, and any pack's errors
        let (mut e, mut w) = (0, 0);
        for found in c.diagnostics.iter().filter(|d| d.file == *path || d.diagnostic.is_error()) {
            let (file, d) = (&found.file, &found.diagnostic);
            let text = c.texts.get(file.as_str()).map(String::as_str).unwrap_or(src.as_str());
            if d.is_error() {
                e += 1;
            } else {
                w += 1;
            }
            if json {
                out::line(out::diagnostic(file, text, d));
            } else {
                // what check reports is its diagnostics: with --out, they go to the file
                out::note(&d.render(file, text));
            }
        }
        let levels = c.module.as_ref().filter(|_| module == "doc").map(levels_of);
        if json {
            let mut v = serde_json::json!({ "type": "file", "file": path, "result": if e > 0 { "errors" } else if w > 0 { "warnings" } else { "clean" }, "errors": e, "warnings": w });
            if let Some(l) = &levels {
                v["levels"] = serde_json::json!(l);
            }
            out::line(v);
        } else {
            let verdict = if e + w == 0 { "clean".to_string() } else { format!("{}, {}", plural(e, "error"), plural(w, "warning")) };
            let needs = levels.as_ref().filter(|l| !l.is_empty()).map_or(String::new(), |l| format!("; needs {}", l.join(", ")));
            out::note(&format!("{}: {}{}", path, verdict, needs));
        }
        with_errors += usize::from(e > 0);
        errors += e;
        warnings += w;
    }
    let code: u8 = if unread > 0 { 2 } else if failed { 1 } else { 0 };
    if json {
        out::line(serde_json::json!({ "type": "summary", "files": files.len(), "errors": errors, "warnings": warnings, "with_errors": with_errors, "unreadable": unread, "exit": code }));
    } else if files.len() > 1 {
        let read_note = if unread > 0 { format!(", {} not read", unread) } else { String::new() };
        out::note(&format!("{}: {} with errors, {} without{}; {}, {}", plural(files.len(), "file"), with_errors, files.len() - with_errors - unread, read_note, plural(errors, "error"), plural(warnings, "warning")));
    }
    ExitCode::from(code)
}

/// `xtmd md <files|folders>… [--pack a,b] [--format json] [--out out]`: each page as Markdown, with its
/// starting values; `--format json` gives its blocks as the checks read them.
fn md(args: &[String]) -> ExitCode {
    let json = match json_or_exit(args) {
        Ok(j) => j,
        Err(c) => return c,
    };
    let (files, _, _, out) = match given(args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(2);
        }
    };
    if files.is_empty() {
        eprintln!("{}", help::of("md").unwrap());
        return ExitCode::from(2);
    }
    let base = std::cell::OnceCell::new();
    let base = || base.get_or_init(|| crate::check::compile_core(&core_sources()));
    let one = |path: &str, packs: &[crate::check::Source]| -> Result<String, u8> {
        let mut m = compile_file(base(), path, packs, json, false)?;
        crate::lower::lower(&mut m);
        let blocks = crate::markdown::page(&m).map_err(|e| {
            eprintln!("{}: writing its Markdown failed: {}", path, e.0);
            1
        })?;
        Ok(if json { serde_json::to_string_pretty(&crate::markdown::blocks_json(&blocks)).unwrap() + "\n" } else { crate::markdown::write_page(m.front_matter.as_deref(), &blocks) })
    };
    if batch::wanted(&files, out.as_deref()) {
        return match batch::jobs("md", &files, out.as_deref(), if json { "blocks.json" } else { "md" }) {
            Ok(jobs) => batch::run(&jobs, args, json, one),
            Err(e) => {
                eprintln!("{}", e);
                ExitCode::from(2)
            }
        };
    }
    match packs_for(args, &files[0]) {
        Ok(packs) => write_one(one(&files[0], &packs), out, out::print),
        Err(e) => {
            eprintln!("{}", e);
            ExitCode::from(2)
        }
    }
}

/// The commands, as `xtmd help` names them.
const COMMANDS: &[&str] = &["fmt", "check", "parse", "compile", "decompile", "export", "md", "surface", "migrate", "lsp"];

/// Flags written before the command (`xtmd --format json check …`), moved to after it, where each
/// command reads them.
fn command_first(args: Vec<String>) -> Vec<String> {
    let mut lead = Vec::new();
    let mut k = 0;
    while k < args.len() && flags::is_flag(&args[k]) && !matches!(args[k].as_str(), "--help" | "--version") {
        lead.push(args[k].clone());
        if flags::VALUED.contains(&args[k].as_str()) {
            if let Some(v) = args.get(k + 1) {
                lead.push(v.clone());
                k += 1;
            }
        }
        k += 1;
    }
    if lead.is_empty() || k >= args.len() {
        return args;
    }
    let mut out: Vec<String> = vec![args[k].clone()];
    out.extend(lead);
    out.extend(args[k + 1..].iter().cloned());
    out
}

fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        None | Some("--help") => {
            out::text(help::TOP);
            return ExitCode::SUCCESS;
        }
        Some("help") => {
            let name = args[1..].join(" ");
            if name.is_empty() {
                out::text(help::TOP);
                return ExitCode::SUCCESS;
            }
            let Some(h) = help::of(&name) else {
                eprintln!("xtmd has no command {}: the commands are {}", name, COMMANDS.join(", "));
                return ExitCode::from(2);
            };
            out::text(h);
            return ExitCode::SUCCESS;
        }
        Some("--version") => {
            out::text(&format!("xtmd {} (Extended Mark 0.4; IR 4, resolved IR {})", migrate::RELEASE, crate::check::RIR_VERSION));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    let (cmd, rest) = (args[0].as_str(), &args[1..]);
    // export's: what follows the engine's name is the engine's, and nothing here reads it
    let (own, engine) = if cmd == "export" { engine::split(rest) } else { (rest.to_vec(), None) };
    let rest = &own[..];
    // a command this release moved says where it went, --help too, and runs nothing
    if let Some((_, _, went)) = MOVED.iter().find(|(c, _, _)| *c == cmd) {
        eprintln!("{}", went);
        return ExitCode::from(2);
    }
    if !COMMANDS.contains(&cmd) {
        if flags::is_flag(cmd) {
            eprintln!("xtmd: {} isn't a command; a flag goes with one: xtmd <command> {} …", cmd, cmd);
        } else if cmd == "target" {
            eprintln!("xtmd target check left xtmd in 0.4.0: whoever writes a runtime, a renderer or an engine checks it");
        } else {
            eprintln!("xtmd has no command {}: the commands are {}", cmd, COMMANDS.join(", "));
        }
        eprintln!("{}", help::TOP);
        return ExitCode::from(2);
    }
    // --help: the command's own usage, and nothing run
    if rest.iter().any(|a| a == "--help") {
        out::text(help::of(cmd).unwrap());
        return ExitCode::SUCCESS;
    }
    // each command's flags: an unknown one is ignored with a warning, a dropped one stops it
    let (unknown, ok) = flags::check(cmd, rest);
    if !ok {
        return ExitCode::from(2);
    }
    let json = match json_or_exit(rest) {
        Ok(j) => j,
        Err(c) => return c,
    };
    for f in &unknown {
        let w = flags::warning(cmd, f);
        if json {
            eprintln!("{}", serde_json::json!({ "type": "warning", "code": "flag.unknown", "flag": f, "message": w }));
        } else {
            eprintln!("warning: {}", w);
        }
    }
    // a run that would write the person's files writes nothing with a flag it doesn't know
    if !unknown.is_empty() && flags::writes(cmd, rest) {
        eprintln!("Nothing written: xtmd {} doesn't know {}, so it stops before writing any file. Correct it and run it again.", cmd, unknown.join(", "));
        return ExitCode::from(2);
    }
    // --out: compile, export, md and surface write their document there; the rest, what they print
    if !matches!(cmd, "compile" | "decompile" | "export" | "md" | "surface") {
        if let Some(p) = out::out_path(rest) {
            out::to_file(&p);
        }
    }
    match cmd {
        "fmt" => fmt(rest),
        "parse" => parse_cmd(rest),
        "check" => check(rest),
        "compile" => compile(rest, None),
        "decompile" => decompile(rest),
        "export" => export(rest, engine),
        "md" => md(rest),
        "surface" => surface::surface_cmd(rest),
        "migrate" => migrate::migrate_cmd(rest),
        // the language server: the protocol over stdin and stdout, for an editor (compiler/xtmd/src/lsp)
        _ => ExitCode::from(crate::lsp::serve(std::io::stdin().lock(), std::io::stdout().lock()) as u8),
    }
}

pub fn main() -> ExitCode {
    let args = command_first(std::env::args().skip(1).collect());
    let code = run(&args);
    out::finish(code)
}
