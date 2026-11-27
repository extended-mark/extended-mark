//! A5 and B3: `warn`, `provides`, `uses`, `migration` and `execute` are keywords in 0.4.0. What the tree
//! declares with one of these names is renamed with `_`, `uses_`, and so is every reference to it, by
//! its id: a read, a call, a field, a named argument. A local is renamed where it is declared and read.
//! A field of `$state` or an exposed fn is renamed too, with a TODO: a host reads it by its name.

use std::collections::BTreeMap;

use serde_json::Value;

use super::walk;
use crate::migrate::rir::Asked;

const KEYWORDS: &[&str] = &["warn", "provides", "uses", "migration", "execute"];

fn keyword(name: &str) -> bool {
    KEYWORDS.contains(&name)
}

/// The id with its last name renamed, when that name is a keyword.
fn renamed(id: &str) -> Option<String> {
    let cut = id.rfind(['/', '.']).map_or(0, |k| k + 1);
    keyword(&id[cut..]).then(|| format!("{}_", id))
}

pub fn keywords(v: &mut Value, asked: &mut Asked) {
    // every id the tree declares whose name is a keyword
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    let mut host: Vec<(usize, String, String)> = Vec::new();
    walk::each(v, &mut |n, at| {
        let Some(id) = n.get("id").and_then(Value::as_str) else { return };
        if let Some(new) = renamed(id) {
            ids.insert(id.to_string(), new);
            if id.starts_with("$state.") || at.node.contains("/expose/") {
                host.push((at.file, at.holder.clone(), id.to_string()));
            }
        }
    });
    for f in v["files"].as_array_mut().into_iter().flatten() {
        rename(f, &ids);
    }
    let todos = host
        .into_iter()
        .map(|(file, holder, id)| {
            let name = id.rsplit(['/', '.']).next().unwrap_or(&id).to_string();
            (file, holder, format!("`{}` is `{}_` now, as `{}` is a keyword: a host that reads or sets it by its name reads the new one", id, name, name))
        })
        .collect();
    walk::apply(v, todos, asked);
}

fn rename(v: &mut Value, ids: &BTreeMap<String, String>) {
    match v {
        Value::Object(o) => {
            // a name beside the id it is the name of: a declaration's, a named argument's, a field read's
            let id = ["id", "target", "member"].iter().find_map(|k| o.get(*k).and_then(Value::as_str)).map(String::from);
            let of_type = o.get("of").and_then(|x| x["type"]["name"].as_str()).map(String::from);
            for key in ["name", "field"] {
                let Some(name) = o.get(key).and_then(Value::as_str).filter(|n| keyword(n)).map(String::from) else { continue };
                let whose = id.clone().or_else(|| of_type.as_ref().map(|t| format!("{}.{}", t, name)));
                // a local's: no id, and not a field of another's record
                let local = whose.is_none() && key == "name";
                if local || whose.map_or(false, |w| ids.contains_key(&w)) {
                    o.insert(key.into(), Value::String(format!("{}_", name)));
                }
            }
            if let Some(g) = o.get_mut("get").and_then(Value::as_object_mut) {
                if let Some(l) = g.get("local").and_then(Value::as_str).filter(|l| keyword(l)).map(String::from) {
                    g.insert("local".into(), Value::String(format!("{}_", l)));
                }
                // a read of `$state`'s field, by its name
                if let Some(s) = g.get("state").and_then(Value::as_str).filter(|s| ids.contains_key(&format!("$state.{}", s))).map(String::from) {
                    g.insert("state".into(), Value::String(format!("{}_", s)));
                }
            }
            for (k, x) in o.iter_mut() {
                match x {
                    Value::String(s) if k != "name" && k != "field" => {
                        if let Some(new) = ids.get(s.as_str()) {
                            *s = new.clone();
                        }
                    }
                    _ => rename(x, ids),
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| rename(x, ids)),
        _ => {}
    }
}
