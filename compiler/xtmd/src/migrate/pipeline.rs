//! A tree moved from the release it was written for to this one, by what each release understands of
//! it, never by its text: the old release reads it (readers/), the resolved IR it gives is brought to
//! this xtmd's version (rir/), decompile prints every file again, and this xtmd compiles what was
//! printed and writes each document's Markdown, which has to be the old release's. Every file is
//! planned before any is written: a plan with anything refused, or a question open, writes nothing.

use std::collections::BTreeMap;

use crate::check::Source;
use crate::migrate::readers::Read;
use crate::migrate::rir::{self, Asked, Question, Step};

/// A file as the plan would write it.
pub struct Planned {
    pub path: String,
    pub text: String,
}

/// What moving a tree comes to: every file's new text, or why nothing can be written.
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
}

impl Plan {
    /// Whether every file can be written.
    pub fn ready(&self) -> bool {
        self.refused.is_empty() && self.open.is_empty()
    }
}

/// What this xtmd makes of the files printed: for each document, by its path, its Markdown, or the
/// errors it doesn't compile with. Packs' files are compiled with the documents that import them.
pub type Check<'a> = &'a dyn Fn(&[Source]) -> BTreeMap<String, Result<String, Vec<String>>>;

/// The plan for what `read` read: through `steps`, with `answers` to their questions.
pub fn plan(read: Read, steps: &[Step], answers: BTreeMap<String, String>, check: Check) -> Plan {
    let mut plan = Plan { from_release: read.release.clone(), from_rir: rir::version(&read.rir).unwrap_or(0), ..Plan::default() };
    let mut v = read.rir;
    let mut asked = Asked::with(answers);
    let stepped = rir::migrate(&mut v, steps, &mut asked);
    plan.open = asked.open;
    plan.todos = asked.todos;
    if let Err(e) = stepped {
        plan.refused.extend(e.into_iter().map(|e| format!("the resolved IR {} read: {}", read.release, e)));
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
    for p in &printed {
        if !crate::migrate::is_doc(&p.path) {
            continue;
        }
        match compiled.get(&p.path) {
            None => plan.refused.push(format!("{}: this xtmd didn't compile it", p.path)),
            Some(Err(errors)) => plan.refused.push(format!("{}: what was printed doesn't compile: {}", p.path, errors.join("; "))),
            Some(Ok(now)) => match read.markdown.get(&p.path) {
                None => plan.refused.push(format!("{}: {} gave no Markdown for it, so nothing says it reads the same", p.path, read.release)),
                Some(was) if was.trim_end() != now.trim_end() => plan.refused.push(format!("{}: its Markdown changed: {}", p.path, first_change(was, now))),
                Some(_) => {}
            },
        }
    }
    plan.files = printed.into_iter().map(|p| Planned { path: p.path, text: p.text }).collect();
    plan
}

/// The first line that differs, as it was and as it is.
fn first_change(was: &str, now: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (was.trim_end().lines().collect(), now.trim_end().lines().collect());
    let at = (0..a.len().max(b.len())).find(|&k| a.get(k) != b.get(k)).unwrap_or(0);
    let show = |l: Option<&&str>| l.map_or("(nothing)".to_string(), |l| format!("{:?}", l));
    format!("line {} was {} and is {}", at + 1, show(a.get(at)), show(b.get(at)))
}
