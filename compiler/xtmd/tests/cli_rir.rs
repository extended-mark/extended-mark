//! The resolved IR: every case in checks/cases that compiles writes one that fits
//! schemas/rir.schema.json, with every span inside the file it names.

use jsonschema::{Registry, Validator};
use serde_json::Value;
use std::path::{Path, PathBuf};

const IR_ID: &str = "https://raw.githubusercontent.com/extended-mark/extended-mark/main/schemas/ir.schema.json";

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap_or_else(|e| panic!("{}: {}", path.display(), e))
}

fn validator() -> Validator {
    let ir = json(&repo().join("schemas/ir.schema.json"));
    let schema = json(&repo().join("schemas/rir.schema.json"));
    let registry = Registry::new().add(IR_ID, ir).unwrap().prepare().unwrap();
    jsonschema::options().with_registry(&registry).build(&schema).unwrap()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().map_or(false, |e| e == "json") {
            out.push(p);
        }
    }
}

fn sources(dir: &Path, names: &[&str]) -> Vec<xtmd::check::Source> {
    fn add(p: &Path, out: &mut Vec<xtmd::check::Source>) {
        if p.is_dir() {
            let mut files: Vec<PathBuf> = std::fs::read_dir(p).unwrap().map(|e| e.unwrap().path()).collect();
            files.sort();
            for f in files {
                add(&f, out);
            }
        } else if p.extension().map_or(false, |x| x == "xtmdpack") {
            out.push(xtmd::check::Source { path: p.to_string_lossy().into_owned(), text: std::fs::read_to_string(p).unwrap() });
        }
    }
    let mut out = Vec::new();
    for n in names {
        add(&dir.join(n), &mut out);
    }
    out
}

fn spans(v: &Value, out: &mut Vec<(u64, u64)>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if (k == "span" || k.ends_with("_span") || k == "front_matter") && x.is_array() {
                    out.push((x[0].as_u64().unwrap(), x[1].as_u64().unwrap()));
                } else {
                    spans(x, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| spans(x, out)),
        _ => {}
    }
}

#[test]
fn every_compiling_case_writes_a_resolved_ir_that_fits_its_schema() {
    // a case runs Extended Mark to the depth limit at compile time, deeper than a test thread's stack
    std::thread::Builder::new().stack_size(64 << 20).spawn(write_every_case).unwrap().join().unwrap();
}

fn write_every_case() {
    let v = validator();
    let core = sources(&repo().join("core"), &["."]);
    let base = xtmd::check::compile_core(&core);
    let mut cases = Vec::new();
    walk(&repo().join("checks/cases"), &mut cases);
    // the runtime's own pages too, which all compile
    walk(&repo().join("runtime/typescript/test/cases"), &mut cases);
    let (mut written, mut failures) = (0, Vec::new());
    for case_path in cases {
        let case = json(&case_path);
        let Some(source) = case["source"].as_str() else { continue };
        let dir = case_path.parent().unwrap();
        let names: Vec<&str> = case["packs"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let packs = sources(dir, &names);
        let path = dir.join(source);
        let name = path.to_string_lossy().into_owned();
        let main = xtmd::check::Source { path: name.clone(), text: std::fs::read_to_string(&path).unwrap() };
        let module = if name.ends_with(".xtmd") { "doc" } else { "t" };
        let (_, rir) = xtmd::check::compile_rir(&base, &[packs], &[main], module);
        let Some(rir) = rir else { continue };
        let errs: Vec<String> = v.iter_errors(&rir).map(|e| format!("{} at {}", e, e.instance_path())).take(3).collect();
        if !errs.is_empty() {
            failures.push(format!("{}: {:#?}", case_path.display(), errs));
            continue;
        }
        for file in rir["files"].as_array().unwrap() {
            let len = std::fs::read_to_string(file["path"].as_str().unwrap()).unwrap().len() as u64;
            let mut all = Vec::new();
            spans(file, &mut all);
            if let Some((s, e)) = all.iter().find(|(s, e)| s > e || *e > len) {
                failures.push(format!("{}: a span [{}, {}] outside {} ({} bytes)", case_path.display(), s, e, file["path"], len));
            }
        }
        written += 1;
    }
    eprintln!("resolved IR: {} cases written", written);
    assert!(failures.is_empty(), "{} of {} fail:\n{}", failures.len(), written + failures.len(), failures.join("\n"));
    assert!(written >= 50, "only {} cases compiled", written);
}

#[test]
fn the_example_is_what_xtmd_build_rir_writes() {
    let (path, want) = ("schemas/examples/counter.xtmd", repo().join("schemas/examples/counter.rir.json"));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_xtmd")).current_dir(repo()).args(["compile", path, "--rir"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let got: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(got, json(&want), "schemas/examples/counter.rir.json is not what `xtmd compile --rir` writes: run it with --out");
}

#[test]
fn the_ir_example_is_what_xtmd_build_writes() {
    let (path, want) = ("schemas/examples/counter.xtmd", repo().join("schemas/examples/counter.ir.json"));
    let build = |pretty: &[&str]| std::process::Command::new(env!("CARGO_BIN_EXE_xtmd")).current_dir(repo()).args(["compile", path]).args(pretty).output().unwrap().stdout;
    let got = build(&["--pretty"]);
    assert_eq!(serde_json::from_slice::<Value>(&got).unwrap(), json(&want), "schemas/examples/counter.ir.json is not what `xtmd compile --pretty` writes: run it with --out");
    // as small as it goes, without --pretty
    let small = build(&[]);
    assert_eq!(serde_json::from_slice::<Value>(&small).unwrap(), json(&want));
    assert_eq!(small.iter().filter(|b| **b == b'\n').count(), 1);
}

/// Every comment in a parse tree, as written.
fn comments_in(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            if m.contains_key("text") && m.contains_key("gap") && m.contains_key("span") {
                out.push(m["text"].as_str().unwrap().to_string());
                return;
            }
            m.values().for_each(|x| comments_in(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| comments_in(x, out)),
        _ => {}
    }
}

/// Every comment a resolved IR's file holds: its comments objects, comment items, doc lines.
fn comments_kept(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                match (k.as_str(), x) {
                    ("leading" | "free", Value::Array(a)) | ("comment", Value::Array(a)) => out.extend(a.iter().filter_map(|s| s.as_str().map(str::to_string))),
                    ("trailing", Value::String(s)) => out.push(s.clone()),
                    ("block_start" | "block_end", Value::Array(a)) => out.extend(a.iter().map(|b| b["text"].as_str().unwrap().to_string())),
                    ("doc", Value::Object(d)) => {
                        for (dk, dv) in d {
                            match dv {
                                Value::String(s) => out.extend(s.split('\n').map(|l| format!("//@{} {}", dk, l))),
                                Value::Object(ps) => {
                                    for (p, s) in ps {
                                        out.extend(s.as_str().unwrap().split('\n').map(|l| format!("//@param {} {}", p, l)));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => comments_kept(x, out),
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| comments_kept(x, out)),
        _ => {}
    }
}

/// The comments a file's source has that its resolved IR doesn't, doc lines compared by what they say.
fn missing_comments(path: &str, file: &Value) -> Vec<String> {
    let src = std::fs::read_to_string(path).unwrap();
    let entry = xtmd::syntax::entry_for(path).unwrap();
    let parsed = xtmd::syntax::parse(&src, entry);
    let mut want = Vec::new();
    comments_in(&serde_json::to_value(parsed.file.as_ref().unwrap()).unwrap(), &mut want);
    want.extend(parsed.orphans.iter().map(|c| c.text.clone()));
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut got: Vec<String> = Vec::new();
    comments_kept(file, &mut got);
    let mut got: Vec<String> = got.iter().map(|s| norm(s)).collect();
    let mut missing = Vec::new();
    for w in want {
        match got.iter().position(|g| *g == norm(&w)) {
            Some(k) => {
                got.swap_remove(k);
            }
            None => missing.push(w),
        }
    }
    missing
}

#[test]
fn every_comment_of_every_file_is_in_its_resolved_ir() {
    std::thread::Builder::new().stack_size(64 << 20).spawn(keep_every_comment).unwrap().join().unwrap();
}

fn keep_every_comment() {
    let core = sources(&repo().join("core"), &["."]);
    let base = xtmd::check::compile_core_logged(&core);
    let mut failures = Vec::new();
    let mut files = 0;
    let mut comments = 0;
    let mut check = |rir: &Value, failures: &mut Vec<String>| {
        for file in rir["files"].as_array().unwrap() {
            let path = file["path"].as_str().unwrap();
            let missing = missing_comments(path, file);
            let mut all = Vec::new();
            comments_kept(file, &mut all);
            comments += all.len();
            files += 1;
            if !missing.is_empty() {
                failures.push(format!("{}: {:?}", path, missing));
            }
        }
    };
    // core's own files
    let (_, rir) = xtmd::check::compile_rir_with(&base, &[], &[], "t", xtmd::check::RirOptions { core: true, partial: true });
    check(&rir.unwrap(), &mut failures);
    let mut cases = Vec::new();
    walk(&repo().join("checks/cases"), &mut cases);
    walk(&repo().join("runtime/typescript/test/cases"), &mut cases);
    for case_path in cases {
        let case = json(&case_path);
        let Some(source) = case["source"].as_str() else { continue };
        let dir = case_path.parent().unwrap();
        let names: Vec<&str> = case["packs"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let packs = sources(dir, &names);
        let path = dir.join(source);
        let name = path.to_string_lossy().into_owned();
        let main = xtmd::check::Source { path: name.clone(), text: std::fs::read_to_string(&path).unwrap() };
        let module = if name.ends_with(".xtmd") { "doc" } else { "t" };
        let (_, rir) = xtmd::check::compile_rir(&base, &[packs], &[main], module);
        if let Some(rir) = rir {
            check(&rir, &mut failures);
        }
    }
    // the examples and apps: each folder's documents, with the folder's packs
    let mut dirs = Vec::new();
    fn folders(d: &Path, out: &mut Vec<PathBuf>) {
        let mut es: Vec<_> = std::fs::read_dir(d).unwrap().map(|e| e.unwrap().path()).collect();
        es.sort();
        if es.iter().any(|p| p.extension().map_or(false, |x| x == "xtmd" || x == "xtmdpack")) {
            out.push(d.to_path_buf());
        }
        for p in es {
            if p.is_dir() && p.file_name().map_or(true, |n| n != "node_modules") {
                folders(&p, out);
            }
        }
    }
    folders(&repo().join("examples"), &mut dirs);
    folders(&repo().join("apps"), &mut dirs);
    for d in dirs {
        let packs = sources(&d, &["."]);
        let mut docs: Vec<PathBuf> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().map_or(false, |x| x == "xtmd")).collect();
        docs.sort();
        if docs.is_empty() {
            let (_, rir) = xtmd::check::compile_rir(&base, &[], &packs, "t");
            if let Some(rir) = rir {
                check(&rir, &mut failures);
            }
        }
        for doc in docs {
            let main = xtmd::check::Source { path: doc.to_string_lossy().into_owned(), text: std::fs::read_to_string(&doc).unwrap() };
            let (_, rir) = xtmd::check::compile_rir(&base, &[packs.clone()], &[main], "doc");
            if let Some(rir) = rir {
                check(&rir, &mut failures);
            }
        }
    }
    eprintln!("resolved IR: {} comments kept in {} files", comments, files);
    assert!(failures.is_empty(), "{} files lose comments:\n{}", failures.len(), failures.join("\n"));
    assert!(files >= 100, "only {} files compiled", files);
}

fn rir_of(path: &str) -> Value {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_xtmd")).current_dir(repo()).args(["compile", path, "--rir"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn each_comment_of_a_file_with_one_in_every_place_is_where_it_belongs() {
    let rir = rir_of("compiler/xtmd/tests/comments/every.xtmdpack");
    assert_eq!(rir["rir"], 2);
    let f = &rir["files"][0];
    assert_eq!(f["head"][0]["comment"][0], "// A file's first comment, with a blank line after it: a comment item.");
    assert_eq!(f["package_comments"]["leading"][0], "// The package line's leading.");
    assert_eq!(f["package_comments"]["trailing"], "// the package line's trailing");
    assert_eq!(f["imports"][0]["comments"]["trailing"], "// an import's trailing");
    assert_eq!(f["comment_items"][0]["comment"][0], "// the file's end: a comment item");
    let decl = |id: &str| f["declarations"].as_array().unwrap().iter().map(|d| d.as_object().unwrap().values().next().unwrap().clone()).find(|d| d["id"] == id).unwrap();

    let tone = decl("kit/tone");
    assert_eq!(tone["comments"]["doc"]["about"], "A state's colour.");
    assert_eq!(tone["comments"]["leading"][0], "// enum's leading, under its doc");
    assert_eq!(tone["comments"]["block_start"][0], serde_json::json!({ "at": 0, "text": "// enum's block_start" }));
    assert_eq!(tone["comments"]["block_end"][0], serde_json::json!({ "at": 0, "text": "// enum's block_end" }));
    let cases = tone["cases"].as_array().unwrap();
    assert_eq!(cases[0]["comments"], serde_json::json!({ "leading": ["// OK's leading"], "trailing": "// OK's trailing" }));
    assert_eq!(cases[1]["comments"]["trailing"], "// BAD's trailing");
    // comments on lines of their own with a blank line after them, and at the list's end, stand in the list
    assert_eq!(cases[2]["comment"][0], "// a comment item: a blank line after it");
    assert_eq!(cases[3]["comment"][0], "// a comment item: at the list's end");

    let stat = decl("kit/stat");
    assert_eq!(stat["fields"][0]["comments"]["doc"]["about"], "What it shows.", "a member's doc");
    assert_eq!(stat["fields"][1]["default"]["op"]["args"][1]["comments"]["free"][0], "// free: before 2");

    let add = decl("kit/add");
    assert_eq!(add["comments"]["doc"]["params"]["a"], "The first.");
    assert_eq!(add["comments"]["block_start"][0]["at"], 1, "the `(` is 0, the `{{` 1");
    assert_eq!(add["comments"]["block_end"][0]["text"], "// add's block_end at 1");
    let body = add["body"].as_array().unwrap();
    assert_eq!(body[0]["comments"]["block_start"][0], serde_json::json!({ "at": 0, "text": "// let's block_start at 0" }));
    let list = body[0]["let"]["value"]["list"].as_array().unwrap();
    assert_eq!(list[0]["comments"]["trailing"], "// a's trailing");
    assert_eq!(list[1]["comments"]["leading"][0], "// b's leading");
    assert_eq!(list[2]["comment"][0], "// a comment item at the list's end");
    let starts: Vec<(u64, &str)> = body[1]["comments"]["block_start"].as_array().unwrap().iter().map(|b| (b["at"].as_u64().unwrap(), b["text"].as_str().unwrap())).collect();
    assert_eq!(starts, [(0, "// if's block_start at 0"), (1, "// if's block_start at 1"), (2, "// if's block_start at 2")]);
    assert_eq!(body[1]["comments"]["block_end"].as_array().unwrap().len(), 2);
    let total = &body[2]["let"]["value"]["op"]["args"];
    // on the expression that starts after it: `b * a`, which starts with `b`
    assert_eq!(total[1]["comments"]["free"][0], "// before tax");
    assert_eq!(total[1]["op"]["args"][1]["comments"]["free"][0], "// and again");

    assert_eq!(decl("kit/one")["comments"]["trailing"], "// one space before the comment, not four");
    assert_eq!(decl("kit/two")["comments"]["leading"][0], "// not lined up: the next node's leading");
    let w = decl("kit/w");
    assert_eq!(w["comments"]["block_start"][0], serde_json::json!({ "at": 1, "text": "// w's block_start at 1" }));
    assert_eq!(w["inputs"][0]["comments"], serde_json::json!({ "leading": ["// a's leading"], "trailing": "// a's trailing" }));
}

#[test]
fn a_documents_front_matter_is_kept_as_its_text() {
    let rir = rir_of("compiler/xtmd/tests/comments/every.xtmd");
    let f = &rir["files"][0];
    assert_eq!(f["front_matter"]["text"], "title: Every place\ntags: [comments]");
    assert_eq!(f["front_matter"]["span"], serde_json::json!([0, 44]));
    assert_eq!(f["state"]["comments"]["leading"][0], "// above the state: its leading");
    assert_eq!(f["state"]["fields"][0]["comments"]["trailing"], "// n's trailing");
}

#[test]
fn the_shipped_resolved_ir_is_kept_as_it_was() {
    let kept = json(&repo().join("schemas/rir-history/1/rir.schema.json"));
    assert_eq!(kept["$defs"]["root"]["properties"]["rir"]["const"], 1);
    assert_eq!(json(&repo().join("schemas/rir.schema.json"))["$defs"]["root"]["properties"]["rir"]["const"], xtmd::check::RIR_VERSION);
}
