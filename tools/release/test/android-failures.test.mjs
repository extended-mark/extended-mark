// tools/release/android-failures.py: only tests that never started may be tried again.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = join(dirname(fileURLToPath(import.meta.url)), "..", "android-failures.py");
const unfetched = `<testcase name="classMethod" classname="a.Live"><failure message="java.lang.AssertionError: Failed to fetch maven artifact org.robolectric:android-all"/></testcase>`;
const failed = `<testcase name="drags" classname="a.Drag"><failure message="expected 1 &#10; but was 2"/></testcase>`;
const passed = `<testcase name="draws" classname="a.Page"/>`;

/** What it says and ends with, for results in modules of the repository. */
function run(modules) {
  const repo = mkdtempSync(join(tmpdir(), "xtmd-android-failures-"));
  for (const [module, cases] of Object.entries(modules)) {
    const dir = join(repo, module, "build/test-results/testDebugUnitTest");
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "TEST-a.xml"), `<testsuite>${cases}</testsuite>`);
  }
  const r = spawnSync("python3", [script, repo], { encoding: "utf8" });
  rmSync(repo, { recursive: true });
  return { status: r.status, out: r.stdout };
}

test("tests that never started, and nothing else, may be tried again", () => {
  const r = run({ "live/compose": unfetched, "renderers/compose/xtmd-android": passed });
  assert.equal(r.status, 0);
  assert.match(r.out, /^::error::a\.Live\.classMethod: .*Failed to fetch/);
});

test("a test that failed is named, and nothing is tried again", () => {
  const r = run({ "live/compose": unfetched, "widgets/compose": failed });
  assert.equal(r.status, 1);
  assert.match(r.out, /::error::a\.Drag\.drags: expected 1 {3}but was 2/);
});

test("a build that failed before any test isn't tried again", () => {
  assert.equal(run({}).status, 1);
  assert.equal(run({ "widgets/compose": passed }).status, 1);
});
