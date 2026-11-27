//! The release before's reader: its own front end (crate::previous, copied from its tag), which gives
//! each file's resolved IR and each document's Markdown as that release did, with its own core. A file
//! it finds an error in isn't read: a migration starts from a tree valid for the release it is written for.
//!
//! 0.3's resolved IR, rir 1, has no place for some of what is written: a front matter's text, a member's
//! doc comments, a comment at a body's edge or alone in an empty one. The release before's own reading of
//! the file has them, so each file's resolved IR is given what it left out, `left_out` (a part rir-1's
//! schema in src/migrate/rir adds): every comment of the code that isn't among rir 1's, with where it
//! stands, the front matter's text, where the release before divided the code from the prose, and
//! `now`: what this release's parser makes of the file as far as it can, from which the step takes where
//! each comment goes, by the spans both give the same text. Nothing written in a 0.3 file is lost.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::check::Source;
use crate::migrate::readers::{Read, Reader};
use crate::previous::check as old;

pub struct Previous {
    base: old::Checker,
    /// This release's, for where its parser puts each comment.
    now: crate::check::Checker,
}

impl Previous {
    pub fn new() -> Self {
        let core: Vec<old::Source> = crate::previous::core_files::SOURCES.iter().map(|(p, t)| old::Source { path: p.to_string(), text: t.to_string() }).collect();
        Previous { base: old::compile_core(&core), now: crate::check::compile_core(&crate::cli::core_sources()) }
    }

    /// Where this release's parser puts the file's comments: what it makes of the file as far as it
    /// can, even with errors (its language may have changed), or nothing when it can't read it at all.
    fn comments_now(&self, f: &Source, world: &[Source]) -> Value {
        let opts = crate::check::RirOptions { core: false, partial: true };
        let module = if crate::migrate::is_doc(&f.path) { "doc" } else { "main" };
        let (_, r) = crate::check::compile_rir_with(&self.now, &[world.to_vec()], &[f.clone()], module, opts);
        r.and_then(|r| r["files"].as_array()?.iter().find(|x| x["path"] == f.path.as_str()).cloned()).unwrap_or(Value::Null)
    }
}

impl Default for Previous {
    fn default() -> Self {
        Self::new()
    }
}

fn old_source(s: &Source) -> old::Source {
    old::Source { path: s.path.clone(), text: s.text.clone() }
}

impl Reader for Previous {
    fn release(&self) -> String {
        crate::previous::RELEASE.to_string()
    }

    fn read(&self, files: &[Source], packs: &[Source]) -> Result<Read, Vec<String>> {
        let own: Vec<&Source> = files.iter().filter(|f| !crate::migrate::is_doc(&f.path)).collect();
        let (mut all, mut markdown, mut errors, mut top) = (Vec::new(), BTreeMap::new(), Vec::new(), None);
        for f in files {
            let doc = crate::migrate::is_doc(&f.path);
            let mut world: Vec<old::Source> = packs.iter().map(old_source).collect();
            world.extend(own.iter().filter(|p| p.path != f.path).map(|p| old_source(p)));
            let (c, r) = old::compile_rir(&self.base, &[world], &[old_source(f)], if doc { "doc" } else { "main" });
            let said: Vec<String> = c.diagnostics.iter().filter(|d| d.diagnostic.is_error()).map(|d| format!("{}: {}", d.file, d.diagnostic.message)).collect();
            let Some(r) = r.filter(|_| said.is_empty()) else {
                errors.extend(if said.is_empty() { vec![format!("{}: {} gave no resolved IR", f.path, self.release())] } else { said });
                continue;
            };
            for file in r["files"].as_array().into_iter().flatten() {
                if file["path"] == f.path.as_str() && !all.iter().any(|x: &Value| x["path"] == file["path"]) {
                    let mut file = file.clone();
                    file["left_out"] = left_out(&f.path, &f.text, &file);
                    let mut now_world: Vec<Source> = packs.to_vec();
                    now_world.extend(own.iter().filter(|p| p.path != f.path).map(|p| (*p).clone()));
                    file["left_out"]["now"] = self.comments_now(f, &now_world);
                    all.push(file);
                }
            }
            if doc {
                if let Some(mut m) = c.module {
                    crate::previous::lower::lower(&mut m);
                    match crate::previous::markdown::page(&m) {
                        Ok(b) => {
                            markdown.insert(f.path.clone(), crate::previous::markdown::write_page(m.front_matter.as_deref(), &b));
                        }
                        Err(e) => errors.push(format!("{}: {} can't write its Markdown: {}", f.path, self.release(), e.0)),
                    }
                }
            }
            top.get_or_insert(r);
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let mut rir = top.unwrap_or_else(|| json!({ "rir": 1, "xtmd": self.release(), "packs": [] }));
        rir.as_object_mut().map(|o| o.remove("levels"));
        rir["files"] = Value::Array(all);
        Ok(Read { release: self.release(), rir, markdown })
    }
}

/// Every comment of a file's code, as the release before's lexer reads it, with where it stands.
pub fn comments_of(path: &str, src: &str) -> (Vec<(usize, usize, String)>, Option<(usize, usize)>, Option<(usize, usize)>) {
    use crate::previous::syntax::{self as syn, lexer};
    let entry = syn::entry_for(path).unwrap_or(syn::parser::Entry::Document);
    let parsed = syn::parse(src, entry);
    let file = parsed.file.as_ref();
    let divider = file.and_then(|f| f.divider).map(|s| (s.start as usize, s.end as usize));
    let front = file.and_then(|f| f.front_matter).map(|s| (s.start as usize, s.end as usize));
    // the code: after the front matter, up to the divider
    let start = front.map_or(0, |f| f.1);
    let end = divider.map_or(src.len(), |d| d.0);
    // a widget's `markdown` block is prose, which the lexer doesn't read: the code is lexed around each
    let mut prose: Vec<(usize, usize)> = Vec::new();
    fn blocks(v: &Value, out: &mut Vec<(usize, usize)>) {
        match v {
            Value::Object(o) => {
                if o.contains_key("empty") && o.contains_key("prose") {
                    if let (Some(a), Some(b)) = (o["span"]["start"].as_u64(), o["span"]["end"].as_u64()) {
                        out.push((a as usize, b as usize));
                    }
                }
                o.values().for_each(|x| blocks(x, out));
            }
            Value::Array(a) => a.iter().for_each(|x| blocks(x, out)),
            _ => {}
        }
    }
    if let Some(f) = file {
        blocks(&serde_json::to_value(f).unwrap_or(Value::Null), &mut prose);
    }
    prose.sort();
    let mut parts = Vec::new();
    let mut at = start;
    for (a, b) in prose.into_iter().filter(|(a, b)| *a >= start && *b <= end) {
        parts.push((at, a));
        at = b;
    }
    parts.push((at, end));
    let mut out = Vec::new();
    for (a, b) in parts {
        let mut lx = lexer::Lexer::new(src, a, b);
        while let Ok(t) = lx.next() {
            for tr in &t.trivia {
                if let lexer::Trivia::Comment(c) = tr {
                    out.push((c.span.start as usize, c.span.end as usize, c.text.clone()));
                }
            }
            if matches!(t.kind, lexer::Tok::Eof) {
                break;
            }
        }
    }
    (out, front, divider)
}

/// What rir 1 left out of a file: its comments that aren't among those it kept, the front matter's text,
/// and the divider's place.
fn left_out(path: &str, src: &str, file: &Value) -> Value {
    let mut kept: BTreeMap<String, usize> = BTreeMap::new();
    fn walk(v: &Value, kept: &mut BTreeMap<String, usize>) {
        match v {
            Value::Object(m) => {
                for (k, x) in m {
                    if k == "comments" || k == "before" || k == "after" {
                        for c in x.as_array().into_iter().flatten().filter_map(Value::as_str) {
                            *kept.entry(c.to_string()).or_default() += 1;
                        }
                    }
                    walk(x, kept);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, kept)),
            _ => {}
        }
    }
    walk(file, &mut kept);
    let (all, front, divider) = comments_of(path, src);
    let mut comments = Vec::new();
    for (start, end, text) in all {
        match kept.get_mut(&text).filter(|n| **n > 0) {
            Some(n) => *n -= 1,
            None => comments.push(json!({ "span": [start, end], "text": text })),
        }
    }
    let mut o = serde_json::Map::new();
    o.insert("comments".into(), Value::Array(comments));
    if let Some((a, b)) = front {
        o.insert("front_matter".into(), json!(src[a..b].to_string()));
    }
    if let Some((a, b)) = divider {
        o.insert("divider".into(), json!([a, b]));
    }
    Value::Object(o)
}
