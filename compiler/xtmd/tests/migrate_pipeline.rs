//! The migration pipeline (migrate/pipeline.rs), through a made-up release: 0.3.9, whose resolved IR
//! is a made-up rir 1, the same as rir 2 but for a document's prose, which it calls `words`. Its reader
//! is this xtmd, writing rir 2 and then putting it back to rir 1; its one step, 1 to 2, renames
//! `words` to `prose`, and asks and leaves what the tests have it ask and leave.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;
use xtmd::check::{compile_core, compile_on, compile_rir, Checker, Source};
use xtmd::migrate::pipeline::{plan, Plan};
use xtmd::migrate::readers::{Read, Reader};
use xtmd::migrate::rir::{self, Asked, Step};

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

const SHELF: &str = "package shelf;\n\npack shelf@1 {\n    provides row_of_books;\n}\n\nwidget row_of_books(title: text) {\n    view {\n        @label(value = title);\n    }\n    markdown {\n        - \\{title}\n    }\n}\n";
// not in fmt's layout, with comments: what comes back is in fmt's, its comments kept
const BOOKS: &str = "import shelf;\n\n// what is on the shelf\nlet first: text   =  \"Read\";\n@@@\n# Books\n\n@row_of_books(title = first);\n@row_of_books(title = \"Write\");\n";
const NOTES: &str = "// a page of its own\n$state {\n    n: int = 2;\n}\n@@@\nNotes: \\{$state.n}.\n";

fn markdown_of(base: &Checker, packs: &[Source], doc: &Source) -> Result<String, Vec<String>> {
    let c = compile_on(base, &[packs.to_vec()], &[doc.clone()], "doc");
    let errors: Vec<String> = c.diagnostics.iter().filter(|d| d.diagnostic.is_error()).map(|d| d.diagnostic.message.to_string()).collect();
    let Some(mut m) = c.module.filter(|_| errors.is_empty()) else { return Err(errors) };
    xtmd::lower::lower(&mut m);
    xtmd::markdown::page(&m).map(|b| xtmd::markdown::write_page(m.front_matter.as_deref(), &b)).map_err(|e| vec![e.0])
}

/// 0.3.9's reader: this xtmd's resolved IR, put back to the made-up rir 1, and its Markdown.
struct MadeUp<'a> {
    base: &'a Checker,
    sources: Vec<Source>,
}

impl Reader for MadeUp<'_> {
    fn read(&self, files: &[String], _: &[String]) -> Result<Read, Vec<String>> {
        let packs: Vec<Source> = self.sources.iter().filter(|s| s.path.ends_with(".xtmdpack")).cloned().collect();
        let (mut all, mut markdown, mut top) = (Vec::new(), BTreeMap::new(), None);
        for f in files {
            let s = self.sources.iter().find(|s| &s.path == f).unwrap();
            let doc = f.ends_with(".xtmd");
            let others: Vec<Source> = packs.iter().filter(|p| &p.path != f).cloned().collect();
            let (_, r) = compile_rir(self.base, &[others.clone()], &[s.clone()], if doc { "doc" } else { "main" });
            let r = r.ok_or_else(|| vec![format!("{}: no resolved IR", f)])?;
            all.extend(r["files"].as_array().unwrap().iter().filter(|x| x["path"] == f.as_str()).cloned());
            top.get_or_insert(r);
            if doc {
                markdown.insert(f.clone(), markdown_of(self.base, &others, s)?);
            }
        }
        let mut rir = top.unwrap();
        for file in &mut all {
            if let Some(p) = file.as_object_mut().unwrap().remove("prose") {
                file["words"] = p;
            }
        }
        rir["files"] = Value::Array(all);
        rir["rir"] = 1.into();
        rir["xtmd"] = "0.3.9".into();
        Ok(Read { release: "0.3.9".into(), rir, markdown })
    }
}

/// The made-up rir 1's schema: rir 2's, with `words` for `prose`.
fn schema_1() -> &'static str {
    let mut s: Value = serde_json::from_str(rir::NOW).unwrap();
    s["$defs"]["root"]["properties"]["rir"]["const"] = 1.into();
    let props = s["$defs"]["file"]["properties"].as_object_mut().unwrap();
    let prose = props.remove("prose").unwrap();
    props.insert("words".into(), prose);
    Box::leak(s.to_string().into_boxed_str())
}

/// The step, 1 to 2: `words` is `prose` again.
fn rename(v: &mut Value, _: &mut Asked) -> Result<(), Vec<String>> {
    for f in v["files"].as_array_mut().unwrap() {
        if let Some(w) = f.as_object_mut().unwrap().remove("words") {
            f["prose"] = w;
        }
    }
    Ok(())
}

/// The step, and a question about the first `let` of each document: keep it, or leave a TODO on it.
fn rename_and_ask(v: &mut Value, asked: &mut Asked) -> Result<(), Vec<String>> {
    rename(v, asked)?;
    for f in v["files"].as_array_mut().unwrap() {
        let path = f["path"].as_str().unwrap().to_string();
        let Some(d) = f["declarations"].as_array_mut().and_then(|ds| ds.iter_mut().find(|d| d.get("let").is_some())) else { continue };
        match asked.ask(&format!("{}: first", path), "keep `first` as it is?", &["keep", "todo"]).as_deref() {
            Some("todo") => asked.todo(d, &path, "first is a made-up release's, look at it"),
            _ => {}
        }
    }
    Ok(())
}

/// A step that writes what rir 2 doesn't take.
fn breaks(v: &mut Value, a: &mut Asked) -> Result<(), Vec<String>> {
    rename(v, a)?;
    v["files"][0]["kind"] = "a poem".into();
    Ok(())
}

/// A step that changes what a document says.
fn rewords(v: &mut Value, a: &mut Asked) -> Result<(), Vec<String>> {
    rename(v, a)?;
    let text = v.to_string().replace("\"Write\"", "\"Wrote\"");
    *v = serde_json::from_str(&text).unwrap();
    Ok(())
}

fn tree() -> Vec<Source> {
    vec![
        Source { path: "shelf/shelf.xtmdpack".into(), text: SHELF.into() },
        Source { path: "books.xtmd".into(), text: BOOKS.into() },
        Source { path: "notes.xtmd".into(), text: NOTES.into() },
    ]
}

fn run(run: fn(&mut Value, &mut Asked) -> Result<(), Vec<String>>, answers: &[(&str, &str)], sources: Vec<Source>) -> Plan {
    let base = base();
    let files: Vec<String> = sources.iter().map(|s| s.path.clone()).collect();
    let packs: Vec<Source> = sources.iter().filter(|s| s.path.ends_with(".xtmdpack")).cloned().collect();
    let read = MadeUp { base: &base, sources }.read(&files, &[]).unwrap();
    let steps = [Step { from: 1, schema: schema_1(), run }];
    let check = |printed: &[Source]| -> BTreeMap<String, Result<String, Vec<String>>> {
        let new_packs: Vec<Source> = printed.iter().filter(|s| s.path.ends_with(".xtmdpack")).cloned().collect();
        let world = if new_packs.is_empty() { packs.clone() } else { new_packs };
        printed.iter().filter(|s| s.path.ends_with(".xtmd")).map(|d| (d.path.clone(), markdown_of(&base, &world, d))).collect()
    };
    plan(read, &steps, answers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), &check)
}

fn text<'a>(p: &'a Plan, path: &str) -> &'a str {
    &p.files.iter().find(|f| f.path == path).unwrap().text
}

#[test]
fn a_tree_of_a_made_up_release_comes_back_in_fmts_layout_its_comments_kept() {
    let p = run(rename, &[], tree());
    assert!(p.ready(), "refused: {:?}, open: {:?}", p.refused, p.open);
    assert_eq!((p.from_release.as_str(), p.from_rir), ("0.3.9", 1));
    assert_eq!(p.files.len(), 3);
    let books = text(&p, "books.xtmd");
    assert!(books.contains("// what is on the shelf\nlet first: text = \"Read\";"), "{}", books);
    assert!(text(&p, "notes.xtmd").starts_with("// a page of its own\n"), "{}", text(&p, "notes.xtmd"));
    assert_eq!(text(&p, "shelf/shelf.xtmdpack"), SHELF);
}

#[test]
fn a_question_left_open_writes_nothing_and_its_answer_is_what_the_step_does() {
    let p = run(rename_and_ask, &[], tree());
    assert!(!p.ready());
    assert_eq!(p.open.iter().map(|q| q.id.as_str()).collect::<Vec<_>>(), ["books.xtmd: first"]);
    assert_eq!(p.open[0].options, ["keep", "todo"]);
    let kept = run(rename_and_ask, &[("books.xtmd: first", "keep")], tree());
    assert!(kept.ready() && kept.todos.is_empty(), "{:?}", kept.refused);
    // an answer the question doesn't take leaves it open
    assert!(!run(rename_and_ask, &[("books.xtmd: first", "maybe")], tree()).ready());
}

#[test]
fn what_a_step_leaves_to_a_person_is_a_todo_above_it() {
    let p = run(rename_and_ask, &[("books.xtmd: first", "todo")], tree());
    assert!(p.ready(), "{:?}", p.refused);
    assert_eq!(p.todos, ["books.xtmd: first is a made-up release's, look at it"]);
    assert!(text(&p, "books.xtmd").contains("// what is on the shelf\n// TODO(xtmd migrate): first is a made-up release's, look at it\nlet first"), "{}", text(&p, "books.xtmd"));
}

#[test]
fn a_step_is_held_to_the_schema_on_both_sides() {
    let p = run(breaks, &[], tree());
    assert!(!p.ready());
    assert!(p.refused.iter().any(|r| r.contains("doesn't fit rir 2's schema")), "{:?}", p.refused);
    // and no step from a version: nothing can be done
    let base = base();
    let files = vec!["notes.xtmd".to_string()];
    let read = MadeUp { base: &base, sources: tree() }.read(&files, &[]).unwrap();
    let p = plan(read, &[], BTreeMap::new(), &|_| BTreeMap::new());
    assert_eq!(p.refused, ["the resolved IR 0.3.9 read: it is rir 1, and no step leads from it to rir 2"]);
}

#[test]
fn a_document_whose_markdown_changes_is_refused_and_so_is_every_file() {
    let p = run(rewords, &[], tree());
    assert!(!p.ready());
    assert_eq!(p.refused.len(), 1, "{:?}", p.refused);
    assert!(p.refused[0].starts_with("books.xtmd: its Markdown changed: line "), "{}", p.refused[0]);
    assert!(p.refused[0].contains("Write") && p.refused[0].contains("Wrote"), "{}", p.refused[0]);
}

#[test]
fn a_document_that_doesnt_compile_after_is_refused() {
    // the step keeps the pack's old name in the document, and the pack is printed under a new one
    fn renames_the_pack(v: &mut Value, a: &mut Asked) -> Result<(), Vec<String>> {
        rename(v, a)?;
        let text = v.to_string().replace("row_of_books", "row_of_tomes");
        *v = serde_json::from_str(&text).unwrap();
        for f in v["files"].as_array_mut().unwrap() {
            if f["path"] == "books.xtmd" {
                *f = serde_json::from_str(&f.to_string().replace("row_of_tomes", "row_of_books")).unwrap();
            }
        }
        Ok(())
    }
    let p = run(renames_the_pack, &[], tree());
    assert!(!p.ready());
    assert!(p.refused.iter().any(|r| r.starts_with("books.xtmd: what was printed doesn't compile")), "{:?}", p.refused);
}

#[test]
fn the_kept_schema_is_rirs_and_every_resolved_ir_of_the_tree_fits_it() {
    assert_eq!(rir::NOW, std::fs::read_to_string(repo().join("schemas/rir.schema.json")).unwrap());
    let base = base();
    let files: Vec<String> = tree().iter().map(|s| s.path.clone()).collect();
    let mut read = MadeUp { base: &base, sources: tree() }.read(&files, &[]).unwrap();
    let mut asked = Asked::default();
    assert_eq!(rir::migrate(&mut read.rir, &[Step { from: 1, schema: schema_1(), run: rename }], &mut asked), Ok(1));
    // a resolved IR of this version needs no step
    assert_eq!(rir::migrate(&mut read.rir, &[], &mut asked), Ok(2));
    read.rir["rir"] = 3.into();
    assert!(rir::migrate(&mut read.rir, &[], &mut asked).unwrap_err()[0].contains("use a later xtmd"));
}

fn xtmd(args: &[&str]) -> (i32, String, String) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_xtmd")).args(args).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtmd-pipeline-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, text) in files {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

#[test]
fn xtmd_migrate_reader_plans_then_writes_every_file_or_none() {
    let me = env!("CARGO_BIN_EXE_xtmd");
    let dir = scratch("cli", &[("shelf/shelf.xtmdpack", SHELF), ("books.xtmd", BOOKS), ("notes.xtmd", NOTES)]);
    let d = dir.to_str().unwrap();
    let pack = dir.join("shelf").display().to_string();
    // a release's own xtmd reads its tree: what changes is the layout, and nothing is written yet
    let (code, _, err) = xtmd(&["migrate", d, "--pack", &pack, "--reader", me, "--dry-run"]);
    assert_eq!(code, 0, "{}", err);
    assert!(err.contains("read by 0.4.0's xtmd as rir 2") && err.contains("books.xtmd: changes"), "{}", err);
    assert_eq!(std::fs::read_to_string(dir.join("books.xtmd")).unwrap(), BOOKS);
    let (code, out, _) = xtmd(&["migrate", d, "--pack", &pack, "--reader", me, "--format", "json", "--dry-run"]);
    assert_eq!(code, 0);
    let last: Value = serde_json::from_str(out.lines().last().unwrap()).unwrap();
    assert_eq!((last["type"].as_str(), last["ready"].as_bool(), last["from"].as_str()), (Some("summary"), Some(true), Some("0.4.0")));
    // without a terminal it writes only with --yes
    let (code, _, err) = xtmd(&["migrate", d, "--pack", &pack, "--reader", me]);
    assert_eq!(code, 1, "{}", err);
    assert!(err.contains("nothing written: --yes writes it"), "{}", err);
    let (code, _, err) = xtmd(&["migrate", d, "--pack", &pack, "--reader", me, "--yes"]);
    assert_eq!(code, 0, "{}", err);
    let books = std::fs::read_to_string(dir.join("books.xtmd")).unwrap();
    assert!(books.contains("// what is on the shelf\nlet first: text = \"Read\";"), "{}", books);
    // once moved, nothing more to do
    let (code, _, err) = xtmd(&["migrate", d, "--pack", &pack, "--reader", me, "--check"]);
    assert_eq!(code, 0, "{}", err);
    assert!(err.contains("nothing to change"), "{}", err);

    // one file the old release can't read: nothing is written, the others neither
    let dir = scratch("cli-none", &[("a.xtmd", "let x: text   = \"a\";\n@@@\n\\{x}\n"), ("b.xtmd", "@@@\n\\{nothing_here}\n")]);
    let (code, _, err) = xtmd(&["migrate", dir.to_str().unwrap(), "--reader", me, "--yes"]);
    assert_eq!(code, 1, "{}", err);
    assert!(err.contains("nothing written"), "{}", err);
    assert_eq!(std::fs::read_to_string(dir.join("a.xtmd")).unwrap(), "let x: text   = \"a\";\n@@@\n\\{x}\n");
    // and a reader that isn't one says so
    let (code, _, err) = xtmd(&["migrate", dir.to_str().unwrap(), "--reader", "/no/such/xtmd", "--yes"]);
    assert_eq!(code, 1);
    assert!(err.contains("/no/such/xtmd can't be run"), "{}", err);
}
