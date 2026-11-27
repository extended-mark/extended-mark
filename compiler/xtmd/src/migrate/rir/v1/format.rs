//! The step's format part: rir 1's shape brought to rir 2's. Comments as 0.4.0's parser keeps them
//! (`leading` and `trailing` for `before` and `after`, a declaration's doc comments among them), a front
//! matter's text, a type parameter's bounds as types; and what rir 1 had no place for, which 0.3.2's
//! front end gives beside it (`left_out`, readers/previous.rs), put where 0.4.0's parser would put it.

use serde_json::{json, Map, Value};

/// rir 1's shape brought to rir 2's. A comment whose place this release's parser couldn't give is kept,
/// at the end of its file, with a TODO saying where it was.
pub fn format(v: &mut Value, asked: &mut crate::migrate::rir::Asked) -> Result<(), Vec<String>> {
    for f in v["files"].as_array_mut().into_iter().flatten() {
        let left = f.as_object_mut().and_then(|o| o.remove("left_out")).unwrap_or(Value::Null);
        file(f, &left, asked);
    }
    Ok(())
}

fn file(f: &mut Value, left: &Value, asked: &mut crate::migrate::rir::Asked) {
    let path = f["path"].as_str().unwrap_or("").to_string();
    let now = left.get("now").filter(|n| n.is_object());
    // the front matter: its text, without its `---` lines, and where it stands
    if let Some(span) = f.get("front_matter").cloned().filter(|s| s.is_array()) {
        let text = match now.and_then(|n| n["front_matter"]["text"].as_str()) {
            Some(t) => t.to_string(),
            None => {
                let lines: Vec<&str> = left["front_matter"].as_str().unwrap_or("").lines().collect();
                if lines.len() >= 2 { lines[1..lines.len() - 1].join("\n") } else { String::new() }
            }
        };
        f["front_matter"] = json!({ "text": text, "span": span });
    }
    // comments that sat with no node, at the end of the file
    if let Some(end) = f.as_object_mut().and_then(|o| o.remove("comments")) {
        let lines: Vec<Value> = end.as_array().cloned().unwrap_or_default();
        if !lines.is_empty() {
            push_item(f, json!({ "comment": lines }));
        }
    }
    reshape(f);
    // where each comment goes: where this release's parser puts it, by the spans both give the same text
    let Some(now) = now else {
        // a declaration's doc comment, which rir 1 kept as its doc, stays there
        let mut docs = Vec::new();
        abouts(f, &mut docs);
        let lost: Vec<Value> = left["comments"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["text"].as_str())
            .filter(|t| !t.strip_prefix("//@").and_then(|a| a.split_once(' ')).map_or(false, |(_, rest)| docs.iter().any(|d| !d.is_empty() && rest.trim().ends_with(d.as_str()))))
            .map(|t| Value::String(t.to_string()))
            .collect();
        if !lost.is_empty() {
            push_item(f, json!({ "comment": lost }));
            todo_at_end(f, asked, &path, "this release's parser couldn't read the file, so the comments above are those it couldn't place: put each back where it was");
        }
        return;
    };
    // a surface's version, from its package line, as this release's parser reads it
    if let Some(n) = now.get("version") {
        f["version"] = n.clone();
    }
    let mut index: Vec<(&Value, &Value)> = Vec::new();
    spans(now, &mut index);
    place(f, now, &index);
    // every comment the release before read is still there
    let (want, have) = (count(now), count(f));
    if have < want {
        todo_at_end(f, asked, &path, &format!("{} of its comments couldn't be put back where they were: look for them in the file as it was", want - have));
    }
}

/// Every text of a doc comment below `v`: its `about`, its parameters', its `deprecated`.
fn abouts(v: &Value, out: &mut Vec<String>) {
    fn texts(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => out.push(s.trim().to_string()),
            Value::Object(o) => o.values().for_each(|x| texts(x, out)),
            Value::Array(a) => a.iter().for_each(|x| texts(x, out)),
            _ => {}
        }
    }
    match v {
        Value::Object(o) => {
            if let Some(d) = o.get("doc") {
                texts(d, out);
            }
            o.values().for_each(|x| abouts(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| abouts(x, out)),
        _ => {}
    }
}

/// Every node of rir 2 with a span, by its span.
fn spans<'a>(v: &'a Value, out: &mut Vec<(&'a Value, &'a Value)>) {
    match v {
        Value::Object(o) => {
            if let Some(s) = o.get("span") {
                out.push((s, v));
            }
            o.values().for_each(|x| spans(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| spans(x, out)),
        _ => {}
    }
}

/// What a node is: its keys but its comments' and its span's.
fn kind(v: &Value) -> Vec<&str> {
    v.as_object().map(|o| o.keys().map(String::as_str).filter(|k| !matches!(*k, "comments" | "comment_items" | "part_comments" | "span" | "name_span" | "type")).collect()).unwrap_or_default()
}

const COMMENT_PARTS: &[&str] = &["comments", "comment_items", "part_comments", "package_comments", "head"];
/// What is how a node is written, not what it means, as this release's parser reads it.
const WRITTEN: &[&str] = &["children"];

/// The comments of `now`'s node in the place of `v`'s, and below: in each list, the comment items it
/// holds, among its entries by where each stands.
fn place(v: &mut Value, now: &Value, index: &[(&Value, &Value)]) {
    for k in COMMENT_PARTS.iter().chain(WRITTEN) {
        match now.get(*k) {
            Some(x) => v[*k] = x.clone(),
            None => {
                v.as_object_mut().map(|o| o.remove(*k));
            }
        }
    }
    // a widget's Markdown written empty, `markdown { }`, rather than left out
    if now.get("markdown").and_then(Value::as_array).map_or(false, Vec::is_empty) && v.get("markdown").is_none() && v.get("view").is_some() {
        v["markdown"] = json!([]);
    }
    // a paragraph's text as this release reads its lines, where it is the same text
    if let (Some(mine), Some(theirs)) = (v.pointer("/markdown/markdown").and_then(Value::as_str), now.pointer("/markdown/markdown").and_then(Value::as_str)) {
        if mine.trim_end() == theirs.trim_end() && mine != theirs {
            v["markdown"]["markdown"] = Value::String(theirs.to_string());
        }
    }
    let Some(o) = v.as_object_mut() else { return };
    for (k, x) in o.iter_mut() {
        if COMMENT_PARTS.contains(&k.as_str()) {
            continue;
        }
        if let (Value::Array(items), Some(Value::Array(theirs))) = (&mut *x, now.get(k)) {
            let comments: Vec<Value> = theirs.iter().filter(|e| e.get("comment").is_some()).cloned().collect();
            items.retain(|e| e.get("comment").is_none());
            for c in comments {
                let at = c["span"][0].as_u64().unwrap_or(u64::MAX);
                let pos = items.iter().position(|e| e["span"][0].as_u64().map_or(false, |s| s > at)).unwrap_or(items.len());
                items.insert(pos, c);
            }
        }
        visit(x, index);
    }
}

/// Every node below `v` given the comments of the node of `now` that stands where it does.
fn visit(v: &mut Value, index: &[(&Value, &Value)]) {
    match v {
        Value::Object(_) => {
            let mine = v.get("span").cloned();
            let found = mine.as_ref().and_then(|s| {
                let same: Vec<&&Value> = index.iter().filter(|(t, _)| *t == s).map(|(_, n)| n).collect();
                let k = kind(v);
                same.iter().find(|n| kind(n) == k).or(if same.len() == 1 { same.first() } else { None }).map(|n| (**n).clone())
            });
            // a paragraph this release ends a line later (its line's end, before a blank line): the
            // node of the same kind that starts where it does
            let found = found.or_else(|| {
                let at = mine.as_ref()?.get(0)?;
                let k = kind(v);
                index.iter().find(|(t, n)| t.get(0) == Some(at) && kind(n) == k && n.pointer("/markdown/markdown").map_or(false, Value::is_string)).map(|(_, n)| (*n).clone())
            });
            match found {
                Some(n) => place(v, &n, index),
                None => {
                    if let Value::Object(o) = v {
                        for (k, x) in o.iter_mut() {
                            if !COMMENT_PARTS.contains(&k.as_str()) {
                                visit(x, index);
                            }
                        }
                    }
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| visit(x, index)),
        _ => {}
    }
}

/// How many comments a resolved IR holds, doc comments' lines apart.
fn count(v: &Value) -> usize {
    match v {
        Value::Object(o) => o
            .iter()
            .map(|(k, x)| match k.as_str() {
                "leading" | "free" | "comment" => x.as_array().map_or(0, Vec::len),
                "trailing" => 1,
                "block_start" | "block_end" => x.as_array().map_or(0, Vec::len),
                "doc" => 0,
                _ => count(x),
            })
            .sum(),
        Value::Array(a) => a.iter().map(count).sum(),
        _ => 0,
    }
}

/// A TODO at the end of a file, for what belongs to no node.
fn todo_at_end(f: &mut Value, asked: &mut crate::migrate::rir::Asked, path: &str, what: &str) {
    push_item(f, json!({ "comment": [format!("// TODO(xtmd migrate): {}", what)] }));
    asked.todos.push(format!("{}: {}", path, what));
}

/// A run of comments on lines of their own, among a body's comment items.
fn push_item(owner: &mut Value, item: Value) {
    if let Some(o) = owner.as_object_mut() {
        o.entry("comment_items").or_insert_with(|| Value::Array(Vec::new())).as_array_mut().unwrap().push(item);
    }
}

/// Every node below `v` in rir 2's shape.
fn reshape(v: &mut Value) {
    match v {
        Value::Object(o) => {
            // a declaration's or a fn's doc comments are among its comments (a node's, never the
            // comments' own `doc`)
            if let Some(doc) = o.contains_key("span").then(|| o.remove("doc")).flatten() {
                if doc.is_object() && !doc.as_object().unwrap().is_empty() {
                    comments_of(o).insert("doc".into(), doc);
                }
            }
            if let Some(c) = o.get_mut("comments").and_then(Value::as_object_mut) {
                let before = c.remove("before");
                let after = c.remove("after");
                if let Some(b) = before.filter(|b| b.as_array().map_or(false, |a| !a.is_empty())) {
                    c.insert("leading".into(), b);
                }
                if let Some(a) = after.and_then(|a| a.as_array().cloned()).filter(|a| !a.is_empty()) {
                    let words: Vec<&str> = a.iter().filter_map(Value::as_str).collect();
                    c.insert("trailing".into(), Value::String(words.join(" ")));
                }
            }
            if o.get("comments").and_then(Value::as_object).map_or(false, Map::is_empty) {
                o.remove("comments");
            }
            // a fn says its name, as its id ends: `doc/w.add` is `add`
            if o.contains_key("params") && o.contains_key("body") && !o.contains_key("name") {
                if let Some(name) = o.get("id").and_then(Value::as_str).map(|id| id.rsplit('/').next().unwrap_or(id).rsplit('.').next().unwrap_or(id).to_string()) {
                    o.insert("name".into(), Value::String(name));
                }
            }
            // a slot filled by the call's block, rather than by name, where this release's parser can't
            // say (place() takes what it says): rir 1 names only those filled by name
            if o.contains_key("slot") && (o.contains_key("view") || o.contains_key("prose")) && !o.contains_key("children") && !o.contains_key("name_span") {
                o.insert("children".into(), Value::Bool(true));
            }
            // a type parameter's bounds: types, not names
            for key in ["bounds"] {
                if let Some(a) = o.get_mut(key).and_then(Value::as_array_mut) {
                    for x in a.iter_mut() {
                        if let Some(name) = x.as_str() {
                            *x = json!({ "name": name });
                        }
                    }
                }
            }
            for (k, x) in o.iter_mut() {
                if k != "comments" {
                    reshape(x);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(reshape),
        _ => {}
    }
}

fn comments_of(o: &mut Map<String, Value>) -> &mut Map<String, Value> {
    if !o.get("comments").map_or(false, Value::is_object) {
        o.insert("comments".into(), Value::Object(Map::new()));
    }
    o.get_mut("comments").unwrap().as_object_mut().unwrap()
}
