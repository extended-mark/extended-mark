//! 0.3's `rich_text` to 0.4's. In 0.3.2 a `rich_text` was one sentence, with parts code could read; in
//! 0.4.0 it is Markdown, blocks too, drawn where it is placed, and nothing reads its parts.
//!
//! B7: a `rich_text` in `$state` keeps its text: the field is a `text`, and each read makes the
//! `rich_text` from it, as Markdown where it reads as such (a TODO tells the host the type changed).
//! B4: `.show()` and `rich_text.read(…)` are gone: where the value was made from a text, that text is
//! used, and anything else is left for a person. B8: code that reads a `rich_text`'s parts, or compares
//! two, is left for a person. B9 and C1: what 0.3.2 read as one sentence, 0.4.0 may read as blocks: a
//! literal's block markers are escaped, and a text read while the page runs is left for a person.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::walk;
use crate::migrate::rir::Asked;

const RICH: &str = "core/rich_text";
/// What 0.3.2's `rich_text` was made of, which 0.4.0 doesn't have.
const PARTS: &[&str] = &["core/segment", "core/part", "core/marks", "core/block", "core/list_item"];

const HELD: &str = "a rich_text is Markdown drawn where it is placed in 0.4.0, and nothing reads its parts: keep the text it is made from, and read that";

fn is(ty: &Value, name: &str) -> bool {
    ty["name"] == name
}

pub fn rich(v: &mut Value, asked: &mut Asked) {
    // B7: the fields of `$state` that held a rich_text
    let mut held: BTreeSet<String> = BTreeSet::new();
    let mut host: Vec<(usize, String)> = Vec::new();
    for (i, f) in v["files"].as_array_mut().into_iter().flatten().enumerate() {
        for (k, field) in f.pointer_mut("/state/fields").and_then(Value::as_array_mut).into_iter().flatten().enumerate() {
            if !is(&field["type"], RICH) {
                continue;
            }
            let name = field["name"].as_str().unwrap_or("").to_string();
            field["type"] = json!({ "name": "core/text" });
            if let Some(d) = field.get_mut("default") {
                d["type"] = json!({ "name": "core/text" });
            }
            held.insert(name);
            host.push((i, format!("/state/fields/{}", k)));
        }
    }
    for (file, at) in host {
        let name = v["files"][file].pointer(&at).and_then(|f| f["name"].as_str()).unwrap_or("").to_string();
        let what = format!("`$state.{}` holds a text now, and each read makes the rich_text from it: a host that saves it or sets it gives the text", name);
        walk::todo(v, file, &at, &what, asked);
    }
    walk::each_mut(v, asked, &mut |n| {
        // B4: what a rich_text was made from, where show() read it
        if n.pointer("/call/fn").map_or(false, |f| f == "core/rich_text.show") {
            let from = n["call"]["self"].clone();
            let text = if from.pointer("/call/fn").map_or(false, |f| f == "core/rich_text.plain") {
                from["call"]["args"][0].clone()
            } else if from.pointer("/get/state").and_then(Value::as_str).map_or(false, |s| held.contains(s)) {
                let mut x = from.clone();
                x["type"] = json!({ "name": "core/text" });
                x
            } else {
                return Some("`.show()` is gone in 0.4.0: a rich_text is drawn, not read as a text; use the text it is made from".into());
            };
            let span = n["span"].clone();
            *n = text;
            if !span.is_null() {
                n["span"] = span;
            }
            return None;
        }
        if n.pointer("/call/fn").map_or(false, |f| f == "core/rich_text.read") {
            return Some("`rich_text.read(…)` is gone in 0.4.0: `rich_text.from(t)` reads a template's Markdown, its holes filled where it is written".into());
        }
        // B7: a read of a field that holds the text now
        if n.pointer("/get/state").and_then(Value::as_str).map_or(false, |s| held.contains(s)) && is(&n["type"], RICH) {
            let span = n["span"].clone();
            let mut read = n.clone();
            read["type"] = json!({ "name": "core/text" });
            *n = json!({
                "call": {
                    "fn": "core/opt.or",
                    "self": { "call": { "fn": "core/rich_text.from", "args": [{ "call": { "fn": "core/template.of", "args": [read.clone()] }, "type": { "name": "core/template" }, "span": span }] }, "type": { "name": "core/opt", "args": [{ "name": RICH }] }, "span": span },
                    "args": [{ "call": { "fn": "core/rich_text.plain", "args": [read] }, "type": { "name": RICH }, "span": span }],
                },
                "type": { "name": RICH },
                "span": span,
            });
            return None;
        }
        // B8: its parts read, or two compared
        if n.pointer("/record/of").map_or(false, |o| o == RICH) || n.pointer("/field/member").and_then(Value::as_str).map_or(false, |m| m.starts_with("core/rich_text.")) {
            return Some(HELD.into());
        }
        if n.pointer("/field/of/type").map_or(false, |t| is(t, RICH)) {
            return Some(HELD.into());
        }
        if let Some(op) = n.get("op").filter(|o| o["op"] == "==" || o["op"] == "!=") {
            if op["args"].as_array().into_iter().flatten().any(|a| is(&a["type"], RICH)) {
                return Some("a rich_text is drawn in 0.4.0, and two aren't compared: compare the texts they are made from".into());
            }
        }
        if n.get("type").map_or(false, mentions_parts) && (n.get("params").is_some() || n.get("name").is_some()) {
            return Some(HELD.into());
        }
        // B9, C1: block markers 0.3.2 read as words
        if is(&n["type"], RICH) {
            if let Some(t) = n.get("text").and_then(Value::as_str) {
                let (escaped, paragraphs) = sentence(t);
                n["text"] = Value::String(escaped);
                if paragraphs {
                    return Some("0.3.2 read this as one sentence, and 0.4.0 reads its paragraphs as blocks: join them, or keep them as blocks".into());
                }
                return None;
            }
            if let Some(pieces) = n.pointer_mut("/template/pieces").and_then(Value::as_array_mut) {
                let mut paragraphs = false;
                let mut line_start = true;
                for p in pieces.iter_mut() {
                    if let Some(t) = p.as_str() {
                        let (e, more) = if line_start { sentence(t) } else { let (rest, more) = sentence(&format!("x{}", t)); (rest[1..].to_string(), more) };
                        line_start = t.ends_with('\n');
                        paragraphs |= more;
                        *p = Value::String(e);
                    }
                }
                if paragraphs {
                    return Some("0.3.2 read this as one sentence, and 0.4.0 reads its paragraphs as blocks: join them, or keep them as blocks".into());
                }
            }
        }
        // what B7 made reads the text as Markdown on purpose
        let made_here = n.pointer("/call/args/0/call/args/0/get/state").and_then(Value::as_str).map_or(false, |s| held.contains(s));
        if n.pointer("/call/fn").map_or(false, |f| f == "core/rich_text.from") && !made_here {
            return Some("0.4.0's `rich_text.from` reads blocks too, where 0.3.2 read one sentence: a text with a line that starts with `#`, `-`, `>`, `1.` or with a blank line between two is blocks now".into());
        }
        None
    });
}

/// Whether a type is, or holds, one of 0.3.2's parts of a rich_text.
fn mentions_parts(t: &Value) -> bool {
    t["name"].as_str().map_or(false, |n| PARTS.contains(&n)) || t["args"].as_array().into_iter().flatten().any(mentions_parts)
}

/// A text 0.3.2 read as one sentence, with the marks that start a block in 0.4.0 escaped where a line
/// starts with one; and whether it has a blank line between two paragraphs, which no escape joins.
fn sentence(t: &str) -> (String, bool) {
    let mut out = Vec::new();
    let lines: Vec<&str> = t.split('\n').collect();
    let paragraphs = lines.windows(3).any(|w| !w[0].trim().is_empty() && w[1].trim().is_empty() && !w[2].trim().is_empty());
    for line in lines {
        let lead = line.len() - line.trim_start().len();
        let (indent, rest) = line.split_at(lead);
        out.push(if lead <= 3 { format!("{}{}", indent, escaped(rest)) } else { line.to_string() });
    }
    (out.join("\n"), paragraphs)
}

/// A line's start, with what would make it a block escaped.
fn escaped(rest: &str) -> String {
    let b = rest.as_bytes();
    let spaced = |k: usize| b.get(k).map_or(true, |c| *c == b' ' || *c == b'\t');
    match b.first() {
        Some(b'#') => {
            let hashes = rest.chars().take_while(|c| *c == '#').count();
            if hashes <= 6 && spaced(hashes) { format!("\\{}", rest) } else { rest.to_string() }
        }
        Some(b'>') => format!("\\{}", rest),
        Some(b'-' | b'+' | b'*') if spaced(1) => format!("\\{}", rest),
        Some(b'`' | b'~') if rest.starts_with("```") || rest.starts_with("~~~") => format!("\\{}", rest),
        Some(c) if c.is_ascii_digit() => {
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            match b.get(digits) {
                Some(b'.' | b')') if digits <= 9 && spaced(digits + 1) => format!("{}\\{}", &rest[..digits], &rest[digits..]),
                _ => rest.to_string(),
            }
        }
        _ => rest.to_string(),
    }
}
