//! B2: a record's field is written only when it is `mut`. Every place 0.3.2 let a page write (an
//! assignment, a `mut` parameter's argument, a `mut` input's, what a `mut for` walks, what an `if (mut
//! …)` opens) names the fields on its way by id; each of those the tree declares is made `mut`. One of
//! another pack's records can't be changed here: that write is left for a person, naming the pack.

use std::collections::BTreeMap;

use serde_json::Value;

use super::walk::{self, At};
use super::Resolved;
use crate::migrate::rir::Asked;

pub fn fields(v: &mut Value, known: &Resolved, asked: &mut Asked) {
    // every field written, by id, with where each write is
    let mut written: BTreeMap<String, Vec<At>> = BTreeMap::new();
    walk::each(v, &mut |n, at| {
        let mut places: Vec<&Value> = Vec::new();
        if let Some(s) = n.get("set") {
            places.push(&s["place"]);
        }
        if let Some(f) = n.get("for").filter(|f| f["mutable"] == true) {
            places.push(&f["over"]);
        }
        if let Some(o) = n.get("opens").filter(|o| o["mutable"] == true) {
            places.push(&o["value"]);
        }
        if let Some(c) = n.get("call") {
            if let Some(sig) = c["fn"].as_str().and_then(|f| known.fns.get(f)) {
                if sig["self"] == true {
                    places.extend(c.get("self"));
                }
                let flags = sig["params"].as_array().cloned().unwrap_or_default();
                for (a, m) in c["args"].as_array().into_iter().flatten().zip(flags) {
                    if m == true {
                        places.push(a.get("value").unwrap_or(a));
                    }
                }
            }
        }
        if let Some(u) = n.get("use") {
            for a in u["args"].as_array().into_iter().flatten() {
                if a["target"].as_str().map_or(false, |t| known.inputs.iter().any(|i| i == t)) {
                    places.push(&a["value"]);
                }
            }
        }
        for p in places {
            let mut ids = Vec::new();
            fields_on_the_way(p, &mut ids);
            for id in ids {
                written.entry(id).or_default().push(at.clone());
            }
        }
    });
    let mut todos = Vec::new();
    for (id, ats) in written {
        if make_mut(v, &id) {
            continue;
        }
        let (record, field) = id.rsplit_once('.').unwrap_or((&id, ""));
        let pack = record.split('/').next().unwrap_or(record);
        for at in ats {
            let what = format!("`{}` of `{}` is written here, and 0.4.0 writes only a field declared `mut`: it is pack {}'s, so make a new value with `with {{ {} = … }}`, or ask for it to be `mut` there", field, record, pack, field);
            todos.push((at.file, at.holder, what));
        }
    }
    walk::apply(v, todos, asked);
}

/// The ids of the fields a place goes through: `a.b.c` is `b` of `a`'s record and `c` of `b`'s.
fn fields_on_the_way(p: &Value, out: &mut Vec<String>) {
    if let Some(f) = p.get("field") {
        let id = f["member"].as_str().map(String::from).or_else(|| Some(format!("{}.{}", f["of"]["type"]["name"].as_str()?, f["field"].as_str()?)));
        if let Some(id) = id {
            out.push(id);
        }
        fields_on_the_way(&f["of"], out);
    } else if let Some(i) = p.get("index") {
        fields_on_the_way(&i["of"], out);
    }
}

/// The field `id` made `mut` where the tree declares its record; false when it doesn't.
fn make_mut(v: &mut Value, id: &str) -> bool {
    let mut found = false;
    for file in v["files"].as_array_mut().into_iter().flatten() {
        for d in file.get_mut("declarations").and_then(Value::as_array_mut).into_iter().flatten() {
            for f in d.pointer_mut("/record/fields").and_then(Value::as_array_mut).into_iter().flatten() {
                if f["id"] == id {
                    f["mutable"] = Value::Bool(true);
                    found = true;
                }
            }
        }
    }
    found
}
