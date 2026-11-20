//! Finds checks/rules.json, where in the spec each diagnostic's rule is written: the crate's own copy
//! when it is packaged (the release puts it in), else the repository's.

use std::path::PathBuf;

fn main() {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file = [here.join("checks/rules.json"), here.join("../../checks/rules.json")].into_iter().find(|p| p.is_file()).expect("checks/rules.json, in the crate or the repository");
    println!("cargo:rerun-if-changed={}", file.display());
    println!("cargo:rustc-env=XTMD_RULES={}", file.canonicalize().unwrap().display());
}
