//! The crate's parts use one another one way only, as text goes through them: one that uses a part it mustn't is a
//! crate that can no longer be taken without it, or a circle.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// What each part may use of the others.
const USES: &[(&str, &[&str])] = &[
    ("formats", &[]),
    ("eval", &["formats"]),
    ("markdown", &["formats", "eval"]),
    ("syntax", &[]),
    ("lower", &["formats", "eval"]),
    ("check", &["formats", "eval", "syntax"]),
    ("decompile", &["syntax"]),
    ("live", &["formats", "syntax", "lower", "check"]),
    // a tree is moved by printing its resolved IR again; a value a migration writes is worked out by
    // running it
    ("migrate", &["formats", "eval", "markdown", "syntax", "lower", "check", "decompile", "live"]),
    ("lsp", &["formats", "syntax", "live"]),
    ("cli", &["formats", "markdown", "syntax", "lower", "check", "decompile", "live", "migrate", "lsp"]),
];

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// The parts a source names: `crate::check::Source`, `use crate::{check, eval::Num}`, comments and tests left out.
fn named(text: &str) -> BTreeSet<String> {
    let code = text.split("#[cfg(test)]").next().unwrap();
    let code: String = code.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    let word = |s: &str| s.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>();
    let mut out = BTreeSet::new();
    for (k, _) in code.match_indices("crate::") {
        let rest = &code[k + "crate::".len()..];
        match rest.strip_prefix('{') {
            Some(list) => out.extend(list[..list.find(';').unwrap_or(list.len())].split([',', '{', '}']).map(|x| word(x.trim()))),
            None => {
                out.insert(word(rest));
            }
        }
    }
    out
}

#[test]
fn each_part_uses_only_the_ones_before_it() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let uses: BTreeMap<&str, &[&str]> = USES.iter().copied().collect();
    let dirs: BTreeSet<String> = std::fs::read_dir(&src).unwrap().map(|e| e.unwrap()).filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(dirs, uses.keys().map(|k| k.to_string()).collect(), "every part is here");
    let mut wrong = Vec::new();
    for (part, may) in &uses {
        let mut all = Vec::new();
        files(&src.join(part), &mut all);
        for f in all {
            for n in named(&std::fs::read_to_string(&f).unwrap()) {
                if n != *part && dirs.contains(&n) && !may.contains(&n.as_str()) {
                    wrong.push(format!("{} uses {}", f.strip_prefix(&src).unwrap().display(), n));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn what_a_source_names_is_found() {
    let found = named("use crate::{check::{Source, Ty}, eval};\n// crate::lsp\nfn f() { crate::lower::lower(x) }\n#[cfg(test)]\nmod tests { use crate::cli; }");
    assert!(["check", "eval", "lower"].iter().all(|n| found.contains(*n)), "{:?}", found);
    assert!(!found.contains("lsp") && !found.contains("cli"));
}
