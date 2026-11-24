//! Every IR version's pages, as the xtmd of its time wrote them (schemas/ir-history): each is brought to
//! the version this xtmd writes, and its Markdown reads as it did, or it is refused with the reason.

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {}", p.display(), e))
}

#[test]
fn every_kept_page_keeps_its_markdown_or_says_what_is_missing() {
    let mut checked = 0;
    for v in std::fs::read_dir(repo().join("schemas/ir-history")).unwrap() {
        let v = v.unwrap().path();
        let version: i64 = v.file_name().unwrap().to_str().unwrap().parse().unwrap();
        for e in std::fs::read_dir(v.join("pages")).unwrap() {
            let p = e.unwrap().path();
            let Some(stem) = p.to_str().unwrap().strip_suffix(".ir.json").map(str::to_string) else { continue };
            match xtmd_migrate::ir::migrate(&read(&p)) {
                Ok((ir, from)) => {
                    assert_eq!(from, version, "{}", p.display());
                    let f: xtmd_ir::ir::File = serde_json::from_value(ir).unwrap();
                    let md = xtmd_markdown::write_page(f.ir.front_matter.as_deref(), &xtmd_markdown::page(&f.ir).unwrap());
                    assert_eq!(md, read(Path::new(&format!("{}.md", stem))), "{}", p.display());
                }
                Err(why) => assert_eq!(why.join("\n") + "\n", read(Path::new(&format!("{}.refused.txt", stem))), "{}", p.display()),
            }
            checked += 1;
        }
    }
    assert!(checked > 10);
}

#[test]
fn the_schemas_kept_are_the_schemas() {
    let same = |a: &str, b: &str| assert_eq!(read(&repo().join(a)), read(&repo().join(b)), "{} is not {}", a, b);
    same("compiler/migrate/src/ir/ir-4.schema.json", "schemas/ir.schema.json");
    same("compiler/migrate/src/ir/ir-3.schema.json", "schemas/ir-history/3/ir.schema.json");
}

#[test]
fn what_isnt_an_ir_or_is_a_later_one_is_refused() {
    let why = |t: &str| xtmd_migrate::ir::migrate(t).unwrap_err().join("\n");
    assert!(why("{").starts_with("it isn't JSON"));
    assert!(why("{\"a\":1}").starts_with("it isn't an IR"));
    assert_eq!(why("{\"ir\":{\"version\":99}}"), "it is IR 99, and this xtmd writes IR 4: use the xtmd that wrote it, or a later one");
    assert_eq!(why("{\"ir\":{\"version\":2}}"), "it is IR 2, and no step leads from it: build the page again from its source");
    assert_eq!(why("{\"ir\":{\"version\":3}}"), "it doesn't fit IR 3's schema, at /ir: `xtmd` is missing");
}
