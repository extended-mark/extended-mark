// tools/llm/check.mjs: what it finds wrong in small trees, and that this repository's own notes are right.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { pathsIn, problems, tracked } from "../check.mjs";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

const right = {
  "CLAUDE.md": "| where | [.index/](.index/) (`00-OVERVIEW.txt`) |\nSee `docs/guide.md`.\n",
  ".index/00-OVERVIEW.txt": "  01-code.txt   code/\ndocs/  guide.md\nVERSION\n",
  ".index/01-code.txt": "code/a    a crate\n  src/lib.rs   all of it\n",
  ".knowledge/how/it-works.txt": "code/a/src/lib.rs does it; lib.rs is its name alone. A tap/click is no path.\n",
  "code/a/Cargo.toml": "[package]\n",
  "code/a/src/lib.rs": "",
  "docs/guide.md": "",
  VERSION: "1\n",
};
const found = (change) => {
  const tree = { ...right, ...change };
  for (const k of Object.keys(tree)) if (tree[k] === null) delete tree[k];
  return problems(Object.keys(tree), (f) => tree[f]);
};

test("notes that agree with their tree have nothing wrong", () => assert.deepEqual(found({}), []));

test("a path that is gone is named with its line", () => {
  assert.deepEqual(found({ ".knowledge/how/it-works.txt": "fine\ncode/a/src/gone.rs does it\n" }), [".knowledge/how/it-works.txt:2: there is no code/a/src/gone.rs"]);
  assert.deepEqual(found({ ".index/01-code.txt": "code/a\n  src/gone.rs\n" }), [".index/01-code.txt:2: there is no src/gone.rs"]);
  assert.deepEqual(found({ "CLAUDE.md": "`docs/gone.md`\n" }), ["CLAUDE.md:1: there is no docs/gone.md"]);
  assert.deepEqual(found({ ".knowledge/how/it-works.txt": "gone.rs\n" }), [".knowledge/how/it-works.txt:1: there is no gone.rs"]);
});

test("a path is from a folder only in the index, and only after that folder", () => {
  assert.deepEqual(found({ ".index/01-code.txt": "  src/lib.rs\ncode/a\n" }), [".index/01-code.txt:1: there is no src/lib.rs"]);
  assert.deepEqual(found({ ".knowledge/how/it-works.txt": "code/a\nsrc/lib.rs\n" }), [".knowledge/how/it-works.txt:2: there is no src/lib.rs"]);
});

test("a star stands for names, and must find one", () => {
  assert.deepEqual(found({ ".knowledge/how/it-works.txt": "code/*/src/*.rs\n" }), []);
  assert.deepEqual(found({ ".knowledge/how/it-works.txt": "code/*/src/*.ts\n" }), [".knowledge/how/it-works.txt:1: there is no code/*/src/*.ts"]);
});

test("a part of the repository no index file names is told", () => {
  assert.deepEqual(found({ "code/b/package.json": "{}" }), [".index: says nothing of code/b"]);
  assert.deepEqual(found({ "new/x.txt": "" }), [".index: says nothing of new"]);
});

test("a package of a test's isn't one to name", () => assert.deepEqual(found({ "code/a/test/app/package.json": "{}" }), []));

test("only small .txt notes", () => {
  assert.deepEqual(found({ ".knowledge/how/long.txt": "a line\n".repeat(31) }), [".knowledge/how/long.txt: 31 lines, over 30: a note is one subject"]);
  assert.deepEqual(found({ ".knowledge/how/long.txt": "a line\n".repeat(30) }), []);
  assert.deepEqual(found({ ".knowledge/how/wide.txt": "x".repeat(111) }), [".knowledge/how/wide.txt:1: over 110 characters"]);
  assert.deepEqual(found({ ".knowledge/how/it.md": "a note\n" }), [".knowledge/how/it.md: only .txt files go here"]);
});

test("the overview lists every file of the index, and is there", () => {
  assert.deepEqual(found({ ".index/02-more.txt": "docs/\n" }), [".index/00-OVERVIEW.txt: doesn't list .index/02-more.txt"]);
  assert.ok(found({ ".index/00-OVERVIEW.txt": null }).includes(".index/00-OVERVIEW.txt: isn't there"));
  assert.ok(found({ "CLAUDE.md": null }).includes("CLAUDE.md: isn't there"));
});

test("what is no path: placeholders, addresses, packages, flags, endings, built files", () => {
  assert.deepEqual(pathsIn("notes/<version>.md https://x.y/z @extended-mark/dom --format .xtmdpack 1,000"), []);
  assert.deepEqual(found({ ".knowledge/how/it-works.txt": "code/a/target/x.rs and dist/index.js and page.ir.json\n" }), []);
});

test("this repository's notes agree with it", () => assert.deepEqual(problems(tracked(repo), (f) => readFileSync(join(repo, f), "utf8")), []));

test("the command says so, and fails when they don't", () => {
  assert.match(execFileSync("node", [join(repo, "tools/llm/check.mjs")], { encoding: "utf8" }), /agree with the repository/);
});
