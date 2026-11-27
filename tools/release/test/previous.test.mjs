// previous.mjs: the release before's front end copied from its tag, on a small repository of each shape (a
// crate a part, as 0.3 has, and the xtmd crate's modules, as 0.4 has), and on this repository's copy.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { copyOf, drift, repo, rewrite } from "../previous.mjs";

const scratch = mkdtempSync(join(tmpdir(), "previous-"));
after(() => rmSync(scratch, { recursive: true, force: true }));

function tagged(name, files, tag) {
  const root = join(scratch, name);
  for (const [p, t] of Object.entries(files)) {
    mkdirSync(dirname(join(root, p)), { recursive: true });
    writeFileSync(join(root, p), t);
  }
  const git = (...a) => execFileSync("git", a, { cwd: root, stdio: "pipe" });
  git("init", "-q");
  git("add", ".");
  git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "release");
  git("tag", tag);
  return root;
}

test("a part's paths become the copy's, and its tests and doc examples are left out", () => {
  const src = [
    "//! ```",
    "//! use xtmd_syntax::parse;",
    "//! ```",
    "use crate::ty::Ty;",
    "use xtmd_ir::ir;",
    "pub(crate) fn f() -> &'static str { env!(\"CARGO_PKG_VERSION\") }",
    "fn e() -> &'static str { include_str!(env!(\"XTMD_ENTITIES\")) }",
    "fn xtmd_line() {}",
    "#[cfg(test)]",
    "mod tests {}",
  ].join("\n");
  const out = rewrite(src, "check", ["syntax", "ir", "check"], { release: "0.3.2" });
  assert.match(out, /^\/\/! ```text$/m);
  assert.match(out, /use crate::previous::check::ty::Ty;/);
  assert.match(out, /use crate::previous::ir::ir;/);
  assert.match(out, /pub\(crate\) fn f\(\) -> &'static str \{ "0\.3\.2" \}/);
  assert.match(out, /include_str!\(concat!\(env!\("CARGO_MANIFEST_DIR"\), "\/previous\/core\/entities\.xtmdpack"\)\)/);
  assert.match(out, /fn xtmd_line\(\)/, "a name that only starts like a crate's stays");
  assert.match(out, /#\[cfg\(any\(\)\)\]\nmod tests/);
  // one crate: its root is the copy
  assert.match(rewrite("use crate::syntax::parse;", "check", [], { modules: true, release: "0.4.0" }), /use crate::previous::syntax::parse;/);
});

test("a tag of 0.3: a crate a part, lib.rs its module, and its core", () => {
  const files = { "compiler/Cargo.toml": '[workspace.package]\nversion = "0.3.2"\n', "core/types.xtmdpack": "package core;\n" };
  for (const p of ["syntax", "ir", "eval", "check", "lower", "markdown"]) {
    files[`compiler/${p}/src/lib.rs`] = `pub mod ${p};\n`;
    files[`compiler/${p}/src/${p}.rs`] = `use crate::x;\n`;
    files[`compiler/${p}/tests/t.rs`] = "a test\n";
  }
  const root = tagged("crates", files, "v0.3.2");
  const { version, files: out } = copyOf("v0.3.2", root);
  assert.equal(version, "0.3.2");
  assert.equal(out.get("ir/mod.rs"), "pub mod ir;\n");
  assert.equal(out.get("ir/ir.rs"), "use crate::previous::ir::x;\n", "a part's file named for it stays in its folder");
  assert.ok(![...out.keys()].some((k) => k.includes("tests/")), "tests aren't copied");
  assert.equal(out.get("core/types.xtmdpack"), "package core;\n");
  assert.match(out.get("core_files.rs"), /\("core\/types\.xtmdpack", include_str!\("core\/types\.xtmdpack"\)\)/);
  assert.match(out.get("mod.rs"), /pub const RELEASE: &str = "0\.3\.2";/);
  assert.match(out.get("mod.rs"), /pub const TAG: &str = "v0\.3\.2";/);
});

test("a tag of 0.4 on: the xtmd crate's modules, without its command, its editor support or its own previous", () => {
  const root = tagged(
    "modules",
    {
      "compiler/Cargo.toml": '[workspace.package]\nversion = "0.4.0"\n',
      "compiler/xtmd/src/lib.rs": "pub mod check;\n",
      "compiler/xtmd/src/main.rs": "fn main() {}\n",
      "compiler/xtmd/src/check/mod.rs": "use crate::syntax::x;\n",
      "compiler/xtmd/src/syntax.rs": "pub fn x() {}\n",
      "compiler/xtmd/src/cli/mod.rs": "cli\n",
      "compiler/xtmd/src/lsp/mod.rs": "lsp\n",
      "compiler/xtmd/previous/mod.rs": "0.3.2's\n",
      "core/types.xtmdpack": "package core;\n",
      "core/surfaces/core@1.xtmdpack": "package core@1;\n",
      "core/migrations/core@2.xtmdpack": "package core;\n",
    },
    "v0.4.0",
  );
  const { files } = copyOf("v0.4.0", root);
  assert.deepEqual([...files.keys()].filter((k) => k.endsWith(".rs")).sort(), ["check/mod.rs", "core_files.rs", "mod.rs", "syntax.rs"]);
  // core is its own files: its surfaces and migrations are only xtmd migrate's
  assert.deepEqual([...files.keys()].filter((k) => k.startsWith("core/")), ["core/types.xtmdpack"]);
  assert.equal(files.get("check/mod.rs"), "use crate::previous::syntax::x;\n");
});

test("this repository's copy is what its tag gives", () => {
  const mod = readFileSync(join(repo, "compiler/xtmd/previous/mod.rs"), "utf8");
  const tag = mod.match(/pub const TAG: &str = "([^"]+)"/)?.[1];
  assert.ok(tag, "compiler/xtmd/previous/mod.rs names its tag");
  assert.deepEqual(drift(join(repo, "compiler/xtmd/previous"), copyOf(tag).files), []);
});
