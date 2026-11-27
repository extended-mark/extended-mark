//! A1: a slot is filled by every call unless it is `opt`, and a plain block goes only to a widget with
//! one slot. A plain block 0.3.2 gave a widget with several filled its first; it is named so. A slot a
//! call left out is given an empty block where it may be empty (any content, or a list with no least
//! length), and is left for a person where it may not.

use serde_json::{json, Value};

use super::walk;
use super::Resolved;
use crate::migrate::rir::Asked;

/// Whether a slot's type lets it hold nothing.
fn may_be_empty(ty: &Value) -> bool {
    match ty["name"].as_str() {
        Some("core/view") => true,
        Some("core/list") => match ty.pointer("/contract/length/low") {
            None | Some(Value::Null) => true,
            Some(low) => low.get("num").and_then(Value::as_str) == Some("0"),
        },
        _ => false,
    }
}

pub fn filled(v: &mut Value, known: &Resolved, asked: &mut Asked) {
    let mut todos: Vec<(usize, String, String)> = Vec::new();
    let n = v["files"].as_array().map_or(0, Vec::len);
    for i in 0..n {
        let mut at = Vec::new();
        calls(&mut v["files"][i], String::new(), String::new(), known, &mut at);
        todos.extend(at.into_iter().map(|(h, w)| (i, h, w)));
    }
    walk::apply(v, todos, asked);
}

/// Every widget's call below `v`, its slots named and filled: `holder` is the statement or prose node
/// that holds it, where a TODO goes.
fn calls(v: &mut Value, node: String, holder: String, known: &Resolved, todos: &mut Vec<(String, String)>) {
    match v {
        Value::Object(o) => {
            for key in ["use", "call"] {
                let Some(c) = o.get_mut(key).filter(|c| c.get("widget").is_some()) else { continue };
                let widget = c["widget"].as_str().unwrap_or("").to_string();
                let Some(slots) = known.slots.get(&widget).and_then(Value::as_array) else { continue };
                let span = c.get("span").cloned().unwrap_or(json!([0, 0]));
                let had = c.get("slots").is_some();
                let entries = c.as_object_mut().unwrap().entry("slots").or_insert_with(|| json!([])).as_array_mut().unwrap();
                // a plain block: the first slot's, and named when there are others
                for e in entries.iter_mut() {
                    if let Some(first) = slots.first().and_then(|s| s["name"].as_str()).filter(|_| e["slot"].as_str().map_or(false, |s| s.ends_with('.'))) {
                        e["slot"] = Value::String(format!("{}.{}", widget, first));
                    }
                    if slots.len() > 1 {
                        e.as_object_mut().map(|o| o.remove("children"));
                    }
                }
                let prose = key == "call";
                // a slot given an empty block goes before those the call fills, in the widget's order, as
                // the printed call has them
                let mut added = Vec::new();
                for s in slots {
                    let name = s["name"].as_str().unwrap_or("");
                    let id = format!("{}.{}", widget, name);
                    if s["type"]["name"] == "core/opt" || entries.iter().any(|e| e["slot"] == id.as_str()) {
                        continue;
                    }
                    if may_be_empty(&s["type"]) {
                        added.push(json!({ "slot": id, (if prose { "prose" } else { "view" }): [], "span": span }));
                    } else {
                        let short = widget.rsplit('/').next().unwrap_or(&widget);
                        todos.push((holder.clone(), format!("`{}` needs its slot `{}`, which this call leaves out: fill it, or make it `opt` in the widget", short, name)));
                    }
                }
                entries.splice(0..0, added);
                if !had && entries.is_empty() {
                    c.as_object_mut().map(|o| o.remove("slots"));
                }
            }
            for (k, x) in o.iter_mut() {
                if k == "comments" {
                    continue;
                }
                let here = format!("{}/{}", node, k.replace('~', "~0").replace('/', "~1"));
                if let Value::Array(a) = x {
                    for (i, y) in a.iter_mut().enumerate() {
                        let item = format!("{}/{}", here, i);
                        let h = if walk::holds(k, y) { item.clone() } else { holder.clone() };
                        calls(y, item, h, known, todos);
                    }
                } else {
                    calls(x, here, holder.clone(), known, todos);
                }
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter_mut().enumerate() {
                calls(x, format!("{}/{}", node, i), holder.clone(), known, todos);
            }
        }
        _ => {}
    }
}
