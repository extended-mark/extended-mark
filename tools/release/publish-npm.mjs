// Publishes Extended Mark's npm packages (packages.mjs), each after the ones it depends on: @extended-mark/runtime
// first, the framework bindings last. A package's `file:` dependencies on its neighbours become
// the release's version for the publish, and its package.json is put back after.
//
//   node tools/release/publish-npm.mjs --version 0.3.0 [--stage] [--dry-run]
//
// Every package is built first (npm run build), and each @extended-mark/cli-<os>-<cpu> holds its binary in
// bin/ (the release workflow puts them there); one without a binary isn't published. Publishing
// needs npm trusted publishing (the workflow's OIDC token) or NPM_TOKEN in ~/.npmrc, as the workflow
// writes it; --dry-run needs nothing.
//
// --stage stages each package instead (`npm stage publish`, npm 11.15.0 or later), for a release
// with no trusted publishing yet, such as a package's first: a token can stage without 2FA, and each
// version waits on npmjs.com until a maintainer approves it. Either way, a version already staged or
// published is passed over, so the run can be repeated.

import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { packages, repo } from "./packages.mjs";

const arg = (name) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : undefined;
};
const version = arg("--version");
const dry = process.argv.includes("--dry-run");
const stage = process.argv.includes("--stage");
if (!version) {
  console.error("usage: node tools/release/publish-npm.mjs --version <x.y.z> [--stage] [--dry-run]");
  process.exit(2);
}

const SECTIONS = ["dependencies", "optionalDependencies", "peerDependencies", "devDependencies"];
const order = packages();

/** npm, its output captured and shown as well. */
function npm(args, cwd) {
  const r = spawnSync("npm", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  return { ok: r.status === 0, out: r.stdout ?? "", err: r.stderr ?? "" };
}

/** "published" or "staged" when name@version is on the registry already, either way. */
function onRegistry(name) {
  const view = npm(["view", `${name}@${version}`, "version"], repo);
  if (view.ok && view.out.trim() === version) return "published";
  // listing what is staged needs the token; without it (a dry run) the stage itself says
  const list = npm(["stage", "list", name, "--json"], repo);
  if (list.ok) {
    try {
      if (JSON.parse(list.out).some((s) => s.version === version && s.status !== "rejected")) return "staged";
    } catch {}
  }
  return null;
}

let failed = 0;
const staged = [];
let passed = 0;
for (const p of order) {
  const where = relative(repo, p.dir);
  if (p.pkg.version !== version) {
    console.error(`${p.pkg.name} (${where}) is ${p.pkg.version}, not ${version}: raise it first`);
    failed++;
    continue;
  }
  if (p.pkg.os && !existsSync(join(p.dir, "bin"))) {
    console.log(`${p.pkg.name}: no binary in ${where}/bin, not published`);
    continue;
  }
  // a version already out (or staged) is passed over, so a release run again after a failure only
  // finishes what's left
  const already = onRegistry(p.pkg.name);
  if (already) {
    console.log(`${p.pkg.name}@${version} is already ${already}: passed over`);
    passed++;
    continue;
  }
  // the neighbours it names by folder are named by the release's version
  const out = JSON.parse(p.text);
  for (const section of SECTIONS) {
    for (const [dep, spec] of Object.entries(out[section] ?? {})) {
      if (typeof spec === "string" && spec.startsWith("file:")) out[section][dep] = version;
    }
  }
  writeFileSync(p.file, JSON.stringify(out, null, 2) + "\n");
  try {
    if (stage) {
      const r = npm(["stage", "publish", "--access", "public", ...(dry ? ["--dry-run"] : [])], p.dir);
      process.stdout.write(r.out);
      process.stderr.write(r.err);
      if (r.ok) {
        const id = /\(staged with id ([^)\s]+)\)/.exec(r.out)?.[1];
        staged.push(`${p.pkg.name}@${version}${id ? ` (id ${id})` : ""}`);
        console.log(`${dry ? "would stage" : "staged"} ${p.pkg.name}@${version}`);
      } else if (/previously published|already (been )?staged|already exists|E409/i.test(r.err)) {
        // staged or published while this ran, or by a run before it whose list this token can't read
        console.log(`${p.pkg.name}@${version} is already on the registry: passed over`);
        passed++;
      } else {
        console.error(`${p.pkg.name}: npm stage publish failed`);
        failed++;
      }
    } else {
      execFileSync("npm", ["publish", "--access", "public", ...(dry ? ["--dry-run"] : [])], { cwd: p.dir, stdio: ["ignore", "inherit", "inherit"] });
      console.log(`${dry ? "would publish" : "published"} ${p.pkg.name}@${version}`);
    }
  } catch {
    console.error(`${p.pkg.name}: npm publish failed`);
    failed++;
  } finally {
    writeFileSync(p.file, p.text);
  }
}
if (stage) {
  console.log(`\n${staged.length} ${dry ? "would be staged" : "staged"}, ${passed} already on the registry${failed ? `, ${failed} failed` : ""}`);
  for (const s of staged) console.log(`  ${s}`);
  console.log("approve them on npmjs.com → Staged Packages (up to 100 at once), or npm stage approve <id>");
}
process.exit(failed ? 1 : 0);
