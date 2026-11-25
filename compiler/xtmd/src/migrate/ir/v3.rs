//! IR 3 (xtmd 0.3) to IR 4: text is calls. A rich_text's segments become calls of core's sentence
//! widgets, `core/md` values the calls of their blocks, a prop's binding the prop itself, and the app is
//! the host. What read a rich_text's parts, or Markdown as the page ran, can't be written in IR 4.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// core's records and enums of text, gone in IR 4.
const GONE: [&str; 7] = ["core/md", "core/list_item", "core/rich_text", "core/segment", "core/marks", "core/block", "core/part"];
/// What core wrote with them.
const GONE_WITH: [&str; 2] = ["core/block_view", "core/blocks"];

/// core's Markdown blocks: the piece of Markdown each is, in IR 4.
const PIECES: [&str; 10] = ["paragraph", "heading", "bullets", "item", "quote", "code", "math", "table", "footnote", "divider"];

/// core's sentence widgets: their inputs, and whether they hold a sentence.
const SENTENCE: [(&str, &[(&str, &str)], bool); 10] = [
    ("words", &[("value", "core/text")], false),
    ("bold", &[], true),
    ("italic", &[], true),
    ("strike", &[], true),
    ("link", &[("to", "core/text")], true),
    ("code_span", &[("src", "core/text")], false),
    ("inline_math", &[("tex", "core/latex")], false),
    ("inline_image", &[("src", "core/text"), ("alt", "core/text")], false),
    ("note_mark", &[("mark", "core/text")], false),
    ("line_break", &[], false),
];

const DRAW: &str = "core.draw";

pub fn step(v: &mut Value) -> Result<(), Vec<String>> {
    let m = v["ir"].as_object_mut().ok_or_else(|| vec!["it has no `ir`".to_string()])?;
    let bindings: HashMap<String, Value> = match m.remove("bindings") {
        Some(Value::Array(b)) => b.into_iter().filter_map(|b| Some((b["id"].as_str()?.to_string(), b["value"].clone()))).collect(),
        _ => HashMap::new(),
    };
    let mut t = Text { bindings, next: 0, used: BTreeSet::new() };
    for part in ["declarations", "templates", "handlers"] {
        if let Some(Value::Array(items)) = m.get_mut(part) {
            // a declaration of what is gone stays as it is until it is dropped
            items.iter_mut().filter(|d| !gone(&name_of(d))).for_each(|d| t.value(d));
        }
    }
    drop_unused(m);
    let wrong = left_over(m);
    if !wrong.is_empty() {
        return Err(wrong);
    }
    if let Some(Value::Array(d)) = m.get_mut("declarations") {
        for w in d.iter_mut().filter_map(|d| d.pointer_mut("/declare/extern/widget").and_then(Value::as_object_mut)) {
            let piece = w["name"].as_str().and_then(|n| n.strip_prefix("core/")).filter(|n| PIECES.contains(n)).map(str::to_string);
            if let Some(p) = piece {
                w.remove("markdown");
                w.insert("piece".into(), p.into());
            }
        }
        for (name, inputs, body) in SENTENCE.iter().filter(|(n, ..)| t.used.contains(*n)) {
            let inputs: Vec<Value> = inputs.iter().map(|(n, ty)| json!({ "name": n, "type": { "name": ty } })).collect();
            let slots = if *body { json!([{ "name": "body", "type": { "name": "core/rich_text" } }]) } else { json!([]) };
            d.push(json!({ "declare": { "extern": { "widget": { "name": format!("core/{}", name), "piece": if *name == "words" { "text" } else { name }, "inputs": inputs, "slots": slots, "events": [], "supplier": "core" } } } }));
        }
    }
    fill_slots(m);
    for part in ["declarations", "templates", "handlers"] {
        if let Some(x) = m.get_mut(part) {
            tidy(x);
        }
    }
    let mut taken = BTreeSet::new();
    for part in ["declarations", "templates", "handlers"] {
        values_taken(&m[part], &mut taken);
    }
    if let Some(n) = m.get_mut("needs").and_then(Value::as_object_mut) {
        let mut host: BTreeSet<String> = strings(n.remove("app"));
        host.extend(taken);
        n.insert("host".into(), json!(host));
        let mut core = strings(n.remove("core"));
        core.extend(t.used.iter().map(|u| format!("core/{}", u)));
        n.insert("core".into(), json!(core));
    }
    Ok(())
}

fn strings(v: Option<Value>) -> BTreeSet<String> {
    match v {
        Some(Value::Array(xs)) => xs.into_iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => BTreeSet::new(),
    }
}

/// A declaration's, a template's or a handler's name.
fn name_of(d: &Value) -> String {
    let mut v = d.get("declare").unwrap_or(d);
    for _ in 0..3 {
        if let Some(n) = v.get("name").or_else(|| v.get("id")).and_then(Value::as_str) {
            return n.to_string();
        }
        match v.as_object().filter(|o| o.len() == 1).and_then(|o| o.values().next()) {
            Some(x) => v = x,
            None => break,
        }
    }
    String::new()
}

fn gone(name: &str) -> bool {
    GONE.contains(&name) || GONE_WITH.contains(&name) || GONE.iter().any(|g| name.strip_prefix(g).map_or(false, |r| r.starts_with('.')))
}

fn one<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.as_object().filter(|o| o.len() == 1).and_then(|o| o.get(key))
}

// ---- text

struct Text {
    bindings: HashMap<String, Value>,
    next: usize,
    /// The sentence widgets called.
    used: BTreeSet<&'static str>,
}

impl Text {
    fn site(&mut self) -> String {
        self.next += 1;
        format!("u{}", self.next)
    }

    fn call(&mut self, name: &'static str, props: Value) -> Value {
        if SENTENCE.iter().any(|(n, ..)| *n == name) {
            self.used.insert(name);
        }
        json!({ "extern": { "name": format!("core/{}", name), "site": self.site(), "props": props } })
    }

    fn holding(&mut self, name: &'static str, props: Value, slot: &str, body: Vec<Value>) -> Value {
        let mut c = self.call(name, props);
        c["extern"]["slots"] = json!({ slot: body });
        c
    }

    /// Everything in a value, what it is in IR 4.
    fn value(&mut self, v: &mut Value) {
        if let Some(id) = one(v, "bind").and_then(Value::as_str) {
            if let Some(x) = self.bindings.get(id).cloned() {
                *v = x;
                return self.value(v);
            }
        }
        match v {
            Value::Array(xs) => xs.iter_mut().for_each(|x| self.value(x)),
            Value::Object(o) => o.values_mut().for_each(|x| self.value(x)),
            _ => return,
        }
        if let Some(Value::Array(segments)) = one(v, "rich").and_then(|r| r.get("segments")).cloned().as_ref() {
            let nodes: Vec<Value> = segments.iter().map(|s| self.segment(&s["segment"])).collect();
            *v = json!({ "rich": { "nodes": nodes } });
        } else if let Some(r) = one(v, "record").cloned() {
            match r["type"].as_str() {
                Some("core/rich_text") => {
                    if let Some(nodes) = self.sentence(&r) {
                        *v = json!({ "rich": { "nodes": nodes } });
                    }
                }
                Some("core/md") => {
                    if let Some(nodes) = one(&r["fields"]["blocks"], "list").and_then(Value::as_array).cloned().and_then(|b| self.blocks(&b)) {
                        *v = json!({ "rich": { "blocks": true, "nodes": nodes } });
                    }
                }
                _ => {}
            }
        } else if let Some(c) = one(v, "call").filter(|c| c["name"] == "core/rich_text.plain").cloned() {
            let shown = json!({ "show": { "value": c["args"][0] } });
            let words = self.call("words", json!({ "value": shown }));
            *v = json!({ "rich": { "nodes": [words] } });
        } else if let Some(u) = one(v, "use").filter(|u| u["name"] == "core/blocks").cloned() {
            *v = json!({ "place": { "site": u["site"], "view": u["args"]["value"] } });
        } else if let Some(x) = one(v, "view").filter(|x| x["widget"] == "core/blocks").map(|x| x["args"]["value"].clone()) {
            *v = x;
        }
        // a `core/md` is a rich_text
        if v.get("name").map_or(false, |n| n == "core/md") {
            v["name"] = "core/rich_text".into();
        }
    }

    fn segment(&mut self, s: &Value) -> Value {
        let value = || s["value"].clone();
        let node = match s["part"].as_str().unwrap_or("") {
            "CODE" => self.call("code_span", json!({ "src": value() })),
            "MATH" => self.call("inline_math", json!({ "tex": value() })),
            "IMAGE" => self.call("inline_image", json!({ "src": s["src"], "alt": s["alt"] })),
            "NOTE" => self.call("note_mark", json!({ "mark": value() })),
            "BREAK" => self.call("line_break", json!({})),
            "VIEW" => json!({ "place": { "site": self.site(), "view": value(), "sentence": true } }),
            _ => self.call("words", json!({ "value": value() })),
        };
        let m = &s["marks"];
        let on = |k: &str| m[k] == true;
        self.marked(node, on("bold"), on("italic"), on("strike"), m.get("link").cloned())
    }

    fn marked(&mut self, mut node: Value, bold: bool, italic: bool, strike: bool, link: Option<Value>) -> Value {
        if italic {
            node = self.holding("italic", json!({}), "body", vec![node]);
        }
        if bold {
            node = self.holding("bold", json!({}), "body", vec![node]);
        }
        if strike {
            node = self.holding("strike", json!({}), "body", vec![node]);
        }
        match link {
            Some(to) => self.holding("link", json!({ "to": to }), "body", vec![node]),
            None => node,
        }
    }

    /// The calls of a `core/rich_text` record written out whole.
    fn sentence(&mut self, r: &Value) -> Option<Vec<Value>> {
        let segments = one(&r["fields"]["segments"], "list")?.as_array()?.clone();
        segments
            .iter()
            .map(|s| {
                let s = one(s, "record").filter(|s| s["type"] == "core/segment")?;
                let part = one(&s["fields"]["part"], "case").filter(|p| p["enum"] == "core/part")?;
                let x = |k: &str| part["values"].get(k).cloned();
                let node = match part["name"].as_str()? {
                    "TEXT" => self.call("words", json!({ "value": x("value")? })),
                    "CODE" => self.call("code_span", json!({ "src": x("value")? })),
                    "MATH" => self.call("inline_math", json!({ "tex": x("tex")? })),
                    "IMAGE" => self.call("inline_image", json!({ "src": x("src")?, "alt": x("alt")? })),
                    "NOTE" => self.call("note_mark", json!({ "mark": x("mark")? })),
                    "BREAK" => self.call("line_break", json!({})),
                    "VIEW" => json!({ "place": { "site": self.site(), "view": x("child")?, "sentence": true } }),
                    _ => return None,
                };
                let Some(marks) = s["fields"].get("marks") else { return Some(node) };
                let m = &one(marks, "record")?["fields"];
                let on = |k: &str| match m.get(k) {
                    None => Some(false),
                    Some(b) => one(b, "bool")?.as_bool(),
                };
                let link = match m.get("link") {
                    None => None,
                    Some(l) if one(l, "none").is_some() => None,
                    Some(l) => Some(one(l, "some").cloned().unwrap_or_else(|| l.clone())),
                };
                Some(self.marked(node, on("bold")?, on("italic")?, on("strike")?, link))
            })
            .collect()
    }

    /// The calls of a `core/md`'s blocks, its sentences already calls.
    fn blocks(&mut self, blocks: &[Value]) -> Option<Vec<Value>> {
        blocks
            .iter()
            .map(|b| {
                let b = one(b, "case").filter(|b| b["enum"] == "core/block")?;
                let x = |k: &str| b["values"].get(k).cloned();
                let body = |t: &mut Text, v: Option<Value>| one(&v?, "list")?.as_array().cloned().and_then(|b| t.blocks(&b));
                Some(match b["name"].as_str()? {
                    "PARAGRAPH" => self.call("paragraph", json!({ "value": x("value")? })),
                    "HEADING" => self.call("heading", json!({ "level": x("level")?, "value": x("value")? })),
                    "CODE" => self.call("code", json!({ "src": x("src")?, "lang": x("lang")? })),
                    "MATH" => self.call("math", json!({ "tex": x("tex")? })),
                    "TABLE" => self.call("table", json!({ "head": x("head")?, "rows": x("rows")?, "align": x("align")? })),
                    "DIVIDER" => self.call("divider", json!({})),
                    "QUOTE" => {
                        let held = body(self, x("body"))?;
                        self.holding("quote", json!({}), "body", held)
                    }
                    "FOOTNOTE" => {
                        let held = body(self, x("body"))?;
                        self.holding("footnote", json!({ "mark": x("mark")? }), "body", held)
                    }
                    "BULLETS" => {
                        let items = one(&x("items")?, "list")?.as_array()?.clone();
                        let items: Option<Vec<Value>> = items
                            .iter()
                            .map(|i| {
                                let i = &one(i, "record")?["fields"];
                                let held = body(self, i.get("body").cloned())?;
                                let props = i.get("done").map_or(json!({}), |d| json!({ "done": d }));
                                Some(self.holding("item", props, "body", held))
                            })
                            .collect();
                        self.holding("bullets", json!({ "ordered": x("ordered")?, "start": x("start")? }), "items", items?)
                    }
                    _ => return None,
                })
            })
            .collect()
    }
}

// ---- what is left out

/// Every string in a value, a member's owner and a read's name among them.
fn names(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::String(s) => {
            out.insert(s.clone());
            if let Some((o, _)) = s.rsplit_once('.') {
                out.insert(o.to_string());
            }
        }
        Value::Array(xs) => xs.iter().for_each(|x| names(x, out)),
        Value::Object(o) => o.values().for_each(|x| names(x, out)),
        _ => {}
    }
}

/// Drops core's declarations of what is gone, and those only they used.
fn drop_unused(m: &mut Map<String, Value>) {
    let mut parts: Vec<(String, BTreeSet<String>, bool)> = Vec::new();
    for part in ["declarations", "templates", "handlers"] {
        for d in m[part].as_array().into_iter().flatten() {
            let name = name_of(d);
            let mut used = BTreeSet::new();
            names(d, &mut used);
            let token = d["declare"].get("token").is_some();
            parts.push((name.clone(), used, token || !name.starts_with("core/")));
        }
    }
    let mut kept: BTreeSet<String> = BTreeSet::new();
    let mut by_type: BTreeSet<String> = ["show", "same", "before"].iter().map(|s| s.to_string()).collect();
    for d in m["declarations"].as_array().into_iter().flatten() {
        by_type.extend(d["declare"]["interface"]["fns"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(str::to_string)));
    }
    let mut todo: Vec<usize> = (0..parts.len()).filter(|&k| parts[k].2 && !gone(&parts[k].0)).collect();
    let mut seen: BTreeSet<usize> = todo.iter().copied().collect();
    while let Some(k) = todo.pop() {
        kept.insert(parts[k].0.clone());
        for (j, (n, ..)) in parts.iter().enumerate() {
            let method = n.rsplit_once('.').map_or(false, |(t, f)| t == parts[k].0 && by_type.contains(f));
            if !seen.contains(&j) && !gone(n) && (parts[k].1.contains(n) || method) {
                seen.insert(j);
                todo.push(j);
            }
        }
    }
    for part in ["declarations", "templates", "handlers"] {
        if let Some(Value::Array(ds)) = m.get_mut(part) {
            ds.retain(|d| {
                let n = name_of(d);
                n.is_empty() || kept.contains(&n)
            });
        }
    }
    if let Some(n) = m.get_mut("needs") {
        for list in ["core"] {
            if let Some(Value::Array(xs)) = n.get_mut(list) {
                xs.retain(|x| x.as_str().map_or(false, |x| kept.contains(x)));
            }
        }
    }
}

/// What can't be written in IR 4, by the part of the page it is in.
fn left_over(m: &Map<String, Value>) -> Vec<String> {
    // the page's own fields of these names are its own
    let mut own = BTreeSet::new();
    for d in m["declarations"].as_array().into_iter().flatten() {
        for f in d["declare"]["record"]["fields"].as_array().into_iter().flatten() {
            own.extend(f["name"].as_str().map(str::to_string));
        }
    }
    let mut out = Vec::new();
    for part in ["declarations", "templates", "handlers"] {
        for d in m[part].as_array().into_iter().flatten() {
            let mut whats = BTreeSet::new();
            look(d, &own, &mut whats);
            let name = name_of(d);
            let name = if name.is_empty() { "`$state`, or what the app may call,".to_string() } else { format!("`{}`", name) };
            out.extend(whats.into_iter().map(|w| format!("{} {}: IR 4 has nothing for it, so build the page again from its source, moved by `xtmd migrate`", name, w)));
        }
    }
    out
}

fn look(v: &Value, own: &BTreeSet<String>, out: &mut BTreeSet<&'static str>) {
    const PARTS: &str = "reads or makes the parts of a rich_text";
    const READS: &str = "reads Markdown while the page runs";
    const SHOWS: &str = "shows a rich_text as text";
    match v {
        Value::Array(xs) => xs.iter().for_each(|x| look(x, own, out)),
        Value::Object(o) => {
            if let Some(c) = one(v, "call").and_then(|c| c["name"].as_str()) {
                if c == "core/md.from" || c == "core/rich_text.from" {
                    out.insert(READS);
                } else if c == "core/rich_text.show" {
                    out.insert(SHOWS);
                } else if gone(c) {
                    out.insert(PARTS);
                }
            }
            if let Some(f) = one(v, "field").and_then(|f| f["name"].as_str()) {
                if (f == "segments" || f == "blocks") && !own.contains(f) {
                    out.insert(PARTS);
                }
            }
            let named = |x: Option<&Value>| x.and_then(Value::as_str).map_or(false, |n| GONE.contains(&n) && n != "core/rich_text");
            if named(o.get("name")) || named(o.get("enum")) || o.get("type").and_then(Value::as_str).map_or(false, |n| GONE.contains(&n)) || o.get("widget").and_then(Value::as_str).map_or(false, gone) {
                out.insert(PARTS);
            }
            o.values().for_each(|x| look(x, own, out));
        }
        _ => {}
    }
}

// ---- the rest

/// A slot a call left out was empty: in IR 4 a call fills every slot that isn't `opt`.
fn fill_slots(m: &mut Map<String, Value>) {
    let mut slots: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut note = |w: &Value| {
        let must: Vec<String> = w["slots"].as_array().into_iter().flatten().filter(|s| s["type"]["name"] != "core/opt").filter_map(|s| s["name"].as_str().map(str::to_string)).collect();
        if let (Some(n), false) = (w["name"].as_str(), must.is_empty()) {
            slots.insert(n.to_string(), must);
        }
    };
    m["templates"].as_array().into_iter().flatten().for_each(&mut note);
    m["declarations"].as_array().into_iter().flatten().for_each(|d| note(&d["declare"]["extern"]["widget"]));
    fn walk(v: &mut Value, slots: &BTreeMap<String, Vec<String>>) {
        match v {
            Value::Array(xs) => xs.iter_mut().for_each(|x| walk(x, slots)),
            Value::Object(o) => {
                for (k, x) in o.iter_mut() {
                    let called = match k.as_str() {
                        "use" | "extern" if x.get("site").is_some() => x["name"].as_str(),
                        "view" => x["widget"].as_str(),
                        _ => None,
                    };
                    if let Some(must) = called.and_then(|n| slots.get(n)).cloned() {
                        for s in must {
                            if x["slots"].get(&s).is_none() {
                                x["slots"][s.as_str()] = json!([]);
                            }
                        }
                    }
                    walk(x, slots);
                }
            }
            _ => {}
        }
    }
    for part in ["declarations", "templates", "handlers"] {
        if let Some(x) = m.get_mut(part) {
            walk(x, &slots);
        }
    }
}

/// The app is the host, every call needs core.draw, and nothing lists what it reads.
fn tidy(v: &mut Value) {
    match v {
        Value::Array(xs) => xs.iter_mut().for_each(tidy),
        Value::Object(o) => {
            if o.get("supplier").map_or(false, |s| s == "app") {
                o.insert("supplier".into(), "host".into());
            }
            if let Some(Value::Array(l)) = o.get_mut("levels") {
                l.retain(|x| x != DRAW);
                if l.is_empty() {
                    o.remove("levels");
                }
            }
            if o.get("reads").map_or(false, Value::is_array) && (o.contains_key("message") || o.contains_key("value")) {
                o.remove("reads");
            }
            o.values_mut().for_each(tidy);
        }
        _ => {}
    }
}

/// core's page's values, which the host gives.
fn values_taken(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Array(xs) => xs.iter().for_each(|x| values_taken(x, out)),
        Value::Object(o) => {
            if let Some(n) = one(v, "get").and_then(|g| g["extern"].as_str()) {
                out.insert(n.to_string());
            }
            o.values().for_each(|x| values_taken(x, out));
        }
        _ => {}
    }
}
