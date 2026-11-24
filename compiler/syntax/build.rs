//! Reads core's table of HTML's named characters (core/entities.xtmdpack) into the crate, for the Markdown
//! reader and for the checker, which takes the table as it is: the crate's own copy when it is packaged
//! (the release puts core's files in), else the repository's. A test holds it to what checking the file gives.

use std::path::PathBuf;

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
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file = [here.join("core/entities.xtmdpack"), here.join("../../core/entities.xtmdpack")].into_iter().find(|p| p.is_file()).expect("core/entities.xtmdpack, in the crate or the repository");
    println!("cargo:rerun-if-changed={}", file.display());
    println!("cargo:rustc-env=XTMD_ENTITIES={}", file.canonicalize().unwrap().display());
    let text = std::fs::read_to_string(&file).unwrap();
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
    std::fs::write(PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("entities.rs"), out).unwrap();
}
