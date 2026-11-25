//! How long compiling takes: core once, then each page named on it.
//!
//!   cargo run --release -p xtmd --example speed -- page.xtmd …

use std::time::Instant;

use xtmd::check::{compile_core, compile_on, Source};

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../core");
    let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().map_or(false, |x| x == "xtmdpack")).collect();
    files.sort();
    let core: Vec<Source> = files.iter().map(|p| Source { path: p.display().to_string(), text: std::fs::read_to_string(p).unwrap() }).collect();
    let t = Instant::now();
    let base = compile_core(&core);
    println!("core: {:?}", t.elapsed());
    for path in std::env::args().skip(1) {
        let main = [Source { path: path.clone(), text: std::fs::read_to_string(&path).unwrap() }];
        let t = Instant::now();
        for _ in 0..4 {
            compile_on(&base, &[], &main, "doc");
        }
        let compile = t.elapsed() / 4;
        let mut m = compile_on(&base, &[], &main, "doc").module.expect("it compiles");
        let t = Instant::now();
        xtmd::lower::lower(&mut m);
        println!("{}: compiled in {:?}, lowered in {:?}", path, compile, t.elapsed());
    }
}
