// The checks of "Pre Commit checks.md" that a program can make, in one command, before a commit.
//
//   node .github/pre-commit/check.mjs           runs them all, and fails naming each one that fails
//   node .github/pre-commit/check.mjs --write   writes the table of moved commands and flags from the code
//
// They are: the notes for agents (tools/llm/check.mjs); every version the same (tools/release/bump.mjs
// --check); core formatted (xtmd fmt --check core, with the xtmd of compiler/target/release, or
// XTMD_BINARY); what moves a tree written for the release before VERSION there (the release preflight's
// migrationFindings, with the tags this clone has); and the table of moved commands and flags, held to
// the code and to VERSION.
//
// The table is made from `MOVED` (compiler/xtmd/src/cli/mod.rs) and `MOVED_FLAGS` (flags.rs). An entry is
// removed in the release after the one that moved it: the next minor one, 0.4.0's in 0.5.0. The check
// fails when the table isn't what the code gives, or when VERSION has reached an entry's "Remove in" and
// the entry is still there: raising the version is what says it is time.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { migrationFindings, migrationTree } from "../../tools/release/preflight.mjs";

const CHECKS = ".github/pre-commit/Pre Commit checks.md";
const COMMANDS = "compiler/xtmd/src/cli/mod.rs";
const FLAGS = "compiler/xtmd/src/cli/flags.rs";
const START = "<!-- the table is written by: node .github/pre-commit/check.mjs --write -->";
const END = "<!-- the table ends -->";

/** The string literals of a list of tuples in Rust, a tuple each. */
function tuples(rust, name, file) {
  const at = rust.indexOf(`const ${name}:`);
  if (at < 0) throw new Error(`${file} has no const ${name}`);
  const body = rust.slice(rust.indexOf("= &[", at), rust.indexOf("];", at));
  const str = /"((?:[^"\\]|\\.)*)"/g;
  return [...body.matchAll(/\(\s*("(?:[^"\\]|\\.)*"\s*,\s*)+("(?:[^"\\]|\\.)*")\s*,?\s*\)/g)].map((m) =>
    [...m[0].matchAll(str)].map((s) => s[1].replace(/\\"/g, '"')),
  );
}

/** Each moved command and flag in the code: what it was, the release that moved it, and where it went. */
export function moved(commands, flags) {
  const cmds = tuples(commands, "MOVED", COMMANDS).map(([cmd, release, went]) => ({
    moved: `xtmd ${cmd}`,
    release,
    // the message opens with what moved and when; the table has columns for those
    went: went.replace(/^.*? removes it\. /, ""),
  }));
  const fl = tuples(flags, "MOVED_FLAGS", FLAGS).map(([cmd, flag, release, went]) => ({ moved: `xtmd ${cmd} ${flag}`, release, went }));
  return [...cmds, ...fl];
}

/** The release that removes what `release` moved: the next minor one. */
export function removeIn(release) {
  const [major, minor] = release.split(".").map(Number);
  return `${major}.${minor + 1}.0`;
}

/** Whether release `a` is `b` or after it. */
export function reached(a, b) {
  const [x, y] = [a, b].map((v) => v.split("-")[0].split(".").map(Number));
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] > y[i];
  return true;
}

/** Where it went, its commands in code spans, so that a `<files>` in one is shown as written. */
export function went(text) {
  return text.replace(/\|/g, "\\|").replace(/: (xtmd .*?)(?=\. |$)/g, ": `$1`");
}

/** The table, as the checklist holds it. */
export function table(entries) {
  const rows = entries.map((e) => `| \`${e.moved}\` | ${e.release} | ${went(e.went)} | ${removeIn(e.release)} |`);
  return ["| Moved | In | Where it went | Remove in |", "|---|---|---|---|", ...rows].join("\n");
}

/** The checklist with its table written anew. */
export function written(checks, entries) {
  const [a, b] = [checks.indexOf(START), checks.indexOf(END)];
  if (a < 0 || b < a) throw new Error(`${CHECKS} has no ${START} … ${END}`);
  return `${checks.slice(0, a + START.length)}\n${entries.length ? table(entries) : "Nothing is moved."}\n${checks.slice(b)}`;
}

/** What is wrong with the table: not the code's, or an entry still there in the release that removes it. */
export function tableProblems(checks, entries, version) {
  const out = [];
  if (written(checks, entries) !== checks) out.push(`${CHECKS}: its table isn't the code's: node .github/pre-commit/check.mjs --write`);
  for (const e of entries) {
    if (reached(version, removeIn(e.release))) {
      out.push(`${e.moved} moved in ${e.release}, and ${version} removes it: take out its entry, its message and its tests, and say so in notes/${version}.md`);
    }
  }
  return out;
}

/** A check that is a command of its own: nothing, or why it fails. */
function run(repo, what, cmd, args) {
  try {
    execFileSync(cmd, args, { cwd: repo, stdio: ["ignore", "pipe", "pipe"], encoding: "utf8" });
    return [];
  } catch (e) {
    return [`${what} fails:\n${`${e.stdout ?? ""}${e.stderr ?? ""}`.trim() || e.message}`];
  }
}

function main() {
  const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
  const read = (f) => readFileSync(join(repo, f), "utf8");
  const entries = moved(read(COMMANDS), read(FLAGS));
  if (process.argv.includes("--write")) {
    writeFileSync(join(repo, CHECKS), written(read(CHECKS), entries));
    console.log(`${CHECKS}: ${entries.length} moved`);
    return;
  }
  const xtmd = process.env.XTMD_BINARY || join(repo, "compiler/target/release/xtmd");
  const problems = [
    ...tableProblems(read(CHECKS), entries, read("VERSION").trim()),
    ...run(repo, "node tools/llm/check.mjs", process.execPath, ["tools/llm/check.mjs"]),
    ...run(repo, "node tools/release/bump.mjs --check", process.execPath, ["tools/release/bump.mjs", "--check"]),
    ...migrationFindings(read("VERSION").trim(), execFileSync("git", ["tag", "--list"], { cwd: repo, encoding: "utf8" }).split("\n").filter(Boolean), migrationTree(repo)).errors,
    ...(existsSync(xtmd)
      ? run(repo, "xtmd fmt --check core", xtmd, ["fmt", "--check", "core"])
      : [`xtmd fmt --check core didn't run: there is no ${xtmd}; cargo build --release -p xtmd, or set XTMD_BINARY`]),
  ];
  for (const p of problems) console.error(`error: ${p}`);
  if (problems.length) process.exit(1);
  console.log(`the pre-commit checks pass; ${entries.length} moved, none due before ${removeIn(entries[0]?.release ?? read("VERSION").trim())}`);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) main();
