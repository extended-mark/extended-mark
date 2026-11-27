//! The step from 0.3.2, rir 1, to 0.4.0, rir 2: plan #2443's section 3. Its format part (format.rs)
//! brings rir 1's shape to rir 2's, every comment in its place; its language part moves what 0.4.0's
//! language changed, a row of the plan a file. Every part works on the JSON, by ids and types, as
//! 0.3.2 resolved them; nothing reads the text.

use serde_json::Value;

use crate::migrate::rir::Asked;

mod externs;
mod format;
mod lets;
mod mutable;
mod names;
mod slots;
mod texts;
mod views;
mod walk;

pub use format::format;

/// rir 1's schema, with what 0.3.2's front end gives beside it: `left_out`, a file's.
pub const SCHEMA: &str = include_str!("rir-1.schema.json");

/// What 0.3.2 resolved that the language part needs and rir 1 doesn't say, gathered from every
/// file's `left_out` before the format part takes it off.
#[derive(Default)]
pub struct Resolved {
    /// Each fn with a `mut` parameter, by its id: `{ "self": bool, "params": [bool] }`.
    pub fns: serde_json::Map<String, Value>,
    /// Each widget's `mut` input, by its id.
    pub inputs: Vec<String>,
    /// Each fn the host gives, by its id.
    pub externs: Vec<String>,
    /// Each widget's slots, by the widget's id, in order: `{ "name": …, "type": … }`.
    pub slots: serde_json::Map<String, Value>,
}

/// The step from 0.3.2: rir 1's shape brought to rir 2's, then what 0.4.0's language changed, a row
/// of plan #2443's section 3 a function.
pub fn step(v: &mut Value, asked: &mut Asked) -> Result<(), Vec<String>> {
    let mut known = Resolved::default();
    for f in v["files"].as_array().into_iter().flatten() {
        let m = &f["left_out"]["resolved"];
        for (k, x) in m["fns"].as_object().into_iter().flatten() {
            known.fns.insert(k.clone(), x.clone());
        }
        for (k, x) in m["slots"].as_object().into_iter().flatten() {
            known.slots.insert(k.clone(), x.clone());
        }
        for (list, key) in [(&mut known.inputs, "inputs"), (&mut known.externs, "externs")] {
            for i in m[key].as_array().into_iter().flatten().filter_map(Value::as_str) {
                if !list.iter().any(|x| x == i) {
                    list.push(i.to_string());
                }
            }
        }
    }
    format(v, asked)?;
    names::keywords(v, asked);
    mutable::fields(v, &known, asked);
    lets::running(v, &known);
    externs::markdown(v);
    slots::filled(v, &known, asked);
    views::blocks(v, &known, asked);
    texts::rich(v, asked);
    Ok(())
}
