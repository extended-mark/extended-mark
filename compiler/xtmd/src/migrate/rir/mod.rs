//! A resolved IR an earlier release's reader wrote, brought to the version this xtmd's decompile
//! prints: plain steps from one version's JSON to the next's, one way, as for the IR (../ir). What a
//! step is given fits its version's schema, kept as it was (schemas/rir-history), and what it gives
//! fits the next's. What a step can't settle by itself it asks, by a question's id, answered in
//! `--answers`; what no answer can settle it leaves to a person, as a TODO above the node.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::migrate::ir::schema;

/// The resolved IR's version this xtmd prints.
pub const VERSION: u64 = crate::decompile::RIR_SHAPE;

/// From a version to the next.
pub struct Step {
    pub from: u64,
    /// What the version's resolved IR fits.
    pub schema: &'static str,
    /// The resolved IR changed in place, or why it can't be.
    pub run: fn(&mut Value, &mut Asked) -> Result<(), Vec<String>>,
}

/// The steps this xtmd takes. rir 1, 0.3's, to 2 comes with 0.3.2's reader, set aside (#14715).
pub static STEPS: &[Step] = &[];

/// What this version's resolved IR fits: `schemas/rir.schema.json`.
pub const NOW: &str = include_str!("rir-2.schema.json");

/// A question a step asks: its id, which an answers file names it by, and the answers it takes.
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    pub id: String,
    pub question: String,
    pub options: Vec<String>,
}

/// What the steps were told and what they leave: the answers given, the questions still open, and
/// the TODOs written.
#[derive(Default)]
pub struct Asked {
    pub answers: BTreeMap<String, String>,
    pub open: Vec<Question>,
    pub todos: Vec<String>,
}

impl Asked {
    pub fn with(answers: BTreeMap<String, String>) -> Self {
        Asked { answers, ..Asked::default() }
    }

    /// The answer to a question, one of its options; with none, or one it doesn't take, the question
    /// is kept open, and nothing is written.
    pub fn ask(&mut self, id: &str, question: &str, options: &[&str]) -> Option<String> {
        if let Some(a) = self.answers.get(id).filter(|a| options.is_empty() || options.contains(&a.as_str())) {
            return Some(a.clone());
        }
        if !self.open.iter().any(|q| q.id == id) {
            self.open.push(Question { id: id.to_string(), question: question.to_string(), options: options.iter().map(|o| o.to_string()).collect() });
        }
        None
    }

    /// A TODO for a person, above a node of `file`: a comment on the lines before it, which decompile
    /// writes there. The node is the one a list holds, `{ "let": { … } }`, or what it names.
    pub fn todo(&mut self, node: &mut Value, file: &str, what: &str) {
        let line = format!("// TODO(xtmd migrate): {}", what);
        let tagged = node.as_object().map_or(false, |o| o.len() == 1 && !o.contains_key("span") && o.values().all(Value::is_object));
        let node = if tagged { node.as_object_mut().unwrap().values_mut().next().unwrap() } else { node };
        if let Some(o) = node.as_object_mut() {
            let comments = o.entry("comments").or_insert_with(|| Value::Object(Default::default()));
            if let Some(c) = comments.as_object_mut() {
                let leading = c.entry("leading").or_insert_with(|| Value::Array(Vec::new()));
                if let Some(l) = leading.as_array_mut() {
                    l.push(Value::String(line));
                }
            }
        }
        self.todos.push(format!("{}: {}", file, what));
    }
}

/// The version of a resolved IR, or none when the JSON isn't one.
pub fn version(v: &Value) -> Option<u64> {
    v.get("rir")?.as_u64()
}

/// The resolved IR brought to `VERSION` through `steps`, each told what was answered; or why it can't
/// be. A question left open stops nothing here: the caller writes nothing while one is.
pub fn migrate(v: &mut Value, steps: &[Step], asked: &mut Asked) -> Result<u64, Vec<String>> {
    let Some(from) = version(v) else { return Err(vec!["it isn't a resolved IR: `xtmd compile --rir` writes { \"rir\": 2, \"files\": … }".into()]) };
    if from > VERSION {
        return Err(vec![format!("it is rir {}, and this xtmd prints rir {}: use a later xtmd", from, VERSION)]);
    }
    let mut at = from;
    while at < VERSION {
        let Some(s) = steps.iter().find(|s| s.from == at) else { return Err(vec![format!("it is rir {}, and no step leads from it to rir {}", at, at + 1)]) };
        fits(v, s.schema, at)?;
        (s.run)(v, asked)?;
        at += 1;
        v["rir"] = at.into();
        fits(v, steps.iter().find(|s| s.from == at).map_or(NOW, |s| s.schema), at)?;
    }
    if from == VERSION {
        fits(v, NOW, VERSION)?;
    }
    Ok(from)
}

fn fits(v: &Value, schema: &str, version: u64) -> Result<(), Vec<String>> {
    let s: Value = serde_json::from_str(schema).map_err(|e| vec![format!("rir {}'s schema isn't JSON: {}", version, e)])?;
    match schema::check(v, &s) {
        None => Ok(()),
        Some(e) => Err(vec![format!("it doesn't fit rir {}'s schema, {}", version, e)]),
    }
}
