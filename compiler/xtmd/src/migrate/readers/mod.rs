//! Readers: how a tree becomes a resolved IR. The release that wrote a file is the one that
//! understands it, so each release's files are read by that release's own front end, and this xtmd
//! works only on what it gives: the resolved IR and each document's Markdown. This xtmd carries two:
//! its own, for a tree written for it, and its previous release's, copied into compiler/xtmd/previous/ at
//! each release, for a tree written for that one. Nothing older: a tree further back is moved by each
//! release in between, in turn. The previous front end is built into the command line only, never into
//! the WebAssembly build.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::check::{compile_rir, Checker, Source};

/// The release whose front end this xtmd carries: a tree written for it is the only one moved by a step.
pub const PREVIOUS: &str = "0.3.2";

/// What a release read of a tree: its resolved IR, every file in it, and each document's Markdown, by
/// its path, as that release writes it.
pub struct Read {
    pub release: String,
    pub rir: Value,
    pub markdown: BTreeMap<String, String>,
}

pub trait Reader {
    /// The release it reads as.
    fn release(&self) -> String;
    /// The tree's files, each one a document or a pack's file, built with `packs`, the other packs'
    /// files; or what that release finds wrong with them, each with its file.
    fn read(&self, files: &[Source], packs: &[Source]) -> Result<Read, Vec<String>>;
}

#[cfg(feature = "cli")]
pub mod previous;

/// The previous release's front end, when this xtmd is built with it: the command line is.
pub fn previous() -> Option<Box<dyn Reader>> {
    #[cfg(feature = "cli")]
    return Some(Box::new(previous::Previous::new()));
    #[cfg(not(feature = "cli"))]
    None
}

/// This xtmd's own front end, for a tree written for it: its pack migrations are the only move.
pub struct This<'a> {
    pub base: &'a Checker,
}

impl Reader for This<'_> {
    fn release(&self) -> String {
        crate::migrate::RELEASE.to_string()
    }

    fn read(&self, files: &[Source], packs: &[Source]) -> Result<Read, Vec<String>> {
        let own: Vec<Source> = files.iter().filter(|f| !crate::migrate::is_doc(&f.path)).cloned().collect();
        let (mut all, mut markdown, mut errors, mut top) = (Vec::new(), BTreeMap::new(), Vec::new(), None);
        for f in files {
            let doc = crate::migrate::is_doc(&f.path);
            let mut world: Vec<Source> = packs.to_vec();
            world.extend(own.iter().filter(|p| p.path != f.path).cloned());
            let (c, r) = compile_rir(self.base, &[world], &[f.clone()], if doc { "doc" } else { "main" });
            let said: Vec<String> = c.diagnostics.iter().filter(|d| d.diagnostic.is_error()).map(|d| format!("{}: {}", f.path, d.diagnostic.message)).collect();
            let Some(r) = r.filter(|_| said.is_empty()) else {
                errors.extend(said);
                continue;
            };
            for file in r["files"].as_array().into_iter().flatten() {
                if file["path"] == f.path.as_str() && !all.iter().any(|x: &Value| x["path"] == file["path"]) {
                    all.push(file.clone());
                }
            }
            if doc {
                if let Some(mut m) = c.module {
                    crate::lower::lower(&mut m);
                    if let Ok(b) = crate::markdown::page(&m) {
                        markdown.insert(f.path.clone(), crate::markdown::write_page(m.front_matter.as_deref(), &b));
                    }
                }
            }
            top.get_or_insert(r);
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let mut rir = top.unwrap_or_else(|| json!({ "rir": crate::migrate::rir::VERSION, "xtmd": crate::migrate::RELEASE, "packs": [] }));
        rir.as_object_mut().map(|o| o.remove("levels"));
        rir["files"] = Value::Array(all);
        Ok(Read { release: self.release(), rir, markdown })
    }
}
