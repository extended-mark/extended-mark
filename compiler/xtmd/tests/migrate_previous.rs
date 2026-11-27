//! The release before's front end, as this xtmd carries it (compiler/xtmd/previous, copied from tag
//! v0.3.2 by tools/release/previous.mjs): it reads every file of the 0.3.2 corpus (checks/migrate-0.3)
//! as the released 0.3.2 command did, refuses what that command refused, and loses no comment.

use std::path::{Path, PathBuf};

use serde_json::Value;
use xtmd::check::Source;
use xtmd::migrate::readers::previous::Previous;
use xtmd::migrate::readers::Reader;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn units() -> Vec<Value> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(repo().join("checks/migrate-0.3/units.json")).unwrap()).unwrap();
    v["units"].as_array().unwrap().clone()
}

fn root_of(u: &Value) -> PathBuf {
    match u["id"].as_str().unwrap().strip_prefix("probes/") {
        Some(p) => repo().join("checks/migrate-0.3/probes").join(p),
        None => repo().join("checks/migrate-0.3/v0.3.2"),
    }
}

fn sources(root: &Path, paths: &Value) -> Vec<Source> {
    paths.as_array().unwrap().iter().map(|p| p.as_str().unwrap()).map(|p| Source { path: p.to_string(), text: std::fs::read_to_string(root.join(p)).unwrap() }).collect()
}

/// A resolved IR without what the reader adds, as rir 1 is.
fn as_rir_1(v: &Value) -> Value {
    let mut v = v.clone();
    for f in v["files"].as_array_mut().unwrap() {
        f.as_object_mut().unwrap().remove("left_out");
    }
    v
}

#[test]
fn every_0_3_2_file_is_read_as_the_released_0_3_2_command_read_it() {
    // 0.3.2's checker goes deep, as the command's main thread lets it
    std::thread::Builder::new().stack_size(64 << 20).spawn(every_0_3_2_file_is_read_as_the_released_0_3_2_command_read_it_).unwrap().join().unwrap();
}

fn every_0_3_2_file_is_read_as_the_released_0_3_2_command_read_it_() {
    let previous = Previous::new();
    assert_eq!(previous.release(), "0.3.2");
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(repo().join("schemas/rir-history/1/rir.schema.json")).unwrap()).unwrap();
    let (mut read, mut refused, mut documents) = (0, 0, 0);
    let mut wrong = Vec::new();
    for u in units() {
        let id = u["id"].as_str().unwrap();
        let root = root_of(&u);
        let (files, packs) = (sources(&root, &u["files"]), sources(&root, &u["packs"]));
        let packs: Vec<Source> = packs.into_iter().filter(|p| !files.iter().any(|f| f.path == p.path)).collect();
        let got = previous.read(&files, &packs);
        match (u["valid"].as_bool().unwrap(), got) {
            (true, Ok(r)) => {
                read += 1;
                if let Some(e) = xtmd::migrate::ir::schema::check(&as_rir_1(&r.rir), &schema) {
                    wrong.push(format!("{}: doesn't fit rir 1, {}", id, e));
                }
                for (path, md) in u["markdown"].as_object().unwrap() {
                    documents += 1;
                    let ours = r.markdown.get(path).map(|s| s.trim_end());
                    if ours != md.as_str().map(str::trim_end) {
                        wrong.push(format!("{}: {}'s Markdown isn't the released 0.3.2's", id, path));
                    }
                }
            }
            (true, Err(e)) => wrong.push(format!("{}: 0.3.2 took it, and the copy refuses it: {}", id, e.join("; "))),
            (false, Err(e)) => {
                refused += 1;
                assert!(!e.is_empty(), "{}: refused with nothing said", id);
            }
            (false, Ok(_)) => wrong.push(format!("{}: 0.3.2 refused it, and the copy reads it", id)),
        }
    }
    eprintln!("0.3.2's front end: {} units read, {} documents' Markdown the same, {} refused as 0.3.2 refused them", read, documents, refused);
    assert!(wrong.is_empty(), "{} wrong:\n{}", wrong.len(), wrong.join("\n"));
    assert!(read >= 100 && documents >= 60 && refused >= 200, "{} read, {} documents, {} refused", read, documents, refused);
}

#[test]
fn what_rir_1_has_no_place_for_is_given_beside_it() {
    // 0.3.2's checker goes deep, as the command's main thread lets it
    std::thread::Builder::new().stack_size(64 << 20).spawn(what_rir_1_has_no_place_for_is_given_beside_it_).unwrap().join().unwrap();
}

fn what_rir_1_has_no_place_for_is_given_beside_it_() {
    let doc = "---\ntitle: Notes\n---\n// the state\n$state {\n    //@about how many\n    n: int = 2;\n}\n\nwidget w(t: text) {\n    view {\n        @label(value = t);\n        // at the end of the view\n    }\n    markdown {\n        \\{t}\n    }\n}\n\nfn none() {\n    // alone in an empty body\n}\n@@@\n@w(t = \"a\");\n";
    let r = Previous::new().read(&[Source { path: "notes.xtmd".into(), text: doc.into() }], &[]).unwrap();
    let file = &r.rir["files"][0];
    let left = &file["left_out"];
    assert_eq!(left["front_matter"], "---\ntitle: Notes\n---\n");
    let texts: Vec<&str> = left["comments"].as_array().unwrap().iter().map(|c| c["text"].as_str().unwrap()).collect();
    for c in ["//@about how many", "// at the end of the view"] {
        assert!(texts.contains(&c), "{:?} isn't among {:?}", c, texts);
    }
    // what rir 1 kept isn't given twice
    assert!(!texts.contains(&"// the state"), "{:?}", texts);
    // each with where it stands in the file, and where the code ends
    for c in left["comments"].as_array().unwrap() {
        let (a, b) = (c["span"][0].as_u64().unwrap() as usize, c["span"][1].as_u64().unwrap() as usize);
        assert_eq!(&doc[a..b], c["text"].as_str().unwrap());
    }
    let d = left["divider"][0].as_u64().unwrap() as usize;
    assert!(doc[d..].starts_with("@@@"));
}

#[test]
fn every_comment_of_the_corpus_is_in_rir_1_or_beside_it() {
    // 0.3.2's checker goes deep, as the command's main thread lets it
    std::thread::Builder::new().stack_size(64 << 20).spawn(every_comment_of_the_corpus_is_in_rir_1_or_beside_it_).unwrap().join().unwrap();
}

fn every_comment_of_the_corpus_is_in_rir_1_or_beside_it_() {
    let previous = Previous::new();
    let mut lost = Vec::new();
    for u in units().into_iter().filter(|u| u["valid"].as_bool().unwrap()) {
        let root = root_of(&u);
        let files = sources(&root, &u["files"]);
        let packs: Vec<Source> = sources(&root, &u["packs"]).into_iter().filter(|p| !files.iter().any(|f| f.path == p.path)).collect();
        let r = previous.read(&files, &packs).unwrap();
        for f in r.rir["files"].as_array().unwrap() {
            let path = f["path"].as_str().unwrap();
            let text = &files.iter().find(|s| s.path == path).unwrap().text;
            // every `//` the source's code holds, by a count of its own: lines whose code starts one
            let (all, _, _) = xtmd::migrate::readers::previous::comments_of(path, text);
            let mut kept = 0;
            fn count(v: &Value, n: &mut usize) {
                match v {
                    Value::Object(m) => m.iter().for_each(|(k, x)| {
                        if k == "before" || k == "after" || (k == "comments" && x.is_array()) {
                            *n += x.as_array().map_or(0, |a| a.iter().filter(|c| c.is_string()).count());
                        }
                        if k != "left_out" {
                            count(x, n)
                        }
                    }),
                    Value::Array(a) => a.iter().for_each(|x| count(x, n)),
                    _ => {}
                }
            }
            count(f, &mut kept);
            let beside = f["left_out"]["comments"].as_array().unwrap().len();
            if kept + beside != all.len() {
                lost.push(format!("{}: {}: {} comments, {} in rir 1 and {} beside it", u["id"], path, all.len(), kept, beside));
            }
        }
    }
    assert!(lost.is_empty(), "{}", lost.join("\n"));
}

#[test]
fn the_front_end_is_the_command_lines_and_never_the_webassemblys() {
    let wasm = std::fs::read_to_string(repo().join("compiler/wasm/Cargo.toml")).unwrap();
    let dep = wasm.lines().find(|l| l.trim_start().starts_with("xtmd = ")).unwrap();
    assert!(dep.contains("default-features = false") && !dep.contains("\"cli\"") && !dep.contains("\"fs\""), "{}", dep);
    assert_eq!(xtmd::previous::RELEASE, xtmd::migrate::readers::PREVIOUS);
    assert_eq!(xtmd::previous::core_files::SOURCES.len(), std::fs::read_dir(repo().join("compiler/xtmd/previous/core")).unwrap().count());
}
