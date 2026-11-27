//! The migration pipeline (migrate/pipeline.rs), through a made-up previous release: 0.3.9, whose
//! resolved IR is a made-up rir 1, the same as rir 2 but for a document's prose, which it calls
//! `words`. Its front end is this xtmd's, writing rir 2 and then putting it back to rir 1; its step, 1
//! to 2, renames `words` to `prose`, and asks and leaves what the tests have it ask and leave. The
//! command's own run (cli/migrate_rir.rs) is given the same front end in place of compiler/previous/.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;
use xtmd::check::{compile_core, compile_on, compile_rir, Checker, Source};
use xtmd::migrate::pipeline::{plan, Checked, Plan};
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

/// 0.3.9's front end: this xtmd's resolved IR, put back to the made-up rir 1, and its Markdown.
struct MadeUp {
    base: Checker,
}

impl Reader for MadeUp {
    fn release(&self) -> String {
        "0.3.9".into()
    }

    fn read(&self, files: &[Source], packs: &[Source]) -> Result<Read, Vec<String>> {
        let own: Vec<Source> = files.iter().filter(|s| s.path.ends_with(".xtmdpack")).cloned().collect();
        let (mut all, mut markdown, mut top) = (Vec::new(), BTreeMap::new(), None);
        for s in files {
            let doc = s.path.ends_with(".xtmd");
            let mut others: Vec<Source> = packs.to_vec();
            others.extend(own.iter().filter(|p| p.path != s.path).cloned());
            let (_, r) = compile_rir(&self.base, &[others.clone()], &[s.clone()], if doc { "doc" } else { "main" });
            let r = r.ok_or_else(|| vec![format!("{}: 0.3.9 doesn't take it", s.path)])?;
            all.extend(r["files"].as_array().unwrap().iter().filter(|x| x["path"] == s.path.as_str()).cloned());
            top.get_or_insert(r);
            if doc {
                markdown.insert(s.path.clone(), markdown_of(&self.base, &others, s)?);
            }
        }
        let mut rir = top.unwrap();
        for file in &mut all {
            if let Some(p) = file.as_object_mut().unwrap().remove("prose") {
                file["words"] = p;
            }
        }
        rir.as_object_mut().unwrap().remove("levels");
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

/// A step that makes an argument in books.xtmd mean another input than its name says: what is printed is
/// the name, so what it compiles back to is not what the step made.
fn retargets(v: &mut Value, a: &mut Asked) -> Result<(), Vec<String>> {
    rename(v, a)?;
    for f in v["files"].as_array_mut().unwrap() {
        if f["path"] == "books.xtmd" {
            *f = serde_json::from_str(&f.to_string().replacen("\"shelf/row_of_books.title\"", "\"shelf/row_of_books.heading\"", 1)).unwrap();
        }
    }
    Ok(())
}

fn tree() -> Vec<Source> {
    vec![
        Source { path: "shelf/shelf.xtmdpack".into(), text: SHELF.into() },
        Source { path: "books.xtmd".into(), text: BOOKS.into() },
        Source { path: "notes.xtmd".into(), text: NOTES.into() },
    ]
}

fn step(run: fn(&mut Value, &mut Asked) -> Result<(), Vec<String>>) -> Step {
    Step { from: 1, schema: schema_1(), run }
}

fn run(run: fn(&mut Value, &mut Asked) -> Result<(), Vec<String>>, answers: &[(&str, &str)], sources: Vec<Source>) -> Plan {
    let base = base();
    let read = MadeUp { base: base.clone() }.read(&sources, &[]).unwrap();
    let check = |printed: &[Source]| -> BTreeMap<String, Result<Checked, Vec<String>>> { xtmd::cli::migrate_rir::compile_all(&base, &[], printed) };
    plan(read, Some(&step(run)), answers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), &check)
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
fn what_a_step_leaves_to_a_person_is_written_and_said() {
    let p = run(rename_and_ask, &[("books.xtmd: first", "todo")], tree());
    assert!(p.ready() && p.for_a_person(), "{:?}", p.refused);
    let clean = run(rename, &[], tree());
    assert!(clean.ready() && !clean.for_a_person());
}

#[test]
fn a_step_is_held_to_the_schema_on_both_sides() {
    let p = run(breaks, &[], tree());
    assert!(!p.ready());
    assert!(p.refused.iter().any(|r| r.contains("doesn't fit rir 2's schema")), "{:?}", p.refused);
    // and without the step from the release before, a resolved IR of it can't be moved
    let base = base();
    let read = MadeUp { base: base.clone() }.read(&tree()[2..], &[]).unwrap();
    let p = plan(read, None, BTreeMap::new(), &|_| BTreeMap::new());
    assert_eq!(p.refused, ["the resolved IR 0.3.9 read: it is rir 1, and this xtmd reads rir 2"]);
}

#[test]
fn what_is_printed_must_compile_back_to_what_the_step_made() {
    let p = run(retargets, &[], tree());
    assert!(!p.ready());
    assert_eq!(p.refused.len(), 1, "{:?}", p.refused);
    assert!(p.refused[0].starts_with("books.xtmd: what was printed compiles to another resolved IR than the step made, at "), "{}", p.refused[0]);
    assert!(p.refused[0].contains("target") && p.refused[0].contains("row_of_books.heading"), "{}", p.refused[0]);
}

#[test]
fn a_document_whose_markdown_changes_is_written_with_a_warning() {
    let p = run(rewords, &[], tree());
    assert!(p.ready(), "{:?}", p.refused);
    assert!(!p.for_a_person(), "a warning leaves nothing for a person");
    assert_eq!(p.warnings.len(), 1, "{:?}", p.warnings);
    assert!(p.warnings[0].starts_with("books.xtmd: its Markdown changed: line "), "{}", p.warnings[0]);
    assert!(p.warnings[0].contains("Write") && p.warnings[0].contains("Wrote"), "{}", p.warnings[0]);
    assert!(text(&p, "books.xtmd").contains("\"Wrote\""));
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
fn the_kept_schema_is_rirs_and_the_one_step_is_from_the_release_before() {
    assert_eq!(rir::NOW, std::fs::read_to_string(repo().join("schemas/rir.schema.json")).unwrap());
    let base = base();
    let mut read = MadeUp { base: base.clone() }.read(&tree(), &[]).unwrap();
    let mut asked = Asked::default();
    let mut older = read.rir.clone();
    assert_eq!(rir::migrate(&mut read.rir, Some(&step(rename)), &mut asked), Ok(1));
    // read by this release: no step, and only checked
    assert_eq!(rir::migrate(&mut read.rir, None, &mut asked), Ok(2));
    // one further back than the step's is never moved, nor a later one
    older["rir"] = 0.into();
    assert!(rir::migrate(&mut older, Some(&step(rename)), &mut asked).unwrap_err()[0].contains("this xtmd's step moves rir 1, its previous release's"));
    read.rir["rir"] = 3.into();
    assert!(rir::migrate(&mut read.rir, None, &mut asked).unwrap_err()[0].contains("this xtmd reads rir 2"));
}

fn xtmd(dir: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_xtmd")).current_dir(dir).args(args).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtmd-pipeline-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (path, text) in files {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

/// The command's run, in `dir`, with 0.3.9's front end for the release before.
fn migrate(dir: &std::path::Path, args: &[&str]) -> std::process::ExitCode {
    let here = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir).unwrap();
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let code = xtmd::cli::migrate_rir::run(&args, Some(Box::new(MadeUp { base: base() })), Some(&step(rename)));
    std::env::set_current_dir(here).unwrap();
    code
}

static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn xtmd_migrate_plans_then_writes_every_file_or_none_and_says_what_is_left() {
    use std::process::ExitCode;
    let _one = ONE_AT_A_TIME.lock().unwrap();
    let dir = scratch("run", &[("xtmd.toml", "xtmd = \"0.3.9\"\npacks = [\"shelf\"]\n"), ("shelf/shelf.xtmdpack", SHELF), ("books.xtmd", BOOKS), ("notes.xtmd", NOTES)]);
    // nothing is written without --yes where nothing can be asked
    assert_eq!(migrate(&dir, &["."]), ExitCode::from(1));
    assert_eq!(std::fs::read_to_string(dir.join("books.xtmd")).unwrap(), BOOKS);
    assert_eq!(migrate(&dir, &[".", "--dry-run"]), ExitCode::from(0));
    assert_eq!(migrate(&dir, &[".", "--yes"]), ExitCode::from(0));
    let books = std::fs::read_to_string(dir.join("books.xtmd")).unwrap();
    assert!(books.contains("// what is on the shelf\nlet first: text = \"Read\";"), "{}", books);
    assert!(std::fs::read_to_string(dir.join("xtmd.toml")).unwrap().starts_with(&format!("xtmd = \"{}\"", env!("CARGO_PKG_VERSION"))));

    // what the step leaves for a person is written, and the run says so with its exit code
    fn leaves_a_todo(v: &mut Value, asked: &mut Asked) -> Result<(), Vec<String>> {
        rename(v, asked)?;
        for f in v["files"].as_array_mut().unwrap() {
            let path = f["path"].as_str().unwrap().to_string();
            if let Some(d) = f["declarations"].as_array_mut().and_then(|ds| ds.iter_mut().find(|d| d.get("let").is_some())) {
                asked.todo(d, &path, "look at first");
            }
        }
        Ok(())
    }
    let dir = scratch("todo", &[("xtmd.toml", "xtmd = \"0.3.9\"\npacks = [\"shelf\"]\n"), ("shelf/shelf.xtmdpack", SHELF), ("books.xtmd", BOOKS)]);
    let args: Vec<String> = [".", "--yes"].iter().map(|a| a.to_string()).collect();
    let here = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    let code = xtmd::cli::migrate_rir::run(&args, Some(Box::new(MadeUp { base: base() })), Some(&step(leaves_a_todo)));
    std::env::set_current_dir(here).unwrap();
    assert_eq!(code, ExitCode::from(3));
    assert!(std::fs::read_to_string(dir.join("books.xtmd")).unwrap().contains("// TODO(xtmd migrate): look at first\nlet first"));

    // a file the release before doesn't take: nothing is written, the others neither
    let dir = scratch("none", &[("xtmd.toml", "xtmd = \"0.3.9\"\n"), ("a.xtmd", "let x: text   = \"a\";\n@@@\n\\{x}\n"), ("b.xtmd", "@@@\n\\{nothing_here}\n")]);
    assert_eq!(migrate(&dir, &[".", "--yes"]), ExitCode::from(1));
    assert_eq!(std::fs::read_to_string(dir.join("a.xtmd")).unwrap(), "let x: text   = \"a\";\n@@@\n\\{x}\n");
    // and an xtmd built without the front end of the release before says so
    let args = vec![".".to_string(), "--yes".to_string()];
    assert_eq!(xtmd::cli::migrate_rir::run(&args, None, Some(&step(rename))), ExitCode::from(1));
}

#[test]
fn a_tree_of_any_release_but_the_one_before_or_this_one_is_refused_by_name() {
    let previous = xtmd::migrate::readers::PREVIOUS;
    for (release, code, says) in [
        ("0.3.1", 1, format!("the tree is written for xtmd 0.3.1, and this xtmd ({}) moves only a tree written for {}, the release before it", env!("CARGO_PKG_VERSION"), previous)),
        ("0.3.0", 1, "tools/migrate/chain.py".to_string()),
        ("9.0.0", 2, "which is newer than this xtmd".to_string()),
        ("three", 2, "isn't a release of xtmd".to_string()),
    ] {
        let dir = scratch(&format!("release-{}", release), &[("xtmd.toml", &format!("xtmd = \"{}\"\n", release)), ("notes.xtmd", NOTES)]);
        let (got, _, err) = xtmd(&dir, &["migrate", ".", "--yes"]);
        assert_eq!(got, code, "{}: {}", release, err);
        assert!(err.contains(&says), "{}: {}", release, err);
        assert_eq!(std::fs::read_to_string(dir.join("notes.xtmd")).unwrap(), NOTES);
    }
}
