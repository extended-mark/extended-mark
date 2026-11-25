//! `xtmd migrate page.ir.json`: an IR an earlier xtmd wrote, brought to the version this one's engines
//! run, in place. It is written only when its Markdown still can be, and what stops it is named.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::cli::out;

struct Found {
    path: PathBuf,
    old: String,
    from: i64,
    /// The IR of this version, or why it can't be.
    new: Result<String, Vec<String>>,
}

/// One IR file's text, brought here.
fn bring(text: &str) -> (i64, Result<String, Vec<String>>) {
    let from = serde_json::from_str(text).ok().as_ref().and_then(crate::migrate::ir::version).unwrap_or(0);
    let new = crate::migrate::ir::migrate(text).and_then(|(v, _)| {
        if from == crate::migrate::ir::VERSION {
            return Ok(text.to_string());
        }
        // over several lines when it was
        Ok(if text.trim_end().contains('\n') { serde_json::to_string_pretty(&v).unwrap() } else { v.to_string() } + if text.ends_with('\n') { "\n" } else { "" })
    });
    (from, new)
}

pub fn run(files: &[String], json: bool, dry: bool, check: bool, stdout: bool, yes: bool) -> ExitCode {
    if stdout && (files.len() != 1 || json) {
        eprintln!("--stdout prints one file's result: name one .json file{}", if json { ", and leave out --format json" } else { "" });
        return ExitCode::from(2);
    }
    let mut found = Vec::new();
    for f in files {
        let Ok(old) = std::fs::read_to_string(f) else {
            eprintln!("{}: can't be read", f);
            return ExitCode::from(2);
        };
        let (from, new) = bring(&old);
        found.push(Found { path: PathBuf::from(f), old, from, new });
    }
    let refused = found.iter().filter(|f| f.new.is_err()).count();
    let moves: Vec<&Found> = found.iter().filter(|f| f.new.as_ref().map_or(false, |n| *n != f.old)).collect();
    if stdout {
        return match &found[0].new {
            Ok(text) => {
                out::print(text);
                ExitCode::SUCCESS
            }
            Err(why) => {
                why.iter().for_each(|w| eprintln!("{}: {}", found[0].path.display(), w));
                ExitCode::from(1)
            }
        };
    }
    let now = crate::migrate::ir::VERSION;
    let say = |s: String| {
        if !json {
            out::text(&s);
        }
    };
    for f in &found {
        let p = f.path.display();
        match &f.new {
            Err(why) => {
                say(format!("{}  not written:", p));
                why.iter().for_each(|w| say(format!("  {}", w)));
            }
            Ok(n) if *n == f.old => say(format!("{}  IR {}, up to date", p, now)),
            Ok(_) => say(format!("{}  IR {} → IR {}", p, f.from, now)),
        }
    }
    let mut wrote = 0;
    let mut code = if refused > 0 { 1 } else { 0 };
    if !moves.is_empty() {
        let go = if dry || check {
            code = code.max(check as u8);
            false
        } else if yes {
            true
        } else if std::io::stdin().is_terminal() && !json {
            print!("This rewrites {} file{} in place. Continue? [y/N] ", moves.len(), if moves.len() == 1 { "" } else { "s" });
            let _ = std::io::stdout().flush();
            let mut answer = String::new();
            let _ = std::io::stdin().read_line(&mut answer);
            let go = matches!(answer.trim(), "y" | "Y" | "yes");
            if !go {
                say("Nothing written.".into());
                code = 1;
            }
            go
        } else {
            say("Nothing written: without a terminal to ask in, xtmd migrate writes only with --yes. --dry-run shows what it would do.".into());
            code = 1;
            false
        };
        if go {
            for f in &moves {
                if let Err(e) = out::write_file(&f.path.display().to_string(), f.new.as_ref().unwrap()) {
                    eprintln!("{}", e);
                    return ExitCode::from(2);
                }
                wrote += 1;
            }
            say(format!("Wrote {} file{}.", wrote, if wrote == 1 { "" } else { "s" }));
        }
    }
    if json {
        for f in &found {
            let (result, why) = match &f.new {
                Err(w) => ("refused", w.clone()),
                Ok(n) if *n == f.old => ("up to date", Vec::new()),
                Ok(_) if wrote > 0 => ("written", Vec::new()),
                Ok(_) => ("would change", Vec::new()),
            };
            out::line(serde_json::json!({ "type": "file", "file": f.path.display().to_string(), "from": f.from, "to": now, "result": result, "why": why }));
        }
        out::line(serde_json::json!({ "type": "summary", "files": found.len(), "refused": refused, "written": wrote, "exit": code }));
    }
    ExitCode::from(code)
}
