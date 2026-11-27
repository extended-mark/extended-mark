//! An IR written by an earlier xtmd, brought to the version this one's engines run: plain steps from
//! one version's JSON to the next's, one way, each told in order. What a step is given fits its version's
//! schema, kept as it was, and what it gives fits the next's. A page a step can't bring keeps its file,
//! and is told what is missing: it is built again from its source.

pub mod schema;
mod v3;

use serde_json::Value;

/// The IR's version this xtmd writes, a number of its own.
pub const VERSION: i64 = 4;

/// From a version to the next: the IR changed, or why it can't be.
pub struct Step {
    pub from: i64,
    /// What the version's IR fits.
    pub schema: &'static str,
    pub run: fn(&mut Value) -> Result<(), Vec<String>>,
}

pub static STEPS: &[Step] = &[Step { from: 3, schema: include_str!("ir-3.schema.json"), run: v3::step }];

/// What this version's IR fits: `schemas/ir.schema.json`.
const NOW: &str = include_str!("ir-4.schema.json");

/// The version of an IR file, or none when the JSON isn't one.
pub fn version(v: &Value) -> Option<i64> {
    v.get("ir")?.get("version")?.as_i64()
}

/// The IR of this version, and the version it was: or why it can't be brought here.
pub fn migrate(text: &str) -> Result<(Value, i64), Vec<String>> {
    migrate_said(text).map(|(v, from, _)| (v, from))
}

/// The IR of this version, the version it was, and what a person must look at before the page runs
/// on it: an IR file holds no comment, so what the step can't settle is said beside it.
pub fn migrate_said(text: &str) -> Result<(Value, i64, Vec<String>), Vec<String>> {
    let mut v: Value = serde_json::from_str(text).map_err(|e| vec![format!("it isn't JSON: {}", e)])?;
    let Some(from) = version(&v) else { return Err(vec!["it isn't an IR: `xtmd compile` writes { \"ir\": { \"version\": … } }".into()]) };
    if from > VERSION {
        return Err(vec![format!("it is IR {}, and this xtmd writes IR {}: use the xtmd that wrote it, or a later one", from, VERSION)]);
    }
    let mut at = from;
    while at < VERSION {
        let Some(s) = STEPS.iter().find(|s| s.from == at) else { return Err(vec![format!("it is IR {}, and no step leads from it: build the page again from its source", at)]) };
        fits(&v, s.schema, at)?;
        (s.run)(&mut v)?;
        at += 1;
        v["ir"]["version"] = at.into();
        fits(&v, STEPS.iter().find(|s| s.from == at).map_or(NOW, |s| s.schema), at)?;
    }
    if from == VERSION {
        fits(&v, NOW, VERSION)?;
    }
    // what it writes as Markdown is what a person would notice missing
    let mut file: crate::formats::ir::File = serde_json::from_value(v.clone()).map_err(|e| vec![e.to_string()])?;
    if file.ir.module == "doc" {
        crate::markdown::page(&file.ir).map_err(|e| {
            vec![match e.0.contains("core/rich_text.show") {
                true => "it shows a rich_text as text: IR 4 has nothing for it, so build the page again from its source, moved by `xtmd migrate`".to_string(),
                false => format!("its Markdown can't be written: {}", e.0),
            }]
        })?;
    }
    // what an earlier IR didn't tell: what is read, written and whole
    let mut said = Vec::new();
    if from < VERSION {
        said = drawn_by_packs(&v, from);
        crate::lower::tell(&mut file.ir);
        v = serde_json::to_value(&file).map_err(|e| vec![e.to_string()])?;
        fits(&v, NOW, VERSION)?;
    }
    Ok((v, from, said))
}

/// The extern widgets a pack's own renderer draws, which a step can't move: the renderer is the pack's,
/// written for the runtime of the IR's time, and the pack may have a later version the IR doesn't know.
fn drawn_by_packs(v: &Value, from: i64) -> Vec<String> {
    let mut by: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for d in v["ir"]["declarations"].as_array().into_iter().flatten() {
        let w = &d["declare"]["extern"]["widget"];
        if w["supplier"] == "pack" {
            let name = w["name"].as_str().unwrap_or("");
            by.entry(name.split('/').next().unwrap_or(name).to_string()).or_default().push(format!("`{}`", name));
        }
    }
    by.into_iter()
        .map(|(pack, ws)| format!("{} {} drawn by pack {}'s own renderer, as it was when IR {} was written: the page runs only where {} has a renderer for this runtime, and {} may have a later version since; build the page again from its source, moved by `xtmd migrate`, to be sure", ws.join(", "), if ws.len() == 1 { "is" } else { "are" }, pack, from, pack, pack))
        .collect()
}

fn fits(v: &Value, schema: &str, version: i64) -> Result<(), Vec<String>> {
    let s: Value = serde_json::from_str(schema).expect("a kept schema is JSON");
    match schema::check(v, &s) {
        None => Ok(()),
        Some(e) => Err(vec![format!("it doesn't fit IR {}'s schema, {}", version, e)]),
    }
}
