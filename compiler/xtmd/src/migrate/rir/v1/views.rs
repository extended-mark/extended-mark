//! B5 and B6: what a call's block gave that 0.3.2 drew otherwise than it reads.
//!
//! B6: a block given to a view held in a value, `@v { … }`, was dropped by 0.3.2 without a word; 0.4.0
//! refuses it. The block is taken off and kept as a comment above the call, with a TODO.
//!
//! B5: a slot that holds one widget, given several. A question each, by the slot's id and the count of
//! such calls before it: keep the first and the rest as a comment, or, when the tree declares the
//! widget, make the slot a list.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::walk;
use super::Resolved;
use crate::migrate::rir::Asked;

const KEEP_FIRST: &str = "first";
const LIST: &str = "list";

pub fn blocks(v: &mut Value, known: &Resolved, asked: &mut Asked) {
    let own: Vec<String> = v["files"].as_array().into_iter().flatten().flat_map(|f| f["declarations"].as_array().into_iter().flatten()).filter_map(|d| d.pointer("/widget/id").and_then(Value::as_str).map(String::from)).collect();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut lists: Vec<String> = Vec::new();
    let n = v["files"].as_array().map_or(0, Vec::len);
    for i in 0..n {
        let path = walk::path(v, i);
        let mut cx = Cx { known, own: &own, seen: &mut seen, lists: &mut lists, asked, path };
        uses(&mut v["files"][i], &mut cx);
    }
    // a slot made a list, in the widget that declares it
    for f in v["files"].as_array_mut().into_iter().flatten() {
        let path = f["path"].as_str().unwrap_or("").to_string();
        for d in f.get_mut("declarations").and_then(Value::as_array_mut).into_iter().flatten() {
            let Some(w) = d.get_mut("widget") else { continue };
            let id = w["id"].as_str().unwrap_or("").to_string();
            let mut made = Vec::new();
            for s in w.get_mut("slots").and_then(Value::as_array_mut).into_iter().flatten() {
                if s["name"].as_str().map_or(false, |n| lists.contains(&format!("{}.{}", id, n))) {
                    let one = s["type"].clone();
                    s["type"] = json!({ "name": "core/list", "args": [one] });
                    made.push(s["name"].as_str().unwrap_or("").to_string());
                }
            }
            for name in made {
                let what = format!("the slot `{}` is a list now, as answered: where this widget reads it, it reads each of them (`for (x in {}) {{ … }}`)", name, name);
                asked.todo(d, &path, &what);
            }
        }
    }
}

struct Cx<'a> {
    known: &'a Resolved,
    own: &'a [String],
    seen: &'a mut BTreeMap<String, usize>,
    lists: &'a mut Vec<String>,
    asked: &'a mut Asked,
    path: String,
}

fn uses(v: &mut Value, cx: &mut Cx) {
    match v {
        Value::Object(o) => {
            let mut kept: Vec<String> = Vec::new();
            let mut todo: Option<String> = None;
            for key in ["use", "call"] {
                let Some(c) = o.get_mut(key).filter(|c| c.get("widget").is_some()) else { continue };
                let widget = c["widget"].as_str().unwrap_or("").to_string();
                // B6: a view held in a value is a local's name, not a declaration's id
                if !widget.contains('/') {
                    if let Some(slots) = c.as_object_mut().and_then(|c| c.remove("slots")).and_then(|s| s.as_array().cloned()).filter(|s| !s.is_empty()) {
                        let inner: Vec<Value> = slots.iter().flat_map(|s| s["view"].as_array().cloned().unwrap_or_default()).collect();
                        kept.push(format!("// a block given to `{}`, which 0.3.2 never drew:", widget));
                        kept.extend(walk::printed(&inner).into_iter().map(|l| format!("//     {}", l)));
                        todo = Some(format!("`{}` is a view held in a value, which takes no block: the block 0.3.2 dropped is kept above; place what it should draw elsewhere, or delete it", widget));
                    }
                    continue;
                }
                // B5: a slot that holds one widget, given several
                let Some(decl) = cx.known.slots.get(&widget).and_then(Value::as_array).cloned() else { continue };
                for e in c.get_mut("slots").and_then(Value::as_array_mut).into_iter().flatten() {
                    let id = e["slot"].as_str().unwrap_or("").to_string();
                    let name = id.rsplit('.').next().unwrap_or("");
                    let Some(ty) = decl.iter().find(|s| s["name"] == name).map(|s| s["type"].clone()) else { continue };
                    if matches!(ty["name"].as_str(), Some("core/view" | "core/list" | "core/opt") | None) {
                        continue;
                    }
                    let Some(view) = e.get_mut("view").and_then(Value::as_array_mut) else { continue };
                    let placed: Vec<usize> = view.iter().enumerate().filter(|(_, s)| s.get("use").is_some()).map(|(k, _)| k).collect();
                    if placed.len() < 2 {
                        continue;
                    }
                    let n = cx.seen.entry(id.clone()).or_default();
                    *n += 1;
                    let short = widget.rsplit('/').next().unwrap_or(&widget);
                    let of = ty["name"].as_str().unwrap_or("").rsplit('/').next().unwrap_or("");
                    let own = cx.own.contains(&widget);
                    let mut options = vec![KEEP_FIRST];
                    if own {
                        options.push(LIST);
                    }
                    let question = format!(
                        "{}: `{}`'s slot `{}` holds one {}, and {} are given to it (call {} of `{}` that does so). Answer `{}` to keep the first and the rest as a comment{}",
                        cx.path, short, name, of, placed.len(), n, short, KEEP_FIRST,
                        if own { format!(", or `{}` to make the slot a list of them, in `{}` itself", LIST, short) } else { String::new() }
                    );
                    match cx.asked.ask(&format!("slot-holds-one:{}:{}", id, n), &question, &options).as_deref() {
                        Some(LIST) => {
                            if !cx.lists.contains(&id) {
                                cx.lists.push(id.clone());
                            }
                        }
                        Some(_) => {
                            let rest: Vec<Value> = placed[1..].iter().rev().map(|k| view.remove(*k)).rev().collect();
                            let mut lines = vec![format!("// `{}`'s slot `{}` holds one {}; these were given to it as well:", short, name, of)];
                            lines.extend(walk::printed(&rest).into_iter().map(|l| format!("//     {}", l)));
                            let span = view[placed[0]]["span"].clone();
                            view.insert(placed[0] + 1, json!({ "comment": lines, "span": span }));
                        }
                        None => {}
                    }
                }
            }
            if !kept.is_empty() {
                let c = o.entry("comments").or_insert_with(|| json!({}));
                let leading = c.as_object_mut().unwrap().entry("leading").or_insert_with(|| json!([]));
                if let Some(l) = leading.as_array_mut() {
                    l.extend(kept.into_iter().map(Value::String));
                }
            }
            if let Some(t) = todo {
                let mut node = Value::Object(std::mem::take(o));
                cx.asked.todo(&mut node, &cx.path, &t);
                *o = node.as_object().cloned().unwrap_or_default();
            }
            for (k, x) in o.iter_mut() {
                if k != "comments" {
                    uses(x, cx);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| uses(x, cx)),
        _ => {}
    }
}
