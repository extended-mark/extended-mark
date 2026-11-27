#!/usr/bin/env node
// The release before's front end, copied into compiler/xtmd/previous/ from its tag, so that this release's
// xtmd can read a tree written for it (compiler/xtmd/src/migrate/readers/previous.rs). It replaces the copy
// there: the repository holds exactly one older front end. The copy is a module of the xtmd crate, built
// only with its command line ("cli"), so it ships with the crate and never with the WebAssembly build;
// modules rather than crates of their own, because Cargo merges two releases it counts as compatible (0.4.2
// and 0.4.3) and the crate is published. Nothing in it is edited by hand: run this again.
//
//   node tools/release/previous.mjs v0.3.2        # at a release: the tag of the release before
//   node tools/release/previous.mjs --check       # the copy is the one its tag gives (CI)
//
// A tag of 0.3 has a crate a part (compiler/syntax, compiler/check, ...); from 0.4 the parts are modules of
// compiler/xtmd. Either way the copy is the parts that read a tree: its syntax, its checker, its IR, its
// evaluator, its lowering and its Markdown, and its core, embedded.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const repo = join(here, "..", "..");
export const PARTS = ["syntax", "ir", "eval", "check", "lower", "markdown"];
const DEST = "compiler/xtmd/previous";

const git = (args, cwd = repo) => execFileSync("git", args, { cwd, encoding: "utf8", maxBuffer: 1 << 28 });
const listed = (tag, path, root = repo) => git(["ls-tree", "-r", "--name-only", tag, "--", path], root).split("\n").filter(Boolean);
const shown = (tag, path, root = repo) => git(["show", `${tag}:${path}`], root);

/// A part's source, its paths made the copy's: `crate::` is the part's module, and another part's crate is
/// its module too. Its tests and doc examples are left out: they run with the release they came from.
export function rewrite(text, part, parts, { modules = false, release }) {
  let t = text;
  if (modules) {
    // one crate already: its root is the copy
    t = t.replace(/\bcrate::/g, "crate::previous::");
  } else {
    t = t.replace(/\bcrate::/g, `crate::previous::${part}::`);
    for (const other of parts) t = t.replace(new RegExp(`\\bxtmd_${other}\\b`, "g"), `crate::previous::${other}`);
  }
  // the release it was: what it writes as its own ("xtmd": "0.3.2" in a resolved IR)
  t = t.replace(/env!\("CARGO_PKG_VERSION"\)/g, JSON.stringify(release));
  // core's table of HTML's named characters, which a build script found: the copy's own core
  t = t.replace(/include_str!\(env!\("XTMD_ENTITIES"\)\)/g, 'include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/previous/core/entities.xtmdpack"))');
  t = t.replace(/#\[cfg\(test\)\]/g, "#[cfg(any())]");
  t = t.replace(/^(\s*\/\/[/!]\s*)```(rust)?\s*$/gm, "$1```text");
  return t;
}

/// The files the tag's front end is: { "syntax/mod.rs": text, ... } and core's.
export function copyOf(tag, root = repo) {
  // a part that is the release before's own previous front end is never copied: one older front end only
  const listed_ = (t, p) => listed(t, p, root);
  const shown_ = (t, p) => shown(t, p, root);
  const version = shown_(tag, "compiler/Cargo.toml").match(/^version = "([^"]+)"/m)[1];
  const out = new Map();
  const modules = !listed_(tag, "compiler/syntax/src").length;
  // 0.3: a crate a part; from 0.4: the xtmd crate's modules, but for what only its own command does
  const parts = modules
    ? [...new Set(listed_(tag, "compiler/xtmd/src").map((p) => p.slice("compiler/xtmd/src/".length).split("/")[0].replace(/\.rs$/, "")))].filter((m) => !["lib", "main", "cli", "lsp", "live"].includes(m))
    : PARTS;
  for (const part of parts) {
    const src = modules ? `compiler/xtmd/src/${part}` : `compiler/${part}/src`;
    let files = listed_(tag, src).filter((p) => p.endsWith(".rs") && p.startsWith(`${src}/`));
    if (modules && !files.length) files = listed_(tag, `${src}.rs`);
    if (!files.length) throw new Error(`${tag} has no ${src}`);
    for (const f of files) {
      // a module of one file is that file; else its folder, a crate's lib.rs being the folder's mod.rs
      const to = !f.startsWith(`${src}/`) ? `${part}.rs` : `${part}/${!modules && f === `${src}/lib.rs` ? "mod.rs" : f.slice(src.length + 1)}`;
      out.set(to, rewrite(shown_(tag, f), part, parts, { modules, release: version }));
    }
  }
  // core's own files, at the top of core/: its surfaces and migrations are only xtmd migrate's
  const core = listed_(tag, "core").filter((p) => p.endsWith(".xtmdpack") && p.split("/").length === 2);
  for (const f of core) out.set(f, shown_(tag, f));
  const names = core.map((f) => f.slice("core/".length));
  out.set(
    "core_files.rs",
    `//! ${version}'s core, as its tag has it: the files its checker is given, by name.\n\n` +
      `pub const SOURCES: &[(&str, &str)] = &[\n` +
      names.map((n) => `    ("core/${n}", include_str!("core/${n}")),\n`).join("") +
      `];\n`,
  );
  out.set(
    "mod.rs",
    `//! ${version}'s front end, copied from tag ${tag} by tools/release/previous.mjs: the release before this\n` +
      `//! one, which reads a tree written for it (src/migrate/readers/previous.rs). Not edited by hand.\n\n` +
      `/// The release it is.\npub const RELEASE: &str = ${JSON.stringify(version)};\n/// The tag it was copied from.\npub const TAG: &str = ${JSON.stringify(tag)};\n\n` +
      `pub mod core_files;\n` +
      parts.map((p) => `pub mod ${p};\n`).join(""),
  );
  return { version, files: out };
}

function write(dest, files) {
  rmSync(dest, { recursive: true, force: true });
  for (const [rel, text] of files) {
    mkdirSync(dirname(join(dest, rel)), { recursive: true });
    writeFileSync(join(dest, rel), text);
  }
}

/// What differs between the copy in the repository and what its tag gives, path by path.
export function drift(dest, files, root = repo) {
  const wrong = [];
  for (const [rel, text] of files) {
    const p = join(dest, rel);
    if (!existsSync(p)) wrong.push(`${rel}: missing`);
    else if (readFileSync(p, "utf8") !== text) wrong.push(`${rel}: not what the tag gives`);
  }
  const have = git(["ls-files", DEST], root).split("\n").filter(Boolean).map((p) => p.slice(DEST.length + 1));
  for (const rel of have) if (!files.has(rel)) wrong.push(`${rel}: not from the tag`);
  return wrong;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const dest = join(repo, DEST);
  if (args[0] === "--check") {
    const mod = existsSync(join(dest, "mod.rs")) ? readFileSync(join(dest, "mod.rs"), "utf8") : "";
    const tag = mod.match(/pub const TAG: &str = "([^"]+)"/)?.[1];
    if (!tag) {
      console.error(`${DEST}/mod.rs names no tag: run node tools/release/previous.mjs <tag of the release before>`);
      process.exit(1);
    }
    const wrong = drift(dest, copyOf(tag).files);
    if (wrong.length) {
      console.error(`${DEST} isn't ${tag}'s front end:\n  ${wrong.join("\n  ")}\nrun node tools/release/previous.mjs ${tag}`);
      process.exit(1);
    }
    console.log(`${DEST}: ${tag}'s front end, as its tag gives it`);
  } else if (args.length === 1 && !args[0].startsWith("-")) {
    const { version, files } = copyOf(args[0]);
    write(dest, files);
    console.log(`${DEST}: ${version}'s front end from ${args[0]}, ${files.size} files`);
  } else {
    console.error("usage: node tools/release/previous.mjs <tag of the release before> | --check");
    process.exit(2);
  }
}
