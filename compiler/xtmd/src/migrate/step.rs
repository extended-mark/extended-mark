//! What the migrations of a run share: an edit of a file's text, a TODO placed above the code a
//! release's step can't move, and the questions a migration asks, numbered, answered and remembered
//! for the run. The pack migrations themselves are applied to the resolved IR (migrate/packs/).

use std::collections::{BTreeMap, BTreeSet};
use std::io::{IsTerminal, Write};

use crate::syntax::ast;
use crate::syntax::{parse, Entry, Span};

/// A text edit: what's between `start` and `end` becomes `text`.
#[derive(Debug, Clone)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// The edits, with an edit inside a larger one left to that one.
pub fn apply(text: &str, edits: &[Edit]) -> String {
    let mut sorted: Vec<&Edit> = edits.iter().collect();
    sorted.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    let mut out = String::new();
    let mut at = 0;
    for e in sorted {
        if e.start < at || (e.start == at && e.end == e.start && out.ends_with(&e.text) && !e.text.is_empty() && false) {
            continue;
        }
        out.push_str(&text[at..e.start]);
        out.push_str(&e.text);
        at = e.end;
    }
    out.push_str(&text[at..]);
    out
}

fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map(|i| i + 1).unwrap_or(0)
}

/// A TODO line above the code at `at`. Its line takes the comment that is right there: `//` in
/// code and inside a call's arguments, `@//` on a line of prose. Inside a fence, a maths block or
/// a case line of `@match`, it goes above the whole block, where a comment is read as one.
pub(crate) fn place_todo(text: &str, edits: &mut Vec<Edit>, at: usize, prose_from: usize, why: &str) {
    fn walk(p: &ast::Prose, at: usize, text: &str) -> Option<(usize, &'static str)> {
        let inside = |s: Span| s.start as usize <= at && at <= s.end as usize;
        for n in &p.nodes {
            match n {
                ast::ProseNode::Fence(ls) | ast::ProseNode::Maths(ls) => {
                    let (first, last) = (ls.first()?, ls.last()?);
                    if inside(first.span.to(last.span)) {
                        return Some((line_start(text, first.span.start as usize), "@//"));
                    }
                }
                ast::ProseNode::Call(c) if inside(c.span) => {
                    if at <= c.call.span.end as usize {
                        let ls = line_start(text, at);
                        return Some((ls, if ls == line_start(text, c.span.start as usize) { "@//" } else { "//" }));
                    }
                    return c.block.as_ref().and_then(|(b, _)| walk(b, at, text));
                }
                ast::ProseNode::If { branches, otherwise, span, .. } if inside(*span) => {
                    return branches.iter().map(|(_, b, _)| b).chain(otherwise.iter().map(|(b, _)| b)).find_map(|b| walk(b, at, text));
                }
                ast::ProseNode::For { body, span, .. } | ast::ProseNode::Slot { body, span, .. } if inside(*span) => return walk(body, at, text),
                ast::ProseNode::Match { cases, span, .. } if inside(*span) => {
                    for c in cases.iter().filter(|c| inside(c.span)) {
                        if line_start(text, at) == line_start(text, c.span.start as usize) {
                            return Some((line_start(text, span.start as usize), "@//"));
                        }
                        return walk(&c.body, at, text);
                    }
                    return None;
                }
                _ => {}
            }
        }
        None
    }
    let ls = line_start(text, at);
    let (mut ls, mark) = if at < prose_from {
        (ls, "//")
    } else {
        let parsed = parse(text, Entry::Document);
        parsed.file.as_ref().and_then(|f| f.prose.as_ref()).and_then(|p| walk(p, at, text)).unwrap_or((ls, "@//"))
    };
    // above an edit that spans the line, so the TODO is never inside replaced text
    while let Some(e) = edits.iter().filter(|e| e.start < ls && ls < e.end).min_by_key(|e| e.start) {
        ls = line_start(text, e.start);
    }
    let indent: String = text[ls..].chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    let line = format!("{}{} TODO(xtmd migrate): {}\n", indent, mark, why);
    // an edit starting on the TODO's own spot takes the line in front of its text
    if let Some(e) = edits.iter_mut().find(|e| e.start == ls && e.end > ls) {
        e.text = format!("{}{}", line, e.text);
    } else {
        edits.push(Edit { start: ls, end: ls, text: line });
    }
}

/// The answers to the questions a migration asks: read from `--answers`, and kept there when
/// someone answers one in a terminal.
#[derive(Default)]
pub struct Answers {
    pub confirm: BTreeMap<String, bool>,
    pub split: BTreeMap<String, String>,
    pub changed: bool,
}

pub static ANSWERS: std::sync::Mutex<Option<Answers>> = std::sync::Mutex::new(None);

pub fn load_answers(text: &str) -> Result<(), String> {
    let v: toml::Value = text.parse().map_err(|e: toml::de::Error| e.message().to_string())?;
    let mut a = Answers::default();
    for (k, x) in v.get("confirm").and_then(|t| t.as_table()).into_iter().flatten() {
        a.confirm.insert(k.clone(), x.as_bool().ok_or_else(|| format!("[confirm] \"{}\" is true or false", k))?);
    }
    for (k, x) in v.get("split").and_then(|t| t.as_table()).into_iter().flatten() {
        a.split.insert(k.clone(), x.as_str().ok_or_else(|| format!("[split] \"{}\" names a case, such as \"OVERDUE\"", k))?.to_string());
    }
    *ANSWERS.lock().unwrap() = Some(a);
    Ok(())
}

/// The answers as a file, when someone answered something new in a terminal.
pub fn answers_text() -> Option<String> {
    let guard = ANSWERS.lock().unwrap();
    let a = guard.as_ref().filter(|a| a.changed)?;
    let q = |k: &str| format!("\"{}\"", k.replace('\\', "\\\\").replace('"', "\\\""));
    let mut out = String::from("# xtmd migrate's answers: kept so a run without a terminal answers the same\n\n[confirm]\n");
    for (k, v) in &a.confirm {
        out.push_str(&format!("{} = {}\n", q(k), v));
    }
    out.push_str("\n[split]\n");
    for (k, v) in &a.split {
        out.push_str(&format!("{} = {}\n", q(k), q(v)));
    }
    Some(out)
}

/// `--allow-todos`: what a migration can't do is left as it was, with a TODO above it, instead of the
/// file being refused.
pub static TODO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `--format json`: no question is asked in a terminal, as where there is none.
pub static NO_ASKING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn can_ask() -> bool {
    std::io::stdin().is_terminal() && !NO_ASKING.load(std::sync::atomic::Ordering::Relaxed)
}

/// `--yes`: every question answered yes, or only the ones numbered (`--yes 1,3`).
pub enum Yes {
    All,
    Numbers(BTreeSet<usize>),
}

pub static YES: std::sync::Mutex<Option<Yes>> = std::sync::Mutex::new(None);

/// Every yes or no answered in this pass: its number, the question, its answer, and who gave it.
pub static ASKED: std::sync::Mutex<Vec<(usize, String, bool, &'static str)>> = std::sync::Mutex::new(Vec::new());

/// A question a migration asks: one yes or no for the whole run (`confirm`), or a choice about
/// one use (`ask`, and `split`), with its options.
#[derive(Clone, Debug, PartialEq)]
pub struct Question {
    /// What identifies it in `--answers`: the `why` of a yes or no, `file:line CASE` of a choice.
    pub key: String,
    pub asks: String,
    /// Empty for a yes or no.
    pub options: Vec<String>,
}

/// Every question the run has met, numbered in the order it first came up (as `--yes 2` names
/// them), and the ones no one has answered yet.
static QUESTIONS: std::sync::Mutex<Vec<Question>> = std::sync::Mutex::new(Vec::new());
static PENDING: std::sync::Mutex<Vec<usize>> = std::sync::Mutex::new(Vec::new());
/// The questions answered in this terminal, by key.
static ANSWERED_HERE: std::sync::Mutex<BTreeSet<String>> = std::sync::Mutex::new(BTreeSet::new());

/// A question's number, registering it the first time it comes up.
fn number(q: Question) -> usize {
    let mut all = QUESTIONS.lock().unwrap();
    match all.iter().position(|x| x.key == q.key) {
        Some(k) => k + 1,
        None => {
            all.push(q);
            all.len()
        }
    }
}

fn wait_for(n: usize) {
    let mut p = PENDING.lock().unwrap();
    if !p.contains(&n) {
        p.push(n);
    }
}

/// A run from the start: no question met, waiting or answered, no answers or `--yes`, and nothing
/// allowed. The xtmd command runs once; an editor migrates again and again.
pub fn reset() {
    *ANSWERS.lock().unwrap() = None;
    TODO.store(false, std::sync::atomic::Ordering::Relaxed);
    NO_ASKING.store(false, std::sync::atomic::Ordering::Relaxed);
    *YES.lock().unwrap() = None;
    ASKED.lock().unwrap().clear();
    QUESTIONS.lock().unwrap().clear();
    PENDING.lock().unwrap().clear();
    ANSWERED_HERE.lock().unwrap().clear();
}

/// A new pass over the files: what was answered stays, what was waiting is met again.
pub fn new_pass() {
    ASKED.lock().unwrap().clear();
    PENDING.lock().unwrap().clear();
}

/// The questions this pass met that no one has answered, with their numbers, in order.
pub fn pending() -> Vec<(usize, Question)> {
    let all = QUESTIONS.lock().unwrap();
    let mut p = PENDING.lock().unwrap().clone();
    p.sort();
    p.into_iter().map(|n| (n, all[n - 1].clone())).collect()
}

/// In a terminal: every question waiting, listed, then each asked in turn until it gets y or n
/// (a choice, its number). `false` when the terminal closes before every one is answered.
pub fn ask_in_terminal(questions: &[(usize, Question)]) -> bool {
    println!("\n{} question{} to answer before anything is written:", questions.len(), if questions.len() == 1 { "" } else { "s" });
    for (n, q) in questions {
        println!("  {}. {}{}", n, q.asks, if q.options.is_empty() { String::new() } else { format!(" ({})", q.options.join(" or ")) });
    }
    println!();
    let mut guard = ANSWERS.lock().unwrap();
    let answers = guard.get_or_insert_with(Answers::default);
    for (n, q) in questions {
        let mut again = false;
        loop {
            // asked once in full; an answer that isn't one is asked for again, briefly
            match (q.options.is_empty(), again) {
                (true, false) => print!("{}. {} [y/n] ", n, q.asks),
                (true, true) => print!("   y or n: "),
                (false, false) => {
                    println!("{}. {}", n, q.asks);
                    for (k, o) in q.options.iter().enumerate() {
                        println!("     {}) {}", k + 1, o);
                    }
                    print!("   [1-{}] ", q.options.len());
                }
                (false, true) => print!("   a number from 1 to {}: ", q.options.len()),
            }
            again = true;
            let _ = std::io::stdout().flush();
            let mut s = String::new();
            if std::io::stdin().read_line(&mut s).map_or(true, |read| read == 0) {
                println!();
                return false;
            }
            let s = s.trim();
            if q.options.is_empty() {
                let a = match s {
                    "y" | "Y" | "yes" => true,
                    "n" | "N" | "no" => false,
                    _ => continue,
                };
                answers.confirm.insert(q.key.clone(), a);
            } else {
                let Some(pick) = s.parse::<usize>().ok().filter(|k| *k >= 1 && *k <= q.options.len()) else { continue };
                answers.split.insert(q.key.clone(), q.options[pick - 1].clone());
            }
            answers.changed = true;
            ANSWERED_HERE.lock().unwrap().insert(q.key.clone());
            break;
        }
    }
    true
}

/// How a question no one answered can be: by number, or in the answers file.
pub fn how_to_answer(n: usize, q: &Question) -> String {
    let quoted = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    if q.options.is_empty() {
        format!("--yes {}, or in --answers: [confirm] {} = true", n, quoted(&q.key))
    } else {
        format!("in --answers: [split] {} = {}", quoted(&q.key), quoted(&q.options[0]))
    }
}

/// A yes or no for the whole run: from the answers or `--yes`; else it waits, to be asked once
/// every question is known, and counts as no in the meantime.
pub(crate) fn confirm(question: &str) -> Option<bool> {
    let mut asked = ASKED.lock().unwrap();
    if let Some((_, _, a, _)) = asked.iter().find(|(_, q, _, _)| q == question) {
        return Some(*a);
    }
    let n = number(Question { key: question.to_string(), asks: question.to_string(), options: Vec::new() });
    let mut guard = ANSWERS.lock().unwrap();
    let answers = guard.get_or_insert_with(Answers::default);
    let here = ANSWERED_HERE.lock().unwrap().contains(question);
    let (a, by) = match (answers.confirm.get(question), &*YES.lock().unwrap()) {
        (Some(a), _) => (*a, if here { "asked" } else { "--answers" }),
        (None, Some(Yes::All)) => (true, "--yes"),
        (None, Some(Yes::Numbers(ns))) => (ns.contains(&n), "--yes"),
        (None, None) => {
            wait_for(n);
            return Some(false);
        }
    };
    asked.push((n, question.to_string(), a, by));
    Some(a)
}

/// A choice about one use: from the answers, by `key`; else it waits to be asked, or with
/// `--allow-todos` is left for a person (`None`).
pub(crate) fn ask(key: &str, question: &str, options: &[String]) -> Option<String> {
    let mut guard = ANSWERS.lock().unwrap();
    let answers = guard.get_or_insert_with(Answers::default);
    if let Some(a) = answers.split.get(key) {
        return options.iter().find(|o| *o == a).cloned();
    }
    if !options.is_empty() && !TODO.load(std::sync::atomic::Ordering::Relaxed) {
        wait_for(number(Question { key: key.to_string(), asks: question.to_string(), options: options.to_vec() }));
    }
    None
}
