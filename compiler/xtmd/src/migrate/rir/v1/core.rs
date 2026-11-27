//! core@1 to core@2, A2, A3 and D3: core is a pack like any other, and its migration
//! (core/migrations/core@2.xtmdpack) is applied as any pack's is (migrate/packs/), to each file's
//! resolved IR once the language part has moved it. What it can't move is a TODO above the node.

use serde_json::{json, Value};

use crate::check::Source;
use crate::migrate::packs::{Done, Moving};
use crate::migrate::rir::Asked;

/// Every file moved by core's migration from core@1; a pack's `uses core@1` becomes `uses core@2`.
pub fn migrated(v: &mut Value, base: &crate::check::Checker, asked: &mut Asked) -> Result<(), Vec<String>> {
    let sources: Vec<Source> = crate::live::core().into_iter().chain(crate::live::CORE_HISTORY.iter().map(|(p, t)| Source { path: p.to_string(), text: t.to_string() })).collect();
    let Some(core) = crate::migrate::packs_of(&sources).into_iter().find(|p| p.name == "core") else { return Ok(()) };
    if !core.steps.contains_key(&1) {
        return Ok(());
    }
    // what can't be moved is said, in the pipeline always: a TODO above it
    let todo = crate::migrate::step::TODO.swap(true, std::sync::atomic::Ordering::Relaxed);
    let mut refused = Vec::new();
    for f in v["files"].as_array_mut().into_iter().flatten() {
        let path = f["path"].as_str().unwrap_or("").to_string();
        let m = Moving::new(base, &[], &core, 1, &path, "");
        let mut done = Done::default();
        m.run(f, &mut done);
        refused.extend(done.refused.iter().map(|r| format!("{}: {}", path, without_line(r))));
        // each TODO is above its place in the file, and said once
        for t in done.todos.iter().map(|t| format!("{}: {}", path, without_line(t))) {
            if !asked.todos.contains(&t) {
                asked.todos.push(t);
            }
        }
        for d in f.get_mut("declarations").and_then(Value::as_array_mut).into_iter().flatten() {
            for u in d.pointer_mut("/pack/uses").and_then(Value::as_array_mut).into_iter().flatten() {
                if u["name"] == "core" && u["version"] == 1 {
                    u["version"] = json!(2);
                }
            }
        }
    }
    crate::migrate::step::TODO.store(todo, std::sync::atomic::Ordering::Relaxed);
    if refused.is_empty() {
        Ok(())
    } else {
        Err(refused)
    }
}

/// A finding without the `line N: ` the engine puts first: the step has no text to count lines in,
/// and the TODO is at the node.
fn without_line(s: &str) -> &str {
    s.strip_prefix("line ").and_then(|r| r.split_once(": ")).map_or(s, |(_, w)| w)
}
