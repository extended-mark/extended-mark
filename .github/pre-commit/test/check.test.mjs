// .github/pre-commit/check.mjs: the table of moved commands and flags, made from the code and held to
// VERSION, and that this repository's own table is right.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { moved, reached, removeIn, tableProblems, went, written } from "../check.mjs";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const read = (f) => readFileSync(join(repo, f), "utf8");

const commands = `
pub(crate) const MOVED: &[(&str, &str, &str)] = &[(
    "build",
    "0.4.0",
    "xtmd build moved in 0.4.0, and 0.5.0 removes it. The files an engine makes: xtmd export <files>",
)];
`;
const flags = `
const ASKED_FOR: &[(&str, &str, &str)] = &[("*", "-o", "use --out")];
pub(crate) const MOVED_FLAGS: &[(&str, &str, &str, &str)] = &[
    // a comment between them
    ("export", "--rir", "0.4.0", "xtmd compile writes the resolved IR: xtmd compile page.xtmd --rir"),
];
`;
const checks = "# Pre Commit checks\n\n<!-- the table is written by: node .github/pre-commit/check.mjs --write -->\nold\n<!-- the table ends -->\n";

test("the moved commands and flags are read from the code, and nothing the code only asks for", () => {
  assert.deepEqual(moved(commands, flags), [
    { moved: "xtmd build", release: "0.4.0", went: "The files an engine makes: xtmd export <files>" },
    { moved: "xtmd export --rir", release: "0.4.0", went: "xtmd compile writes the resolved IR: xtmd compile page.xtmd --rir" },
  ]);
});

test("what a release moves, the next minor one removes", () => {
  assert.equal(removeIn("0.4.0"), "0.5.0");
  assert.equal(removeIn("0.4.2"), "0.5.0");
  assert.equal(removeIn("1.9.0"), "1.10.0");
  assert.ok(reached("0.5.0", "0.5.0") && reached("0.5.1", "0.5.0") && reached("1.0.0", "0.5.0"));
  assert.ok(!reached("0.4.9", "0.5.0") && !reached("0.10.0", "1.0.0"));
});

test("a command in where it went is a code span, so that its <files> is shown", () => {
  assert.equal(went("makes: xtmd export <files>. runs: xtmd compile a.xtmd"), "makes: `xtmd export <files>`. runs: `xtmd compile a.xtmd`");
});

test("a table the code doesn't give fails, and writing it makes one that passes", () => {
  const entries = moved(commands, flags);
  assert.match(tableProblems(checks, entries, "0.4.0").join("\n"), /isn't the code's/);
  const now = written(checks, entries);
  assert.deepEqual(tableProblems(now, entries, "0.4.0"), []);
  assert.match(now, /\| `xtmd build` \| 0\.4\.0 \| The files an engine makes: `xtmd export <files>` \| 0\.5\.0 \|/);
  assert.equal(written(now, entries), now);
  assert.match(written(checks, []), /Nothing is moved\./);
});

test("raising VERSION to the release that removes an entry fails until it is taken out", () => {
  const entries = moved(commands, flags);
  const now = written(checks, entries);
  const due = tableProblems(now, entries, "0.5.0");
  assert.equal(due.length, 2);
  assert.match(due[0], /^xtmd build moved in 0\.4\.0, and 0\.5\.0 removes it/);
  assert.deepEqual(tableProblems(written(checks, []), [], "0.5.0"), []);
});

test("this repository's table is its code's, and nothing in it is due", () => {
  const entries = moved(read("compiler/xtmd/src/cli/mod.rs"), read("compiler/xtmd/src/cli/flags.rs"));
  assert.ok(entries.some((e) => e.moved === "xtmd build"));
  assert.deepEqual(tableProblems(read(".github/pre-commit/Pre Commit checks.md"), entries, read("VERSION").trim()), []);
});
