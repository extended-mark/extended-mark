//! `xtmd decompile`: every file the repository has, compiled to its resolved IR and printed back, compiles to the
//! same resolved IR, spans apart; and what is printed back is in `xtmd fmt`'s layout already.

use serde_json::Value;
use std::path::{Path, PathBuf};
use xtmd::check::{compile_core_logged, compile_rir, compile_rir_with, Compiled, RirOptions, Source};

fn ir(c: &Compiled) -> Value {
    serde_json::to_value(&c.module).unwrap()
}

/// A document's Markdown, as `xtmd md` writes it; nothing for a pack.
fn markdown(c: &Compiled) -> String {
    let Some(m) = &c.module else { return String::new() };
    let mut m = m.clone();
    xtmd::lower::lower(&mut m);
    match xtmd::markdown::page(&m) {
        Ok(blocks) => xtmd::markdown::write_page(m.front_matter.as_deref(), &blocks),
        Err(_) => String::new(),
    }
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap_or_else(|e| panic!("{}: {}", path.display(), e))
}

fn cases(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            cases(&p, out);
        } else if p.extension().map_or(false, |e| e == "json") {
            out.push(p);
        }
    }
}

fn sources(dir: &Path, names: &[&str]) -> Vec<Source> {
    fn add(p: &Path, out: &mut Vec<Source>) {
        if p.is_dir() {
            let mut files: Vec<PathBuf> = std::fs::read_dir(p).unwrap().map(|e| e.unwrap().path()).collect();
            files.sort();
            for f in files {
                add(&f, out);
            }
        } else if p.extension().map_or(false, |x| x == "xtmdpack") {
            out.push(Source { path: p.to_string_lossy().into_owned(), text: std::fs::read_to_string(p).unwrap() });
        }
    }
    let mut out = Vec::new();
    for n in names {
        add(&dir.join(n), &mut out);
    }
    out
}

/// A resolved IR without what says where its parts were written, which printing back changes.
fn unplaced(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.iter().filter(|(k, _)| !(k.as_str() == "span" || k.ends_with("_span") || k.as_str() == "sha256")).map(|(k, x)| (k.clone(), unplaced(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(unplaced).collect()),
        other => other.clone(),
    }
}

/// Where two values first differ, as a path into them.
fn first_difference(a: &Value, b: &Value, at: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            for k in keys {
                if let Some(d) = first_difference(x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null), &format!("{}.{}", at, k)) {
                    return Some(d);
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            for k in 0..x.len().max(y.len()) {
                if let Some(d) = first_difference(x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null), &format!("{}[{}]", at, k)) {
                    return Some(d);
                }
            }
            None
        }
        _ if a == b => None,
        _ => Some(format!("{}: {} became {}", at, short(a), short(b))),
    }
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 300 {
        format!("{}…", &s[..300])
    } else {
        s
    }
}

#[derive(Default)]
struct Tally {
    /// Set by `XTMD_DECOMPILE_DIFF`: each failure printed as it is found, and each file printed differently from
    /// how fmt prints its source kept in a folder to compare.
    verbose: bool,
    files: usize,
    same_text: usize,
    failures: Vec<String>,
}

impl Tally {
    /// Prints back each file of `rir`; gives the sources with those files' texts replaced by what was printed.
    fn print_back(&mut self, rir: &Value, all: &[Source]) -> Option<Vec<Source>> {
        let printed = match xtmd::decompile::decompile(rir) {
            Ok(p) => p,
            Err(e) => {
                self.fail(e.join("\n"));
                return None;
            }
        };
        let mut out = all.to_vec();
        for p in printed {
            self.files += 1;
            let Some(s) = out.iter_mut().find(|s| s.path == p.path) else {
                self.fail(format!("{}: printed back a file that isn't one of the sources", p.path));
                return None;
            };
            let entry = if p.path.ends_with(".xtmd") { xtmd::syntax::Entry::Document } else { xtmd::syntax::Entry::Xtmd };
            match xtmd::syntax::format(&p.text, entry) {
                Ok(again) if again == p.text => {}
                Ok(_) => self.fail(format!("{}: what decompile printed isn't in fmt's layout", p.path)),
                Err(_) => self.fail(format!("{}: what decompile printed doesn't read:\n{}", p.path, p.text)),
            }
            if xtmd::syntax::format(&s.text, entry).ok().as_deref() == Some(p.text.as_str()) {
                self.same_text += 1;
            } else if self.verbose {
                let dir = std::env::temp_dir().join("xtmd-decompile");
                let name = p.path.replace(['/', '\\'], "_");
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join(format!("{}.fmt", name)), xtmd::syntax::format(&s.text, entry).unwrap_or_default()).unwrap();
                std::fs::write(dir.join(format!("{}.out", name)), &p.text).unwrap();
            }
            s.text = p.text;
        }
        Some(out)
    }

    fn fail(&mut self, what: String) {
        if self.verbose {
            eprintln!("{}\n", what.chars().take(3000).collect::<String>());
        }
        self.failures.push(what);
    }

    /// The resolved IR, spans apart; and the IR a runtime runs and the page's Markdown, which say what the text
    /// means where the resolved IR keeps it as it is written.
    fn compare(&mut self, what: &str, before: &(Compiled, Option<Value>), after: (Compiled, Option<Value>)) {
        let (Some(b), Some(a)) = (&before.1, &after.1) else {
            let errors: Vec<String> = after.0.diagnostics.iter().filter(|d| d.diagnostic.is_error()).map(|d| format!("{}: {}", d.file, d.diagnostic.message)).collect();
            return self.fail(format!("{}: what decompile printed doesn't compile: {}", what, errors.join("; ")));
        };
        if let Some(d) = first_difference(&unplaced(b), &unplaced(a), "") {
            return self.fail(format!("{}: {}", what, d));
        }
        if let Some(d) = first_difference(&ir(&before.0), &ir(&after.0), "") {
            return self.fail(format!("{}: its IR: {}", what, d));
        }
        if markdown(&before.0) != markdown(&after.0) {
            self.fail(format!("{}: its Markdown was\n{}\nand is\n{}", what, markdown(&before.0), markdown(&after.0)));
        }
    }
}

#[test]
fn every_file_printed_back_from_its_resolved_ir_compiles_to_the_same_resolved_ir() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(round_trip).unwrap().join().unwrap();
}

fn round_trip() {
    let mut t = Tally { verbose: std::env::var("XTMD_DECOMPILE_DIFF").is_ok(), ..Tally::default() };
    let core = sources(&repo().join("core"), &["."]);
    let base = compile_core_logged(&core);
    let core_rir = |base| compile_rir_with(base, &[], &[], "t", RirOptions { core: true, partial: true });

    // core's own files, printed back and compiled as core
    let before = core_rir(&base);
    if let Some(back) = t.print_back(before.1.as_ref().unwrap(), &core) {
        let again = compile_core_logged(&back);
        t.compare("core", &before, core_rir(&again));
    }

    // each case of checks/cases and the runtimes' cases that compiles
    let mut all = Vec::new();
    cases(&repo().join("checks/cases"), &mut all);
    cases(&repo().join("runtime/typescript/test/cases"), &mut all);
    for case_path in all {
        let case = json(&case_path);
        let Some(source) = case["source"].as_str() else { continue };
        let dir = case_path.parent().unwrap();
        let names: Vec<&str> = case["packs"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let packs = sources(dir, &names);
        let path = dir.join(source);
        let name = path.to_string_lossy().into_owned();
        let main = vec![Source { path: name.clone(), text: std::fs::read_to_string(&path).unwrap() }];
        let module = if name.ends_with(".xtmd") { "doc" } else { "t" };
        let before = compile_rir(&base, &[packs.clone()], &main, module);
        let Some(rir) = &before.1 else { continue };
        let every: Vec<Source> = packs.iter().chain(main.iter()).cloned().collect();
        if let Some(back) = t.print_back(rir, &every) {
            let (p, m): (Vec<Source>, Vec<Source>) = back.into_iter().partition(|s| packs.iter().any(|x| x.path == s.path));
            t.compare(&name, &before, compile_rir(&base, &[p], &m, module));
        }
    }

    // the examples and apps: each folder's documents, with the folder's packs
    let mut dirs = Vec::new();
    fn folders(d: &Path, out: &mut Vec<PathBuf>) {
        let mut es: Vec<_> = std::fs::read_dir(d).unwrap().map(|e| e.unwrap().path()).collect();
        es.sort();
        if es.iter().any(|p| p.extension().map_or(false, |x| x == "xtmd" || x == "xtmdpack")) {
            out.push(d.to_path_buf());
        }
        for p in es {
            if p.is_dir() && p.file_name().map_or(true, |n| n != "node_modules") {
                folders(&p, out);
            }
        }
    }
    folders(&repo().join("examples"), &mut dirs);
    folders(&repo().join("apps"), &mut dirs);
    for d in dirs {
        let packs = sources(&d, &["."]);
        let mut docs: Vec<PathBuf> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().map_or(false, |x| x == "xtmd")).collect();
        docs.sort();
        let mut runs: Vec<(Vec<Source>, &str)> = docs.iter().map(|doc| (vec![Source { path: doc.to_string_lossy().into_owned(), text: std::fs::read_to_string(doc).unwrap() }], "doc")).collect();
        if runs.is_empty() {
            runs.push((Vec::new(), "t"));
        }
        for (main, module) in runs {
            let (with, mains) = if main.is_empty() { (Vec::new(), packs.clone()) } else { (packs.clone(), main) };
            let before = compile_rir(&base, &[with.clone()], &mains, module);
            let Some(rir) = &before.1 else { continue };
            let every: Vec<Source> = with.iter().chain(mains.iter()).cloned().collect();
            if let Some(back) = t.print_back(rir, &every) {
                let (p, m): (Vec<Source>, Vec<Source>) = back.into_iter().partition(|s| with.iter().any(|x| x.path == s.path));
                t.compare(&d.display().to_string(), &before, compile_rir(&base, &[p], &m, module));
            }
        }
    }

    eprintln!("decompile: {} files printed back, {} of them exactly as xtmd fmt prints their source", t.files, t.same_text);
    assert!(t.failures.is_empty(), "{} failures:\n{}", t.failures.len(), t.failures.join("\n\n"));
    assert!(t.files >= 100, "only {} files printed back", t.files);
}

fn xtmd(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_xtmd")).current_dir(dir).args(args).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn xtmd_decompile_writes_a_resolved_irs_files_back_where_it_is_told() {
    let dir = std::env::temp_dir().join(format!("xtmd-decompile-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let source = "// the reader's name\n$state {\n    name: text = \"Ada\";\n}\n@@@\n# Hello\n\nHello, \\{$state.name}.\n";
    std::fs::write(dir.join("page.xtmd"), source).unwrap();
    let (code, _, err) = xtmd(&dir, &["compile", "page.xtmd", "--rir", "--out", "page.rir.json"]);
    assert_eq!(code, 0, "{}", err);

    // printed, it is the source as fmt prints it
    let (code, out, err) = xtmd(&dir, &["decompile", "page.rir.json", "--stdout"]);
    assert_eq!((code, out.as_str()), (0, source), "{}", err);
    // where the file is, nothing would change; the source changed, --check says so and --diff shows how
    assert_eq!(xtmd(&dir, &["decompile", "page.rir.json", "--check"]).0, 0);
    std::fs::write(dir.join("page.xtmd"), source.replace("Hello,", "Hi,")).unwrap();
    assert_eq!(xtmd(&dir, &["decompile", "page.rir.json", "--check"]).0, 1);
    let (code, diff, _) = xtmd(&dir, &["decompile", "page.rir.json", "--diff"]);
    assert_eq!(code, 0);
    assert!(diff.contains("-Hi, \\{$state.name}.") && diff.contains("+Hello, \\{$state.name}."), "{}", diff);
    assert_eq!(std::fs::read_to_string(dir.join("page.xtmd")).unwrap(), source.replace("Hello,", "Hi,"), "--diff writes nothing");
    // --out puts each file under a folder, at the path the resolved IR names
    let (code, out, _) = xtmd(&dir, &["decompile", "page.rir.json", "--out", "back", "--format", "json"]);
    assert_eq!(code, 0);
    assert!(out.contains("\"written\""), "{}", out);
    assert_eq!(std::fs::read_to_string(dir.join("back/page.xtmd")).unwrap(), source);
    // without --out, at that path itself
    assert_eq!(xtmd(&dir, &["decompile", "page.rir.json"]).0, 0);
    assert_eq!(std::fs::read_to_string(dir.join("page.xtmd")).unwrap(), source);

    // what isn't a resolved IR is a usage error that says what is
    std::fs::write(dir.join("ir.json"), "{\"ir\": {}}").unwrap();
    let (code, _, err) = xtmd(&dir, &["decompile", "ir.json"]);
    assert_eq!(code, 1);
    assert!(err.contains("xtmd compile --rir"), "{}", err);
    let (code, _, _) = xtmd(&dir, &["decompile", "missing.json"]);
    assert_eq!(code, 2);
    let _ = std::fs::remove_dir_all(&dir);
}
