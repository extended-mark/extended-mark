//! B1: a `let` at the top of a file is a constant, worked out before the page runs. One whose value
//! reads the page (`$state`, the host's values, a fn the host gives, or a `let` or fn of the tree that
//! does) becomes a fn with no parameters, and each read of it a call.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::Resolved;

pub fn running(v: &mut Value, known: &Resolved) {
    // what reads the page, by id, until nothing more is found
    let mut lets: Vec<(String, Value)> = Vec::new();
    let mut fns: Vec<(String, Value)> = Vec::new();
    for f in v["files"].as_array().into_iter().flatten() {
        for d in f["declarations"].as_array().into_iter().flatten() {
            if let Some(l) = d.get("let") {
                lets.push((l["id"].as_str().unwrap_or("").to_string(), l["value"].clone()));
            }
            if let Some(f) = d.get("fn") {
                fns.push((f["id"].as_str().unwrap_or("").to_string(), f["body"].clone()));
            }
        }
    }
    let mut running: BTreeSet<String> = known.externs.iter().cloned().collect();
    let mut changed = true;
    while changed {
        changed = false;
        for (id, value) in lets.iter().chain(&fns) {
            if !running.contains(id) && reads_the_page(value, &running) {
                running.insert(id.clone());
                changed = true;
            }
        }
    }
    let moved: BTreeSet<String> = lets.iter().map(|(id, _)| id.clone()).filter(|id| running.contains(id)).collect();
    if moved.is_empty() {
        return;
    }
    for f in v["files"].as_array_mut().into_iter().flatten() {
        for d in f.get_mut("declarations").and_then(Value::as_array_mut).into_iter().flatten() {
            let Some(l) = d.get("let").filter(|l| l["id"].as_str().map_or(false, |id| moved.contains(id))).cloned() else { continue };
            let id = l["id"].as_str().unwrap_or("");
            let mut f = json!({
                "id": id,
                "name": id.rsplit('/').next().unwrap_or(id),
                "params": [],
                "returns": l["type"],
                "body": [{ "return": { "value": l["value"] }, "span": l["span"] }],
                "span": l["span"],
            });
            for k in ["name_span", "comments"] {
                if let Some(x) = l.get(k) {
                    f[k] = x.clone();
                }
            }
            *d = json!({ "fn": f });
        }
        calls(f, &moved);
    }
}

/// Whether an expression or a body reads what is only known when the page runs.
fn reads_the_page(v: &Value, running: &BTreeSet<String>) -> bool {
    match v {
        Value::Object(o) => {
            if let Some(g) = o.get("get") {
                if g.get("state").is_some() || g.get("extern").is_some() || g["let"].as_str().map_or(false, |l| running.contains(l)) {
                    return true;
                }
            }
            if o.get("call").and_then(|c| c["fn"].as_str()).map_or(false, |f| running.contains(f)) {
                return true;
            }
            o.iter().any(|(k, x)| k != "comments" && reads_the_page(x, running))
        }
        Value::Array(a) => a.iter().any(|x| reads_the_page(x, running)),
        _ => false,
    }
}

/// Each read of a moved `let` a call of the fn it became.
fn calls(v: &mut Value, moved: &BTreeSet<String>) {
    match v {
        Value::Object(o) => {
            let read = o.get("get").and_then(|g| g["let"].as_str()).filter(|l| moved.contains(*l)).map(String::from);
            if let Some(id) = read {
                o.remove("get");
                o.insert("call".into(), json!({ "fn": id, "args": [] }));
            }
            for (k, x) in o.iter_mut() {
                if k != "comments" {
                    calls(x, moved);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| calls(x, moved)),
        _ => {}
    }
}
