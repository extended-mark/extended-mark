//! The compiler reads a page's sentences in Rust (xtmd-eval's md.rs), and engines with core's own
//! `md_inline`. Both give the same segments, for every example of CommonMark and GitHub, for core's own
//! words, and for texts made of everything inline Markdown reads.

use std::path::PathBuf;
use std::rc::Rc;

use xtmd_check::{compile_core, Source};
use xtmd_eval::{interp::Constants, Binding, Machine, Program, Value};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn program() -> Program {
    let dir = repo().join("core");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().map_or(false, |x| x == "xtmdpack")).collect();
    files.sort();
    compile_core(&files.iter().map(|p| Source { path: p.display().to_string(), text: std::fs::read_to_string(p).unwrap() }).collect::<Vec<_>>()).program()
}

fn record(ty: &str, fields: Vec<(&str, Value)>) -> Value {
    Value::Record(Rc::new(xtmd_eval::value::Record { ty: Rc::from(ty), fields: fields.into_iter().map(|(n, v)| (Rc::from(n), v)).collect() }))
}

const LINKS: &[(&str, &str)] = &[("foo", "/url"), ("bar", "/bar \"x\""), ("foo bar", "/spaced"), ("ss", "/eszett"), ("a*b", "/star")];

/// A text as a sentence's template: each `\u{fffc}` a hole, a text's or a view's by turns.
fn read(prog: &Program, native: bool, text: &str) -> Result<String, String> {
    let mut m = Machine::new(prog, Box::new(Constants));
    m.md_native = native;
    let pieces: Vec<&str> = text.split('\u{fffc}').collect();
    let hole = |k: usize| Value::Case(Rc::new(xtmd_eval::value::Case { ty: Rc::from("core/hole"), name: Rc::from("TEXT"), values: vec![(Rc::from("value"), Value::text(["*shown*", "", "`x`"][k % 3]))] }));
    let holes = m.make_list((0..pieces.len() - 1).map(hole).collect()).unwrap();
    let pieces = m.make_list(pieces.into_iter().map(Value::text).collect()).unwrap();
    let links = LINKS.iter().map(|(k, v)| record("record", vec![("key", Value::text(k)), ("value", Value::text(v))])).collect();
    let links = record("core/map", vec![("entries", m.make_list(links).unwrap())]);
    let template = record("core/template", vec![("pieces", pieces), ("holes", holes)]);
    match m.call("core/md_inline", vec![Binding::Value(template), Binding::Value(links)]) {
        Ok(Some(v)) => Ok(serde_json::to_string(&v.to_ir()).unwrap()),
        Ok(None) => Err("nothing".into()),
        Err(f) => Err(f.message),
    }
}

/// The texts that the two read differently.
fn differ<'a>(prog: &Program, texts: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    texts
        .into_iter()
        .filter_map(|t| {
            let (core, rust) = (read(prog, false, t), read(prog, true, t));
            // core's own code has a limit of steps, and this one hasn't
            let limited = matches!(&core, Err(e) if e.contains("steps"));
            (core != rust && !limited).then(|| format!("{:?}\n  core {:?}\n  rust {:?}", t, core, rust))
        })
        .collect()
}

#[test]
fn every_example_reads_the_same() {
    let prog = program();
    let mut texts = Vec::new();
    for e in std::fs::read_dir(repo().join("checks/commonmark")).unwrap() {
        let p = e.unwrap().path();
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&p).unwrap_or_default()) else { continue };
        texts.extend(v["examples"].as_array().into_iter().flatten().filter_map(|x| x["markdown"].as_str()).map(|t| t.trim_end_matches('\n').to_string()));
    }
    assert!(texts.len() > 1000, "{}", texts.len());
    let bad = differ(&prog, texts.iter().map(String::as_str));
    assert!(bad.is_empty(), "{} of {}:\n{}", bad.len(), texts.len(), bad[..bad.len().min(6)].join("\n"));
}

#[test]
fn every_turn_of_the_reading_is_the_same() {
    let prog = program();
    let stars = "*".repeat(70);
    let texts = [
        "", " ", "\n", "\\", "a\\", "\\\n b", "a  \n  b", "a \n b", "\u{fffc}", "*\u{fffc}*", "a\u{fffc}b\u{fffc}", "`\u{fffc}`", "[\u{fffc}](/x)", "<\u{fffc}>",
        "$x$", "$ x$", "$x $", "$x$1", "$5 and $6", "$x\\$y$", "$\u{fffc}^2$", "$\\\\{a\\\\}$", "$", "$$", "$\\frac$", "${$", "$x^$", "![$x^2$](/i)",
        "**a**", "***a***", "*a **b* c**", "_a_b_", "a_b_c", "~a~", "~~a~~", "~~~a~~~", "~a~~", "**a", "a**", "* a*", "*a *", "**a*b**", &stars, "****a****b****", "*(*a*)*", "«*a*»", "é*a*é", "a*\"b\"*",
        "[a](/b)", "[a](/b \"t\")", "[a](/b 't')", "[a](/b (t))", "[a](<b c>)", "[a](<b\nc>)", "[a](b\\)c)", "[a](b(c)d)", "[a](b(c)", "[a]()", "[a](/b", "[a](/b \"t)", "[a](/b\"t\")", "[a](\n/b\n\"t\"\n)", "[a](\n\n/b)", "[a](&amp;x\\*)",
        "[foo]", "[Foo]", "[foo][]", "[x][foo]", "[x][nope]", "[foo][nope]", "[foo\nbar]", "[FOO   BAR]", "[SS]", "[ẞ]", "[ß]", "[a*b]", "[a][b][foo]", "[[foo]]", "![foo]", "![a *b* `c` [d](/e)](/f)", "![a\nb  \nc](/d)", "![[^1] \u{fffc}](/x)", "[a [b](/c) d](/e)", "[a ![b](/c) d](/e)", "]", "[", "![", "[]", "[](/x)", "\\[foo]", "[foo\\]", "[a\\\\]",
        "[^1]", "[^]", "[^a b]", "[^a[b]", "[^a", "a[^1]b",
        "<http://a.b>", "<a@b.c>", "<a@b>", "<a@-b.c>", "<made-up:x>", "<m:x>", "<a b>", "<>", "<http://a b>", "<b>", "</b>", "</b x>", "<b a=\"x>\" c>", "<b a='x", "<b `>", "<!-- a -->", "<!-- a", "<?a?>", "<!A b>", "<![CDATA[x]]>", "<1>", "<b/>", "<b\n>", "a < b", "<",
        "&amp;", "&copy", "&nope;", "&#35;", "&#0;", "&#x22;", "&#X1F600;", "&#xD800;", "&#12345678;", "&#x1234567;", "&#;", "&#x;", "&#xg;", "&", "&;", "&a", "&hearts;&CounterClockwiseContourIntegral;", "&#1114112;",
        "www.a.b", "www.a", "www.a_b.c", "xwww.a.b", "(www.a.b)", "www.a.b/(c)", "www.a.b/(c))", "www.a.b/c?d=e&amp;f", "www.a.b&amp;", "www.a.b;", "www.a.b.", "www.a.b!?*", "http://a.b/c", "HTTP://a.b", "https://a", "ftp://x.y z", "http://.a", "http://a.b<c", "http://\u{fffc}", "hello www.x.y, and more", "f", "ftp:/", "www.", "*www.a.b*", "www.a.b\u{3000}c",
        "a@b.c", "a.b+c@d-e.f", "a@b", "a@b.c.", "a@b.c-", "a@b_c.d", "@b.c", " @b.c", "é@b.c", "x a@b.c y", "a@.b.c", "a@b.c_", "a@@b.c",
        "`a`", "``a`b``", "` a `", "`  `", "`a", "``a`", "`a\nb`", "`` ` ``", "\\`a`",
        "日本語 *強調* 。", "e\u{301}*a*", "🙂**b**🙂", "\u{a0}*a*", "a\tb \t\nc", "\r*a*\r", "a\u{0}b",
    ];
    let bad = differ(&prog, texts);
    assert!(bad.is_empty(), "{} of {}:\n{}", bad.len(), texts.len(), bad[..bad.len().min(8)].join("\n"));
}

#[test]
fn texts_made_of_everything_read_the_same() {
    let prog = program();
    let parts = [
        "*", "**", "***", "_", "__", "~", "~~", "`", "``", "[", "]", "(", ")", "![", "](/u)", "](/u \"t\")", "[foo]", "[^1]", "<", ">", "<b>", "</b>", "<http://a.b>", "&amp;", "&", ";", "\\", "\\*", "\n", "  \n", " ", "  ", "a", "b c", "foo", "é", "。", "\"", "'", ".", "!", "$", "$x$", "www.a.b", "http://c.d/e", "x@y.z", "@", "\u{fffc}", "1", "-", "\t", "&#35;", "日",
    ];
    // the same texts every run
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let texts: Vec<String> = (0..4000).map(|_| (0..1 + next(14)).map(|_| parts[next(parts.len())]).collect()).collect();
    let bad = differ(&prog, texts.iter().map(String::as_str));
    assert!(bad.is_empty(), "{} of {}:\n{}", bad.len(), texts.len(), bad[..bad.len().min(8)].join("\n"));
}
