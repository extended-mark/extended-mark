//! Puts what the crate embeds in it, from its own copy when it is packaged (the release puts the files in), else from
//! the repository: core's sources, so core is compiled with no files around it; core's table of HTML's named
//! characters, for the Markdown reader and the checker, which takes it as it is (a test holds it to what checking the
//! file gives); and checks/rules.json, where in the spec each diagnostic's rule is written.

use std::path::{Path, PathBuf};

/// A text literal's text.
fn decode(s: &str) -> String {
    let mut out = String::new();
    let mut cs = s.chars();
    while let Some(c) = cs.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match cs.next().expect("an escape") {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'u' => {
                let hex: String = cs.by_ref().skip(1).take_while(|c| *c != '}').collect();
                out.push(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).expect("a character"));
            }
            c @ ('\\' | '"' | '{' | '}') => out.push(c),
            c => panic!("\\{} in core/entities.xtmdpack", c),
        }
    }
    out
}

fn main() {
    // the compiler's, and only its: a crate without it embeds nothing
    if std::env::var_os("CARGO_FEATURE_COMPILER").is_none() {
        return;
    }
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let found = |path: &str| [here.join(path), here.join("../..").join(path)].into_iter().find(|p| p.exists()).unwrap_or_else(|| panic!("{}, in the crate or the repository", path));
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    core(&found("core"), &out);
    entities(&found("core/entities.xtmdpack"), &out);
    let rules = found("checks/rules.json");
    println!("cargo:rerun-if-changed={}", rules.display());
    println!("cargo:rustc-env=XTMD_RULES={}", rules.canonicalize().unwrap().display());
}

fn core(dir: &Path, dest: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().map_or(false, |x| x == "xtmdpack")).collect();
    files.sort();
    let mut out = String::from("/// core's sources, as (path, text).\npub const CORE: &[(&str, &str)] = &[\n");
    for f in files {
        println!("cargo:rerun-if-changed={}", f.display());
        out.push_str(&format!("    (\"core/{}\", include_str!({:?})),\n", f.file_name().unwrap().to_str().unwrap(), f.canonicalize().unwrap()));
    }
    out.push_str("];\n");
    // and what migrating from its earlier versions needs, which no compile of a page reads: each
    // released version's surface, and its migrations
    out.push_str("/// core's surfaces and migrations, as (path, text): only xtmd migrate reads them.\npub const CORE_HISTORY: &[(&str, &str)] = &[\n");
    for sub in ["surfaces", "migrations"] {
        let d = dir.join(sub);
        println!("cargo:rerun-if-changed={}", d.display());
        let mut files: Vec<PathBuf> = std::fs::read_dir(&d).map(|r| r.map(|e| e.unwrap().path()).filter(|p| p.extension().map_or(false, |x| x == "xtmdpack")).collect()).unwrap_or_default();
        files.sort();
        for f in files {
            println!("cargo:rerun-if-changed={}", f.display());
            out.push_str(&format!("    (\"core/{}/{}\", include_str!({:?})),\n", sub, f.file_name().unwrap().to_str().unwrap(), f.canonicalize().unwrap()));
        }
    }
    out.push_str("];\n");
    std::fs::write(dest.join("core.rs"), out).unwrap();
}

fn entities(file: &Path, dest: &Path) {
    println!("cargo:rerun-if-changed={}", file.display());
    println!("cargo:rustc-env=XTMD_ENTITIES={}", file.canonicalize().unwrap().display());
    let text = std::fs::read_to_string(file).unwrap();
    let mut all: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("{ key = \"") else { continue };
        let (key, rest) = rest.split_once("\", value = \"").expect("a value");
        all.push((key.to_string(), decode(rest.strip_suffix("\" },").expect("the line's end"))));
    }
    assert!(all.windows(2).all(|w| w[0].0 < w[1].0), "core/entities.xtmdpack is in order, each name once");
    let mut out = format!("pub static ENTITIES: [(&str, &str); {}] = [\n", all.len());
    for (k, v) in &all {
        out.push_str(&format!("    ({:?}, {:?}),\n", k, v));
    }
    out.push_str("];\n");
    std::fs::write(dest.join("entities.rs"), out).unwrap();
}
