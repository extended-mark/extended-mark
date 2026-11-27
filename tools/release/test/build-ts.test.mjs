// CI's npm job tests on several machines at once, each with its part of the packages: build-ts.mjs's --only and
// --except, as ci.yml gives them. Together they are every package, each once.

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { repo } from "../packages.mjs";

const list = (...args) => execFileSync(process.execPath, [join(repo, "tools/release/build-ts.mjs"), "--list", ...args], { cwd: repo, encoding: "utf8" }).split("\n").filter(Boolean);

test("CI's npm parts test every package, each once", () => {
  const ci = readFileSync(join(repo, ".github/workflows/ci.yml"), "utf8");
  const parts = [...ci.matchAll(/^\s+tests: (--(?:only|except) \S+)$/gm)].map((m) => m[1].split(" "));
  assert.ok(parts.length >= 2, "ci.yml's npm job names its parts");
  const tested = parts.flatMap((p) => list(...p));
  assert.deepEqual([...tested].sort(), [...list()].sort());
});

test("--only and --except choose by folder, and a folder that names no package stops the run", () => {
  assert.deepEqual(list("--only", "engines"), ["engines/web"]);
  assert.ok(list("--except", "engines").every((f) => !f.startsWith("engines/")));
  assert.ok(!list("--only", "renderers").includes("renderers-x"), "a folder is a whole name");
  assert.throws(() => execFileSync(process.execPath, [join(repo, "tools/release/build-ts.mjs"), "--test-only", "--only", "nosuch"], { cwd: repo, stdio: "pipe" }), /no package to test: nosuch/);
});
