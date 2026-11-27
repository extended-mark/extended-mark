//! The step from 0.3.2, its format part (src/migrate/rir/v1.rs): every file of the 0.3.2 corpus that
//! 0.4.0's language takes as it is, read by 0.3.2's front end and moved to rir 2's shape, is the
//! resolved IR 0.4.0 itself gives the same text, every comment in its place. Spans and expressions'
//! types apart (pipeline::written), as the pipeline compares.

use std::path::PathBuf;

use serde_json::Value;
use xtmd::check::{compile_core, compile_rir, Checker, Source};
use xtmd::migrate::pipeline::{first_difference, written};
use xtmd::migrate::readers::previous::Previous;
use xtmd::migrate::readers::Reader;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn base() -> Checker {
    let mut core = Vec::new();
    for e in std::fs::read_dir(repo().join("core")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map_or(false, |x| x == "xtmdpack") {
            core.push(Source { path: p.display().to_string(), text: std::fs::read_to_string(&p).unwrap() });
        }
    }
    core.sort_by(|a, b| a.path.cmp(&b.path));
    compile_core(&core)
}

fn sources(root: &std::path::Path, paths: &Value) -> Vec<Source> {
    paths.as_array().unwrap().iter().map(|p| p.as_str().unwrap()).map(|p| Source { path: p.to_string(), text: std::fs::read_to_string(root.join(p)).unwrap() }).collect()
}

/// What 0.4.0 makes of the unit's files as they are, each file's resolved IR; none when it refuses one.
fn as_0_4_0(base: &Checker, files: &[Source], packs: &[Source]) -> Option<Vec<Value>> {
    let own: Vec<Source> = files.iter().filter(|f| f.path.ends_with(".xtmdpack")).cloned().collect();
    let mut out = Vec::new();
    for f in files {
        let mut world = packs.to_vec();
        world.extend(own.iter().filter(|p| p.path != f.path).cloned());
        let (c, r) = compile_rir(base, &[world], &[f.clone()], if f.path.ends_with(".xtmd") { "doc" } else { "main" });
        if c.diagnostics.iter().any(|d| d.diagnostic.is_error()) {
            return None;
        }
        out.push(r?["files"].as_array()?.iter().find(|x| x["path"] == f.path.as_str())?.clone());
    }
    Some(out)
}

#[test]
fn a_0_3_2_file_with_no_change_of_language_is_moved_to_0_4_0s_shape_exactly() {
    // 0.3.2's checker goes deep, as the command's main thread lets it
    std::thread::Builder::new().stack_size(64 << 20).spawn(a_0_3_2_file_with_no_change_of_language_is_moved_to_0_4_0s_shape_exactly_).unwrap().join().unwrap();
}

fn a_0_3_2_file_with_no_change_of_language_is_moved_to_0_4_0s_shape_exactly_() {
    let units: Value = serde_json::from_str(&std::fs::read_to_string(repo().join("checks/migrate-0.3/units.json")).unwrap()).unwrap();
    let (base, previous) = (base(), Previous::new());
    let (mut same, mut wrong) = (0, Vec::new());
    for u in units["units"].as_array().unwrap().iter().filter(|u| u["valid"].as_bool().unwrap()) {
        let id = u["id"].as_str().unwrap();
        let root = match id.strip_prefix("probes/") {
            Some(p) => repo().join("checks/migrate-0.3/probes").join(p),
            None => repo().join("checks/migrate-0.3/v0.3.2"),
        };
        let files = sources(&root, &u["files"]);
        let packs: Vec<Source> = sources(&root, &u["packs"]).into_iter().filter(|p| !files.iter().any(|f| f.path == p.path)).collect();
        let Some(now) = as_0_4_0(&base, &files, &packs) else { continue };
        let mut v = previous.read(&files, &packs).unwrap().rir;
        let mut asked = xtmd::migrate::rir::Asked::default();
        if let Err(e) = xtmd::migrate::rir::v1::format(&mut v, &mut asked) {
            wrong.push(format!("{}: {}", id, e.join("; ")));
            continue;
        }
        if !asked.todos.is_empty() {
            wrong.push(format!("{}: left for a person: {}", id, asked.todos.join("; ")));
        }
        for file in &now {
            let path = file["path"].as_str().unwrap();
            let moved = v["files"].as_array().unwrap().iter().find(|f| f["path"] == path).unwrap();
            match first_difference(&written(moved), &written(file), "") {
                None => same += 1,
                Some(d) => wrong.push(format!("{}: {}: {}", id, path, d)),
            }
        }
    }
    eprintln!("the format part: {} files of 0.3.2 the same as 0.4.0 gives them, {} not", same, wrong.len());
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert!(same >= 80, "only {} files", same);
}

const LEFT_OUT: &str = "---\ntitle: Notes\n---\n// the state\n$state {\n    //@about how many\n    n: int = 2; // two\n}\n\n//@about a widget\nwidget w(t: text) {\n    view {\n        @label(value = t);\n        // at the end of the view\n    }\n    markdown {\n        \\{t}\n    }\n}\n\nfn none() {\n    // alone in an empty body\n}\n\n// at the end\n@@@\n@w(t = \"a\");\n";

fn in_a_thread(f: fn()) {
    std::thread::Builder::new().stack_size(64 << 20).spawn(f).unwrap().join().unwrap();
}

#[test]
fn what_rir_1_had_no_place_for_is_put_where_0_4_0_puts_it_and_each_side_fits_its_schema() {
    in_a_thread(|| {
        let f = Source { path: "notes.xtmd".into(), text: LEFT_OUT.into() };
        let mut v = Previous::new().read(&[f.clone()], &[]).unwrap().rir;
        let before: Value = serde_json::from_str(xtmd::migrate::rir::v1::SCHEMA).unwrap();
        assert_eq!(xtmd::migrate::ir::schema::check(&v, &before), None, "what 0.3.2's reader gives fits rir 1 and left_out");
        let mut asked = xtmd::migrate::rir::Asked::default();
        xtmd::migrate::rir::v1::format(&mut v, &mut asked).unwrap();
        assert!(asked.todos.is_empty(), "{:?}", asked.todos);
        let after: Value = serde_json::from_str(xtmd::migrate::rir::NOW).unwrap();
        v["rir"] = Value::from(xtmd::migrate::rir::VERSION);
        assert_eq!(xtmd::migrate::ir::schema::check(&v, &after), None, "the format part gives rir 2");
        let now = as_0_4_0(&base(), &[f], &[]).unwrap();
        assert_eq!(first_difference(&written(&v["files"][0]), &written(&now[0]), ""), None);
        let text = v.to_string();
        for c in ["how many", "// at the end of the view", "// alone in an empty body", "// two", "// at the end", "title: Notes"] {
            assert!(text.contains(c), "{:?} is lost", c);
        }
    });
}

#[test]
fn comments_whose_place_this_release_cant_give_are_kept_at_the_end_with_a_todo() {
    in_a_thread(|| {
        let f = Source { path: "notes.xtmd".into(), text: LEFT_OUT.into() };
        let mut v = Previous::new().read(&[f], &[]).unwrap().rir;
        // as when 0.4.0's parser can't read the file at all
        v["files"][0]["left_out"]["now"] = Value::Null;
        let mut asked = xtmd::migrate::rir::Asked::default();
        xtmd::migrate::rir::v1::format(&mut v, &mut asked).unwrap();
        assert_eq!(asked.todos.len(), 1, "{:?}", asked.todos);
        let end = v["files"][0]["comment_items"].as_array().unwrap();
        let text = serde_json::to_string(end).unwrap();
        assert!(text.contains("// alone in an empty body") && text.contains("// at the end of the view"), "{}", text);
        assert!(end.last().unwrap().to_string().contains("TODO(xtmd migrate)"));
        assert!(text.contains("//@about how many"), "a member's doc comment, which rir 1 has no place for, is kept");
        assert!(!text.contains("//@about a widget"), "a declaration's doc comment stays its doc: {}", text);
    });
}
