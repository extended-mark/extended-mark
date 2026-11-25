//! core's table of named characters, embedded when the compiler is built, is what checking its file gives.

use xtmd::check::{compile_core, compile_core_logged, compile_on, Source};

fn core_sources() -> Vec<Source> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../core");
    let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().map_or(false, |x| x == "xtmdpack")).collect();
    files.sort();
    files.iter().map(|p| Source { path: p.display().to_string(), text: std::fs::read_to_string(p).unwrap() }).collect()
}

#[test]
fn the_embedded_table_is_what_checking_the_file_gives() {
    let core = core_sources();
    // compiled with a log, every file of core's is read and checked
    let (fast, whole) = (compile_core(&core), compile_core_logged(&core));
    let said = |c: &xtmd::check::Checker| c.diags.iter().map(|f| format!("{} {}: {}", f.file, f.diagnostic.code, f.diagnostic.message)).collect::<Vec<_>>();
    assert_eq!(said(&fast), said(&whole));
    assert!(!whole.diags.iter().any(|f| f.diagnostic.is_error()));
    let same = |name: &str| assert_eq!(serde_json::to_value(fast.constants[name].to_ir()).unwrap(), serde_json::to_value(whole.constants[name].to_ir()).unwrap(), "{}", name);
    same("core/entities");
    assert_eq!(fast.constants.len(), whole.constants.len());
    fast.constants.keys().for_each(|k| same(k));
    let text = |c: &xtmd::check::Checker| {
        let l = c.lets.iter().position(|l| l.full == "core/entities").unwrap();
        serde_json::to_string(c.out.let_values.get(&l).unwrap()).unwrap()
    };
    assert_eq!(text(&fast), text(&whole));
}

#[test]
fn a_table_that_was_changed_is_checked_as_it_is() {
    let mut core = core_sources();
    let k = core.iter().position(|s| s.path.ends_with("entities.xtmdpack")).unwrap();
    core[k].text = core[k].text.replacen("\"AElig\"", "\"AElog\"", 1);
    let c = compile_core(&core);
    assert!(serde_json::to_string(&c.constants["core/entities"].to_ir()).unwrap().contains("AElog"));
}

#[test]
fn compiles_on_one_core_dont_reach_each_other() {
    let base = compile_core(&core_sources());
    let pack = Source { path: "acme.xtmdpack".into(), text: "package acme;\n\npack acme@1 {\n    implements core.draw@1;\n}\n\ntokens acme {\n    color.INK = color_value { light = \"#111111\", dark = \"#eeeeee\" };\n    color.ACME = color_value { light = \"#222222\", dark = \"#dddddd\" };\n}\n\nfn text.shouted(self): text {\n    return self.upper();\n}\n".into() };
    let doc = Source { path: "page.xtmd".into(), text: "# Hi\n\nWords &copy; *here*.\n".into() };
    let ir = |base: &xtmd::check::Checker, packs: &[Vec<Source>]| {
        let c = compile_on(base, packs, std::slice::from_ref(&doc), "doc");
        assert!(c.module.is_some(), "{:?}", c.diagnostics.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        serde_json::to_string(&c.module.unwrap()).unwrap()
    };
    let before = ir(&base, &[]);
    let styled = ir(&base, &[vec![pack]]);
    assert!(styled.contains("#111111") && !before.contains("#111111"));
    assert_eq!(ir(&base, &[]), before);
    assert_eq!(ir(&compile_core(&core_sources()), &[]), before);
}
