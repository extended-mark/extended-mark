//! A tree moved from the release before this one to this one, by what each release understands of it,
//! never by its text: the release that wrote it reads it (readers/), its resolved IR is moved one step
//! to this xtmd's (rir/), decompile prints every file again, and this xtmd compiles what was printed
//! back to its resolved IR, which has to be what the step made: the files written mean what the step
//! decided. Each document's Markdown is compared with the old release's, and a difference is a
//! warning, never a refusal: the Markdown misses what a page does, and a step may change it on purpose.
//!
//! A migration speeds up what is plain and never promises the result: what a step can't settle it
//! leaves at its place as a TODO, and the run lists it. A file is refused only when it can't be
//! written, and then nothing is.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::check::Source;
use crate::migrate::readers::Read;
use crate::migrate::rir::{self, Asked, Question, Step};

/// A file as the plan would write it.
pub struct Planned {
    pub path: String,
    pub text: String,
}

/// What moving a tree comes to: every file's new text, what a person must look at, or why nothing can
/// be written.
#[derive(Default)]
pub struct Plan {
    /// The release that read it, and the resolved IR's version it gave.
    pub from_release: String,
    pub from_rir: u64,
    pub files: Vec<Planned>,
    /// Why it can't be written, each with the file it is about.
    pub refused: Vec<String>,
    /// Questions a step asked that no answer was given for.
    pub open: Vec<Question>,
    /// The TODOs written above what a step couldn't move.
    pub todos: Vec<String>,
    /// What is worth a look and needs nothing: a document whose Markdown changed.
    pub warnings: Vec<String>,
}

impl Plan {
    /// Whether every file can be written.
    pub fn ready(&self) -> bool {
        self.refused.is_empty() && self.open.is_empty()
    }

    /// Whether what is written leaves anything for a person.
    pub fn for_a_person(&self) -> bool {
        !self.todos.is_empty()
    }
}

/// A printed file, compiled by this xtmd: its resolved IR's entry for the file, and a document's
/// Markdown.
pub struct Checked {
    pub rir: Value,
    pub markdown: Option<String>,
}

/// What this xtmd makes of the files printed, by each one's path: what it compiles to, or the errors it
/// doesn't compile with. Packs' files are compiled with the documents that import them.
pub type Check<'a> = &'a dyn Fn(&[Source]) -> BTreeMap<String, Result<Checked, Vec<String>>>;

/// The plan for what `read` read: through `step` when it was read by the release before this one,
/// with `answers` to its questions.
pub fn plan(read: Read, step: Option<&Step>, answers: BTreeMap<String, String>, check: Check) -> Plan {
    let mut plan = Plan { from_release: read.release.clone(), from_rir: rir::version(&read.rir).unwrap_or(0), ..Plan::default() };
    let mut v = read.rir;
    let mut asked = Asked::with(answers);
    let stepped = rir::migrate(&mut v, step, &mut asked);
    plan.open = asked.open;
    plan.todos = asked.todos;
    if let Err(e) = stepped {
        plan.refused.extend(e.into_iter().map(|e| format!("the resolved IR {} read: {}", read.release, e)));
        return plan;
    }
    // what the step made while a question is open isn't what it will make: nothing is printed till then
    if !plan.open.is_empty() {
        return plan;
    }
    let printed = match crate::decompile::decompile(&v) {
        Ok(p) => p,
        Err(e) => {
            plan.refused.extend(e.into_iter().map(|e| format!("it can't be printed back: {}", e)));
            return plan;
        }
    };
    let sources: Vec<Source> = printed.iter().map(|p| Source { path: p.path.clone(), text: p.text.clone() }).collect();
    let compiled = check(&sources);
    let made: BTreeMap<&str, &Value> = v["files"].as_array().into_iter().flatten().filter_map(|f| Some((f["path"].as_str()?, f))).collect();
    // a file left with a TODO is written whether or not it compiles: what it can't compile with until a
    // person has done what the TODO says is said beside it
    let todo_in = |path: &str| plan.todos.iter().any(|t| t.strip_prefix(path).map_or(false, |r| r.starts_with(": ")));
    for p in &printed {
        match compiled.get(&p.path) {
            None => plan.refused.push(format!("{}: this xtmd didn't compile it", p.path)),
            Some(Err(errors)) if todo_in(&p.path) => plan.warnings.push(format!("{}: it doesn't compile yet, and is written with its TODOs for a person: {}", p.path, errors.join("; "))),
            Some(Err(errors)) => plan.refused.push(format!("{}: what was printed doesn't compile: {}", p.path, errors.join("; "))),
            Some(Ok(now)) => {
                if let Some(d) = made.get(p.path.as_str()).and_then(|was| first_difference(&written(was), &written(&now.rir), "")) {
                    plan.refused.push(format!("{}: what was printed compiles to another resolved IR than the step made, at {}", p.path, d));
                }
                if let (Some(was), Some(now)) = (read.markdown.get(&p.path), &now.markdown) {
                    if was.trim_end() != now.trim_end() {
                        plan.warnings.push(format!("{}: its Markdown changed: {}", p.path, first_change(was, now)));
                    }
                }
            }
        }
    }
    plan.files = printed.into_iter().map(|p| Planned { path: p.path, text: p.text }).collect();
    plan
}

/// A file's resolved IR as it is written: without where its parts stand, which printing changes, and
/// without the types the checker gives each expression, which this xtmd works out again from what is
/// written. Every name's target stays: the files written must mean the same declarations.
pub fn written(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.iter().filter(|(k, _)| !(k.as_str() == "span" || k.ends_with("_span") || k.as_str() == "sha256" || k.as_str() == "type")).map(|(k, x)| (k.clone(), written(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(written).collect()),
        other => other.clone(),
    }
}

/// Where two values first differ, as a path into them, or none.
pub fn first_difference(a: &Value, b: &Value, at: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            keys.into_iter().find_map(|k| first_difference(x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null), &format!("{}.{}", at, k)))
        }
        (Value::Array(x), Value::Array(y)) => (0..x.len().max(y.len())).find_map(|k| first_difference(x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null), &format!("{}[{}]", at, k))),
        _ if a == b => None,
        _ => Some(format!("{}: {} became {}", if at.is_empty() { "the file" } else { at.trim_start_matches('.') }, short(a), short(b))),
    }
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    match s.char_indices().nth(120) {
        Some((k, _)) => format!("{}…", &s[..k]),
        None => s,
    }
}

/// The first line that differs, as it was and as it is.
fn first_change(was: &str, now: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (was.trim_end().lines().collect(), now.trim_end().lines().collect());
    let at = (0..a.len().max(b.len())).find(|&k| a.get(k) != b.get(k)).unwrap_or(0);
    let show = |l: Option<&&str>| l.map_or("(nothing)".to_string(), |l| format!("{:?}", l));
    format!("line {} was {} and is {}", at + 1, show(a.get(at)), show(b.get(at)))
}
