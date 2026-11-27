// Installs and builds Extended Mark's npm packages (compiler/npm, engines/, then the playground) in order, each after
// what it uses: npm links a neighbour named by `file:`, so it is built first. A framework a
// neighbour expects of its user (its peerDependencies: React, Vue, Solid, Svelte, Angular) is then
// one copy for both, the neighbour's, so a binding and its live twin run one React. --test then
// runs every package's tests.
//
//   node tools/release/build-ts.mjs [--test | --test-only] [--only <folders> | --except <folders>]
//
// --test-only runs the tests of packages already built, as the release does once a browser is
// installed for the ones that need it. --only and --except (folders, comma-separated, as
// `runtime,engines/web`) test some of the packages and build them all: CI tests them on several
// machines at once, each naming its folders, and one taking every other, so none is left out.

import { execFileSync } from "node:child_process";
import { existsSync, lstatSync, rmSync, symlinkSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { packages, repo } from "./packages.mjs";

const only = process.argv.includes("--test-only");
const test = only || process.argv.includes("--test");
/** The folders after a flag, or undefined without it. */
const folders = (flag) => {
  const at = process.argv.indexOf(flag);
  return at < 0 ? undefined : (process.argv[at + 1] ?? "").split(",").map((f) => f.trim().replace(/\/+$/, "")).filter(Boolean);
};
const [chosen, left] = [folders("--only"), folders("--except")];
const inside = (p, fs) => fs.some((f) => relative(repo, p.dir) === f || relative(repo, p.dir).startsWith(`${f}/`));
const tested = (p) => (!chosen || inside(p, chosen)) && !(left && inside(p, left));
const run = (cwd, args) => execFileSync("npm", args, { cwd, stdio: ["ignore", "inherit", "inherit"] });
// every package with something to install, build or test
const all = packages({ published: false }).filter((p) => p.pkg.scripts?.build || p.pkg.scripts?.test || Object.keys({ ...p.pkg.dependencies, ...p.pkg.devDependencies }).length);
const byName = new Map(packages({ published: false }).map((p) => [p.pkg.name, p]));
// --list: the folders of the packages it would test, and nothing else done
if (process.argv.includes("--list")) {
  for (const p of all.filter((p) => p.pkg.scripts?.test && tested(p))) console.log(relative(repo, p.dir));
  process.exit(0);
}

/** Each framework a neighbour expects of `p`, installed in both, becomes a link to the
 * neighbour's: two copies of React in one page don't share hooks. */
function shareFrameworks(p) {
  for (const dep of Object.keys(p.pkg.dependencies ?? {})) {
    const n = byName.get(dep);
    for (const peer of Object.keys(n?.pkg.peerDependencies ?? {})) {
      const mine = join(p.dir, "node_modules", peer);
      const theirs = join(n.dir, "node_modules", peer);
      if (!existsSync(theirs) || !existsSync(mine) || lstatSync(mine).isSymbolicLink()) continue;
      rmSync(mine, { recursive: true, force: true });
      symlinkSync(relative(dirname(mine), theirs), mine, "dir");
    }
  }
}
// `npm install`, not `npm ci`: a lock file names its neighbours by folder, and one that has fallen
// behind its package.json (a neighbour added since) would stop `ci`
for (const p of only ? [] : all) {
  console.log(`== ${p.pkg.name} (${relative(repo, p.dir)}): install, build`);
  run(p.dir, ["install", "--no-audit", "--no-fund"]);
  shareFrameworks(p);
  if (p.pkg.scripts?.build) run(p.dir, ["run", "build"]);
}
if (test) {
  // every package's tests, then the ones that failed
  const failed = [];
  const testing = all.filter((p) => p.pkg.scripts?.test && tested(p));
  // a folder that names no package is a mistake in the list: nothing of it would be tested
  const none = (chosen ?? []).filter((f) => !all.some((p) => p.pkg.scripts?.test && inside(p, [f])));
  if (none.length) {
    console.error(`--only names folders with no package to test: ${none.join(", ")}`);
    process.exit(1);
  }
  for (const p of testing) {
    console.log(`== ${p.pkg.name} (${relative(repo, p.dir)}): test`);
    try {
      run(p.dir, ["test"]);
    } catch {
      failed.push(p.pkg.name);
    }
  }
  if (failed.length) {
    console.error(`failed: ${failed.join(", ")}`);
    process.exit(1);
  }
}
