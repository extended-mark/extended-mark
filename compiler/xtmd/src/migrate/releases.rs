//! Steps between releases of Extended Mark: what a release changes in the files written for an earlier
//! one, beyond any pack's migrations. Each step is made with Extended Mark's own reader, or with what
//! the compiler finds in the files, never by searching the text. `xtmd migrate` runs the steps of every
//! release after the one a tree is written for (xtmd.toml's `xtmd`), oldest first and before anything
//! else.
//!
//! A step that knows what the calls of a file mean is given the files compiled, with the packs they
//! use. What it can't settle it asks, or leaves for a person with a TODO, as a pack's migrations do.

use crate::check::{Compiled, Source};
use crate::syntax::Entry;

use crate::migrate::release_of;
use crate::migrate::step::{apply, place_todo, Edit};

/// One release's change to a file.
pub struct ReleaseStep {
    /// The release that makes the change: the step runs on files written for an earlier one.
    pub release: &'static str,
    /// What changes and why, as whoever migrates is told.
    pub why: &'static str,
    pub by: By,
}

pub enum By {
    /// The edits made to one file, read as a document or a `.xtmdpack` file; an error says why the
    /// file can't be moved, and refuses it.
    Text(fn(&str, Entry) -> Result<Vec<Edit>, String>),
    /// The changes made to a document, or to a pack's files together, as they compile now. Run again
    /// on what it leaves until it changes nothing.
    Compiled(fn(&Files) -> Changes),
}

/// The files a step is given, and what compiling them with this release found.
pub struct Files<'a> {
    pub files: &'a [Source],
    pub compiled: &'a Compiled,
}

impl Files<'_> {
    pub fn text(&self, path: &str) -> Option<&str> {
        self.files.iter().find(|f| f.path == path).map(|f| f.text.as_str())
    }
}

/// What a step changes: edits to a file, and what is left for a person, at a place in a file.
#[derive(Default)]
pub struct Changes {
    pub edits: Vec<(String, Edit)>,
    pub todos: Vec<(String, usize, String)>,
}

/// Every release's steps, oldest first. 0.3.0 is the first release of 0.3.
pub static STEPS: &[ReleaseStep] = &[
    ReleaseStep { release: "0.4.0", why: "`warn` is a keyword, so a name spelled so is `warn_`; a host that reads it or sets it reads the new name", by: By::Text(crate::migrate::externs::names) },
    ReleaseStep { release: "0.4.0", why: "every extern widget says what it is as Markdown, and one that said nothing is `markdown { }`", by: By::Text(crate::migrate::externs::markdown) },
    ReleaseStep { release: "0.4.0", why: "`md` is `rich_text`, which holds blocks too, and one is placed where it is drawn", by: By::Compiled(crate::migrate::texts::step) },
    ReleaseStep { release: "0.4.0", why: "a slot is filled by every call unless it is `opt`, and a plain block goes to a widget with one slot", by: By::Compiled(crate::migrate::slots::step) },
];

/// The steps of the releases after `from`, up to `to` and including it, oldest first.
pub fn between<'s>(from: &str, to: &str, steps: &'s [ReleaseStep]) -> Vec<&'s ReleaseStep> {
    let (Some(from), Some(to)) = (release_of(from), release_of(to)) else { return Vec::new() };
    let mut out: Vec<&ReleaseStep> = steps.iter().filter(|s| release_of(s.release).map_or(false, |r| r > from && r <= to)).collect();
    out.sort_by_key(|s| release_of(s.release));
    out
}

/// Files moved through the steps.
#[derive(Default)]
pub struct Moved {
    pub files: Vec<Source>,
    /// What each step that changed a file says, by its path.
    pub said: std::collections::BTreeMap<String, Vec<String>>,
    /// What `--allow-todos` left for a person, by path.
    pub todos: std::collections::BTreeMap<String, Vec<String>>,
    /// Why a file can't be moved, by path.
    pub refused: std::collections::BTreeMap<String, String>,
}

/// A document, or a pack's files, moved through the steps. `compile` compiles the files with the packs
/// they use.
pub fn run(files: &[Source], steps: &[&ReleaseStep], compile: &dyn Fn(&[Source]) -> Compiled) -> Moved {
    let mut m = Moved { files: files.to_vec(), ..Moved::default() };
    for s in steps {
        let label = |why: &str| format!("xtmd {}: {}", s.release, why);
        let mut changed: std::collections::BTreeSet<String> = Default::default();
        match &s.by {
            By::Text(edits) => {
                for f in m.files.iter_mut() {
                    let Some(entry) = crate::syntax::entry_for(&f.path) else { continue };
                    match edits(&f.text, entry) {
                        Err(e) => {
                            m.refused.insert(f.path.clone(), label(&e));
                        }
                        Ok(edits) if !edits.is_empty() => {
                            f.text = apply(&f.text, &edits);
                            changed.insert(f.path.clone());
                        }
                        Ok(_) => {}
                    }
                }
            }
            By::Compiled(step) => {
                // each round leaves files that compile further, so what was hidden is found; a file that
                // a round leaves as it was is done
                for _ in 0..8 {
                    let compiled = compile(&m.files);
                    let done = step(&Files { files: &m.files, compiled: &compiled });
                    let mut again = false;
                    for f in m.files.iter_mut() {
                        let mut edits: Vec<Edit> = done.edits.iter().filter(|(p, _)| *p == f.path).map(|(_, e)| e.clone()).collect();
                        let left: Vec<&(String, usize, String)> = done.todos.iter().filter(|(p, ..)| *p == f.path).collect();
                        again |= !edits.is_empty();
                        if edits.is_empty() && !left.is_empty() {
                            let whys: Vec<String> = left.iter().map(|(_, at, why)| format!("line {}: {}", crate::syntax::Lines::new(&f.text).place(*at as u32).line, why)).collect();
                            if !crate::migrate::step::TODO.load(std::sync::atomic::Ordering::Relaxed) {
                                m.refused.insert(f.path.clone(), format!("{}; --allow-todos writes it with a TODO at each", label(&whys.join("; "))));
                            } else if !m.todos.contains_key(&f.path) {
                                let prose_from = crate::syntax::divider_at(&f.text).unwrap_or(if f.path.ends_with(".xtmd") { 0 } else { usize::MAX });
                                for (_, at, why) in &left {
                                    place_todo(&f.text, &mut edits, *at, prose_from, why);
                                }
                                m.todos.insert(f.path.clone(), whys);
                            }
                        }
                        if !edits.is_empty() {
                            f.text = apply(&f.text, &edits);
                            changed.insert(f.path.clone());
                        }
                    }
                    if !again {
                        break;
                    }
                }
            }
        }
        for p in changed {
            m.said.entry(p).or_default().push(label(s.why));
        }
    }
    m
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::syntax::ast::ProseNode;

    /// A release that renamed core's `rule` to `divider`: every call of it in prose.
    fn rule(text: &str, entry: Entry) -> Result<Vec<Edit>, String> {
        let parsed = crate::syntax::parse(text, entry);
        let file = parsed.file.ok_or("it doesn't read")?;
        let mut out = Vec::new();
        for n in file.prose.iter().flat_map(|p| p.nodes.iter()) {
            if let ProseNode::Call(c) = n {
                if c.call.name.name == "rule" {
                    out.push(Edit { start: c.call.name.span.start as usize, end: c.call.name.span.end as usize, text: "divider".into() });
                }
            }
        }
        Ok(out)
    }

    fn broken(_: &str, _: Entry) -> Result<Vec<Edit>, String> {
        Err("a form only a person can rewrite".into())
    }

    pub const STEPS: &[ReleaseStep] = &[
        ReleaseStep { release: "0.3.2", why: "`rule` is `divider` now", by: By::Text(rule) },
        ReleaseStep { release: "0.3.1", why: "nothing in these files", by: By::Text(|_, _| Ok(Vec::new())) },
        ReleaseStep { release: "0.4.0", why: "later", by: By::Text(broken) },
    ];

    #[test]
    fn a_tree_takes_the_steps_of_each_release_after_its_own_oldest_first() {
        let names = |v: Vec<&ReleaseStep>| v.iter().map(|s| s.release).collect::<Vec<_>>();
        assert_eq!(names(between("0.3.0", "0.3.2", STEPS)), ["0.3.1", "0.3.2"]);
        assert_eq!(names(between("0.3.1", "0.3.2", STEPS)), ["0.3.2"]);
        assert!(between("0.3.2", "0.3.2", STEPS).is_empty());
        assert_eq!(names(between("0.3.0", "0.4.0", STEPS)), ["0.3.1", "0.3.2", "0.4.0"]);
        assert!(between("0.3.0", "0.3.0", super::STEPS).is_empty());
    }

    #[test]
    fn a_step_edits_what_the_reader_finds_and_says_why() {
        let doc = "# Notes\n\nA rule, in words.\n\n@rule();\n";
        let files = [Source { path: "notes.xtmd".into(), text: doc.into() }];
        let no = |_: &[Source]| -> Compiled { unreachable!("a step over a file's text compiles nothing") };
        let m = run(&files, &between("0.3.0", "0.3.2", STEPS), &no);
        assert_eq!(m.files[0].text, "# Notes\n\nA rule, in words.\n\n@divider();\n");
        assert_eq!(m.said["notes.xtmd"], ["xtmd 0.3.2: `rule` is `divider` now"]);
        let m = run(&files, &between("0.3.2", "0.4.0", STEPS), &no);
        assert_eq!(m.refused["notes.xtmd"], "xtmd 0.4.0: a form only a person can rewrite");
    }
}
