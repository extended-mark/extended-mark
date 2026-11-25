//! A call is checked the same wherever it is written: in code, and in a document's prose. Each rule
//! about a call's inputs and slots has a case on both paths, so one can't be checked on only one.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const CODES: &[&str] = &["views.missing-input", "views.unknown-input", "views.missing-slot", "views.children-need-one-slot", "views.unknown-slot", "views.slot-takes-its-type", "views.slot-length"];

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().map_or(false, |x| x == "json") {
            out.push(p);
        }
    }
}

#[test]
fn every_rule_about_a_call_has_a_case_in_code_and_one_in_prose() {
    let mut files = Vec::new();
    walk(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../checks/cases"), &mut files);
    let mut seen: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for f in files {
        let c: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        let Some(source) = c["source"].as_str() else { continue };
        let text = std::fs::read_to_string(f.parent().unwrap().join(source)).unwrap();
        let divider = text.lines().position(|l| l.trim() == "@@@");
        for d in c["expect"]["diagnostics"].as_array().into_iter().flatten() {
            let line = d["line"].as_u64().unwrap_or(0) as usize;
            // below a document's `@@@` line is prose, and a document with none is prose all through
            let prose = source.ends_with(".xtmd") && divider.map_or(true, |k| line > k + 1);
            seen.entry(d["code"].as_str().unwrap_or("").to_string()).or_default().insert(if prose { "prose" } else { "code" });
        }
    }
    for code in CODES {
        let on = seen.get(*code).cloned().unwrap_or_default();
        assert_eq!(on, BTreeSet::from(["code", "prose"]), "{code} has cases only in {on:?}");
    }
}
