//! `xtmd render`: a document drawn by an engine. The engine is a program: the IR on its standard input, what
//! it draws on its standard output, its errors on standard error. Everything after its name is its own.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use crate::{out, Config};

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
            return Err(format!("{}: [engines] {} is an empty list: it is the engine's command, `{} = [\"node\", \"render.mjs\"]`", path.unwrap().display(), name, name));
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
    Err(format!("no engine `{}`: {}. Add it under [engines] in xtmd.toml, `{} = [\"node\", \"render.mjs\"]`, or give a path: --engine ./{}", name, here, name, name))
}

pub(crate) fn render_cmd(args: &[String], engine: Option<(String, Vec<String>)>) -> ExitCode {
    let usage = |e: &str| {
        eprintln!("{}", e);
        ExitCode::from(2)
    };
    let (files, packs, out_path) = match crate::options(args) {
        Ok(x) => x,
        Err(e) => return usage(&e),
    };
    if let Err(c) = crate::one_file("render", &files) {
        return c;
    }
    let Some(path) = files.first() else { return usage(crate::help::of("render").unwrap()) };
    let Some((name, given)) = engine else {
        return usage("xtmd render needs --engine and the engine's name or path: xtmd has no engine of its own");
    };
    let config = crate::value_of(args, "--config");
    let (cmd, dir) = match command(&name, config.as_deref(), path) {
        Ok(x) => x,
        Err(e) => return usage(&e),
    };
    let json = match crate::json_or_exit(args) {
        Ok(j) => j,
        Err(c) => return c,
    };
    let base = xtmd_check::compile_core(&crate::core_sources());
    let ir = match crate::build_ir(&base, path, &packs, json, false) {
        Ok(t) => t,
        Err(c) => return ExitCode::from(c),
    };
    let mut command = Command::new(&cmd[0]);
    command.args(&cmd[1..]).args(&given).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    if let Some(d) = dir {
        command.current_dir(d);
    }
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => return usage(&format!("--engine {}: {} can't be run: {}", name, cmd[0], e)),
    };
    // the engine may draw as it reads, or stop before it has read it all
    let mut stdin = child.stdin.take().unwrap();
    let feed = std::thread::spawn(move || drop(stdin.write_all(ir.as_bytes())));
    let drawn = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => return usage(&format!("--engine {}: {}", name, e)),
    };
    let _ = feed.join();
    if !drawn.status.success() {
        match drawn.status.code() {
            Some(c) => {
                eprintln!("--engine {}: exited with {}", name, c);
                return ExitCode::from(u8::try_from(c).unwrap_or(1));
            }
            None => {
                eprintln!("--engine {}: {}", name, drawn.status);
                return ExitCode::from(1);
            }
        }
    }
    match out_path {
        Some(o) => {
            if let Err(e) = out::write_file(&o, drawn.stdout) {
                eprintln!("{}", e);
                return ExitCode::from(1);
            }
        }
        None => out::bytes(&drawn.stdout),
    }
    ExitCode::SUCCESS
}
