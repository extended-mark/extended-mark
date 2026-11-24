//! `xtmd build --engine`: documents built into files by an engine. The engine is a program: what is built on
//! its standard input, the files it makes on its standard output (schemas/build.schema.json), its errors on
//! standard error. Everything after its name is its own. It writes no file itself: xtmd does, into `--out`,
//! once it has ended well.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use xtmd_ir::build::{self, Built, Document, Job};

use crate::{out, plural, Config};

/// xtmd's own arguments, and the engine's: its name (or path), and what follows it, untouched.
pub(crate) fn split(args: &[String]) -> (Vec<String>, Option<(String, Vec<String>)>) {
    for (k, a) in args.iter().enumerate() {
        if a == "--engine" {
            let Some(name) = args.get(k + 1) else { break };
            return (args[..k].to_vec(), Some((name.clone(), args[k + 2..].to_vec())));
        }
        if let Some(name) = a.strip_prefix("--engine=") {
            return (args[..k].to_vec(), Some((name.to_string(), args[k + 1..].to_vec())));
        }
    }
    (args.to_vec(), None)
}

/// The engine as a command, and the folder it runs in: one of `xtmd.toml`'s `[engines]`, run beside that
/// file, or a path, run where xtmd is.
fn command(name: &str, named_config: Option<&str>, file: &str) -> Result<(Vec<String>, Option<PathBuf>), String> {
    let path = named_config.map(PathBuf::from).or_else(|| crate::find_config(Some(file)));
    let config = match &path {
        Some(p) => crate::read_config(p)?,
        None => Config::default(),
    };
    if let Some(list) = config.engines.get(name) {
        if list.is_empty() {
            return Err(format!("{}: [engines] {} is an empty list: it is the engine's command, `{} = [\"node\", \"engine.mjs\"]`", path.unwrap().display(), name, name));
        }
        let dir = path.and_then(|p| p.parent().map(|d| d.to_path_buf())).filter(|d| !d.as_os_str().is_empty());
        return Ok((list.clone(), dir));
    }
    if name.contains(['/', '\\']) {
        if !std::path::Path::new(name).is_file() {
            return Err(format!("--engine {}: no such file", name));
        }
        return Ok((vec![name.to_string()], None));
    }
    let known: Vec<&str> = config.engines.keys().map(String::as_str).collect();
    let here = match (&path, known.is_empty()) {
        (Some(p), false) => format!("{} has {}", p.display(), known.join(", ")),
        (Some(p), true) => format!("{} has none", p.display()),
        (None, _) => "there is no xtmd.toml".into(),
    };
    Err(format!("no engine `{}`: {}. Add it under [engines] in xtmd.toml, `{} = [\"node\", \"engine.mjs\"]`, or give a path: --engine ./{}", name, here, name, name))
}

/// The engine's own answer to what is built, its bytes, or the exit code it ends xtmd with.
fn answer(name: &str, cmd: &[String], given: &[String], dir: Option<PathBuf>, job: &Job) -> Result<Vec<u8>, ExitCode> {
    let usage = |e: String| {
        eprintln!("{}", e);
        ExitCode::from(2)
    };
    let mut command = Command::new(&cmd[0]);
    command.args(&cmd[1..]).args(given).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    if let Some(d) = dir {
        command.current_dir(d);
    }
    let mut child = command.spawn().map_err(|e| usage(format!("--engine {}: {} can't be run: {}", name, cmd[0], e)))?;
    let text = serde_json::to_string(job).unwrap();
    // the engine may work as it reads, or stop before it has read it all
    let mut stdin = child.stdin.take().unwrap();
    let feed = std::thread::spawn(move || drop(stdin.write_all(text.as_bytes())));
    let done = child.wait_with_output().map_err(|e| usage(format!("--engine {}: {}", name, e)))?;
    let _ = feed.join();
    if done.status.success() {
        return Ok(done.stdout);
    }
    match done.status.code() {
        Some(c) => eprintln!("--engine {}: exited with {}", name, c),
        None => eprintln!("--engine {}: {}", name, done.status),
    }
    Err(ExitCode::from(done.status.code().and_then(|c| u8::try_from(c).ok()).unwrap_or(1)))
}

/// Each file to write and its bytes, or what is wrong with the first that can't be.
fn files_of(answer: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let v: serde_json::Value = serde_json::from_slice(answer).map_err(|e| format!("what it gave isn't JSON: {}", e))?;
    if v["version"] != build::VERSION {
        return Err(format!("what it gave is version {}, and this xtmd reads version {}", v["version"], build::VERSION));
    }
    let built: Built = serde_json::from_value(v).map_err(|e| format!("what it gave isn't a list of files: {}", e))?;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for f in built.files {
        if !inside(&f.path) {
            return Err(format!("`{}` is no path under the folder it builds into", f.path));
        }
        if !seen.insert(f.path.clone()) {
            return Err(format!("`{}` is given twice", f.path));
        }
        let bytes = match (f.text, f.base64) {
            (Some(t), None) => t.into_bytes(),
            (None, Some(b)) => unbase64(&b).ok_or_else(|| format!("`{}`'s base64 doesn't read", f.path))?,
            _ => return Err(format!("`{}` has a text or a base64, one of the two", f.path)),
        };
        out.push((f.path, bytes));
    }
    // a file can't be a folder too
    if let Some(p) = seen.iter().find(|p| seen.iter().any(|q| q.starts_with(&format!("{}/", p)))) {
        return Err(format!("`{}` is a file and a folder", p));
    }
    Ok(out)
}

/// Whether a path stays in the folder it is under, on every system.
fn inside(path: &str) -> bool {
    !path.is_empty() && !path.contains(['\\', ':', '\0']) && path.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}

fn unbase64(s: &str) -> Option<Vec<u8>> {
    let s = s.trim_end_matches('=');
    let (mut out, mut held, mut bits) = (Vec::with_capacity(s.len() * 3 / 4), 0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        held = held << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((held >> bits) as u8);
            held &= (1 << bits) - 1;
        }
    }
    // what is left over is no byte, and nothing
    (bits < 6 && held == 0).then_some(out)
}

/// Builds each document and gives them all to the engine, then writes what it made. A document that doesn't
/// compile is left out, and said.
pub(crate) fn build(files: &[String], args: &[String], (name, given): (String, Vec<String>), out_dir: Option<String>, json: bool, mut one: impl FnMut(&str, &[xtmd_check::Source]) -> Result<xtmd_ir::ir::Module, u8>) -> ExitCode {
    let usage = |e: &str| {
        eprintln!("{}", e);
        ExitCode::from(2)
    };
    let Some(to) = out_dir else {
        return usage("xtmd build --engine needs --out and the folder to build into, before --engine");
    };
    if Path::new(&to).is_file() {
        return usage(&format!("--out {}: a file, and an engine builds into a folder", to));
    }
    let config = crate::value_of(args, "--config");
    let (cmd, dir) = match command(&name, config.as_deref(), &files[0]) {
        Ok(x) => x,
        Err(e) => return usage(&e),
    };
    // each document's place, as `--out` would have it
    let jobs = match crate::batch::jobs("build", files, Some("."), "") {
        Ok(j) => j,
        Err(e) => return usage(&e),
    };
    let mut documents = Vec::new();
    let mut code = 0;
    for j in &jobs {
        let made = match crate::packs_for(args, &j.file) {
            Ok(packs) => one(&j.file, &packs),
            Err(e) => return usage(&e),
        };
        match made {
            Ok(ir) => {
                let place = j.to.strip_prefix(".").unwrap_or(&j.to).to_string_lossy().replace('\\', "/");
                documents.push(Document { name: place.trim_end_matches('.').to_string(), ir });
            }
            Err(c) => code = code.max(c),
        }
    }
    if documents.is_empty() {
        return ExitCode::from(code);
    }
    let built = documents.len();
    let given = match answer(&name, &cmd, &given, dir, &Job { version: build::VERSION, documents }) {
        Ok(a) => a,
        Err(c) => return c,
    };
    let made = match files_of(&given) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("--engine {}: {}; nothing is written", name, e);
            return ExitCode::from(1);
        }
    };
    for (path, bytes) in &made {
        let at = Path::new(&to).join(path);
        if let Err(e) = out::write_file(&at.to_string_lossy(), bytes) {
            eprintln!("{}", e);
            return ExitCode::from(2);
        }
        if json {
            out::line(serde_json::json!({ "type": "file", "out": at, "bytes": bytes.len() }));
        } else {
            out::note(&format!("{} ({} bytes)", at.display(), bytes.len()));
        }
    }
    let left = jobs.len() - built;
    if json {
        out::line(serde_json::json!({ "type": "summary", "documents": jobs.len(), "built": built, "with_errors": left, "written": made.len(), "exit": code }));
    } else {
        let errors = if left > 0 { format!(", {} with errors", left) } else { String::new() };
        out::note(&format!("{}: {} built into {}{}", plural(jobs.len(), "document"), built, plural(made.len(), "file"), errors));
    }
    ExitCode::from(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_stays_in_its_folder() {
        for p in ["a.html", "a/b.js", ".well-known/x", "a b/ü.txt", "..."] {
            assert!(inside(p), "{p}");
        }
        for p in ["", "/a", "a/", "a//b", "../a", "a/../b", "./a", "a/.", "C:/a", "a\\b", "a\0b"] {
            assert!(!inside(p), "{p}");
        }
    }

    #[test]
    fn base64_is_bytes() {
        for (s, b) in [("", &b""[..]), ("AA==", b"\0"), ("iVBORw==", b"\x89PNG"), ("/+8=", b"\xff\xef"), ("YWJj", b"abc"), ("YWI", b"ab")] {
            assert_eq!(unbase64(s).as_deref(), Some(b), "{s}");
        }
        for s in ["A", "AB==", "a b=", "AAA*"] {
            assert_eq!(unbase64(s), None, "{s}");
        }
    }

    #[test]
    fn what_isnt_a_list_of_files_is_refused_whole() {
        let file = |x: &str| format!("{{\"version\":1,\"files\":[{}]}}", x);
        assert_eq!(files_of(file("{\"path\":\"a\",\"text\":\"x\"}").as_bytes()).unwrap(), vec![("a".to_string(), b"x".to_vec())]);
        for (given, why) in [
            ("<html>".to_string(), "isn't JSON"),
            ("{\"version\":2,\"files\":[]}".into(), "version 2"),
            ("{\"files\":[]}".into(), "version null"),
            ("{\"version\":1}".into(), "isn't a list of files"),
            (file("{\"path\":\"../a\",\"text\":\"x\"}"), "no path under"),
            (file("{\"path\":\"a\",\"text\":\"x\"},{\"path\":\"a\",\"text\":\"y\"}"), "given twice"),
            (file("{\"path\":\"a\",\"text\":\"x\"},{\"path\":\"a/b\",\"text\":\"y\"}"), "a file and a folder"),
            (file("{\"path\":\"a\"}"), "one of the two"),
            (file("{\"path\":\"a\",\"text\":\"x\",\"base64\":\"AA==\"}"), "one of the two"),
            (file("{\"path\":\"a\",\"base64\":\"!\"}"), "doesn't read"),
        ] {
            let e = files_of(given.as_bytes()).unwrap_err();
            assert!(e.contains(why), "{given}: {e}");
        }
    }
}
