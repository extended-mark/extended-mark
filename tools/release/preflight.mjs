// Before a merge into main: can every registry take the release VERSION names? A release that fails on
// main can't be taken back (a version a registry has is its for good), so what can be known beforehand is
// checked here, on the merge request, by the "Release dry run" job (.github/workflows/ci.yml):
//
//   - npm: an unscoped name (extended-mark) is ours on npm already; npm refuses a name too like another
//     (it refused `xtmd`) and says so only when the name is first published, so a new one is published by
//     hand first. NPM_TOKEN works and may write every package, unless trusted publishing is set up, in
//     which case every package exists already (npm sets trusted publishing up per package).
//   - crates.io: under trusted publishing, every crate exists already; otherwise a token is there.
//   - Open VSX: the namespace exists, and OVSX_PAT works when it is what publishes.
//   - each registry: what it will do (publish, stage, pass over a version it has, or skip for want of a
//     credential), in the job's summary.
//   - xtmd migrate: the release moves a tree written for the release right before it, so it holds that
//     release's front end (compiler/xtmd/previous/, from its tag), its step, on, and a corpus of that
//     release's files (checks/migrate-<x.y>/), which the Rust tests migrate.
//
// The job then dry-runs each publish (publish-npm.mjs, publish-crates.sh, the .vsix). Nothing is published.
//
//   node tools/release/preflight.mjs
//
// Reads, from the environment: NODE_AUTH_TOKEN (NPM_TOKEN), NPM_TRUSTED, CARGO_REGISTRY_TOKEN,
// CRATES_TRUSTED, VSCE_TRUSTED, OVSX_TRUSTED, OVSX_PAT and GH_TOKEN; each is optional, and none is printed.

import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { packages, repo } from "./packages.mjs";

/** What npm's answers say about the packages: an unscoped name npm has never seen can't be released
 * (npm may refuse it, and says so only at its first publish), a new scoped one is staged by the token,
 * and under trusted publishing (set up per package) none may be new. `seen` maps each name to npm's
 * status for it and the versions it has. */
export function npmFindings(names, seen, { version, trusted }) {
  const errors = [], notes = [], fresh = [];
  for (const name of names) {
    const { status, versions } = seen[name];
    if (status === 404) {
      if (!name.startsWith("@")) errors.push(`${name}: npm has no package of this name. npm refuses a name too like another's, and says so only at its first publish: publish ${name} by hand once (npm publish --access public), or rename it`);
      else if (trusted) errors.push(`${name} is new: trusted publishing is set up per package, so its first release needs NPM_TOKEN (set NPM_TRUSTED_PUBLISHING to false for it)`);
      else fresh.push(name);
    } else if (status !== 200) errors.push(`${name}: npm answered ${status}`);
    else if (versions?.[version]) notes.push(`${name}@${version} is on npm already: passed over`);
  }
  return { errors, notes, fresh };
}

/** Whether an npm token (an entry of `npm token list --json`) may write every package, and doesn't
 * run out within two days of `now`. */
export function tokenFindings(token, names, now = Date.now()) {
  const errors = [];
  if (!(token.permissions ?? []).some((p) => p.name === "package" && p.action === "write")) return { errors: ["NPM_TOKEN can't write packages: give it Read and write on packages"], ok: null };
  const scopes = (token.scopes ?? []).filter((s) => s.type === "package");
  const all = scopes.some((s) => s.name == null);
  const covers = (name) => all || scopes.some((s) => s.name === name || (s.name?.startsWith("@") && name.startsWith(`${s.name}/`)));
  const out = names.filter((n) => !covers(n));
  if (out.length) errors.push(`NPM_TOKEN may not write ${out.join(", ")}: give it Read and write on all packages`);
  if (token.expiry && Date.parse(token.expiry) < now + 2 * 86400e3) errors.push(`NPM_TOKEN expires ${token.expiry.slice(0, 10)}: make a new one before the release`);
  return { errors, ok: errors.length ? null : `NPM_TOKEN may write all ${names.length} packages${token.expiry ? `; it expires ${token.expiry.slice(0, 10)}` : ""}` };
}

/** The tag of the release right before `version` among `tags` (`v0.3.2`), or null. */
export function releaseBefore(version, tags) {
  const parse = (v) => v.replace(/^v/, "").split(".").map(Number);
  const less = (a, b) => a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
  const now = parse(version);
  const before = tags.filter((t) => /^v\d+\.\d+\.\d+$/.test(t) && less(parse(t), now) < 0).sort((a, b) => less(parse(a), parse(b)));
  return before.at(-1) ?? null;
}

/** Whether `version` can move a tree written for the release before it, from what the tree holds: the
 * tag compiler/xtmd/previous/ was copied from (`copied`), the release the readers read (`reads`), whether
 * there is a step and it is on (`step`, `ready`), and the corpus folders of checks/ (`corpora`). */
export function migrationFindings(version, tags, { copied, reads, step, ready, corpora }) {
  const errors = [];
  const before = releaseBefore(version, tags);
  if (!before) return { errors, ok: `${version} is the first release: there is nothing to migrate from` };
  const plain = before.slice(1);
  if (copied !== before) errors.push(`compiler/xtmd/previous/ ${copied ? `is ${copied}'s front end` : "holds no front end"}, not ${before}'s: run node tools/release/previous.mjs ${before}`);
  if (reads !== plain) errors.push(`readers::PREVIOUS (compiler/xtmd/src/migrate/readers/mod.rs) is "${reads}", not "${plain}"`);
  if (!step) errors.push(`there is no step from ${plain} (compiler/xtmd/src/migrate/rir/mod.rs STEP): a tree written for it can't be moved`);
  else if (!ready) errors.push(`the step from ${plain} isn't on (rir::READY): xtmd migrate wouldn't take it`);
  const corpus = `migrate-${plain.split(".").slice(0, 2).join(".")}`;
  if (!corpora.includes(corpus)) errors.push(`checks/${corpus}/ has no corpus of ${plain}'s files: the step is tested on none`);
  return { errors, ok: errors.length ? null : `a tree written for ${plain} moves: its front end, its step, on, and checks/${corpus}/` };
}

/** What migrationFindings reads, from this repository. */
export function migrationTree(root = repo) {
  const read = (p) => (existsSync(join(root, p)) ? readFileSync(join(root, p), "utf8") : "");
  const rir = read("compiler/xtmd/src/migrate/rir/mod.rs");
  return {
    copied: read("compiler/xtmd/previous/mod.rs").match(/pub const TAG: &str = "([^"]+)"/)?.[1] ?? null,
    reads: read("compiler/xtmd/src/migrate/readers/mod.rs").match(/pub const PREVIOUS: &str = "([^"]+)"/)?.[1] ?? null,
    step: /pub static STEP: Option<Step> = Some\(/.test(rir),
    ready: /pub const READY: bool = true;/.test(rir),
    corpora: existsSync(join(root, "checks")) ? readdirSync(join(root, "checks")).filter((d) => existsSync(join(root, "checks", d, "expected.tsv"))) : [],
  };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) await main();

async function main() {
  const env = process.env;
  const version = readFileSync(join(repo, "VERSION"), "utf8").trim();
  const errors = [];
  const lines = [];
  const say = (line) => {
    lines.push(line);
    console.log(line);
  };
  const fail = (line) => {
    errors.push(line);
    say(`- ✗ ${line}`);
  };
  const ok = (line) => say(`- ✓ ${line}`);
  const note = (line) => say(`- · ${line}`);
  const run = (cmd, args, opts = {}) => {
    const r = spawnSync(cmd, args, { encoding: "utf8", cwd: repo, stdio: ["ignore", "pipe", "pipe"], ...opts });
    return { ok: r.status === 0, out: r.stdout ?? "", err: r.stderr ?? "" };
  };
  const agent = "extended-mark release preflight (https://github.com/extended-mark/extended-mark)";
  async function get(url, headers = {}) {
    for (let tries = 1; ; tries++) {
      try {
        const r = await fetch(url, { headers: { "user-agent": agent, ...headers } });
        if (r.status >= 500 && tries < 4) throw new Error(`${r.status}`);
        return { status: r.status, json: r.status === 200 ? await r.json() : null };
      } catch (e) {
        if (tries >= 4) throw new Error(`${url}: ${e.message}`);
        await new Promise((ok) => setTimeout(ok, 5000 * tries));
      }
    }
  }

  say(`# Release preflight: ${version}\n`);

  // the GitHub release: a version released already has nothing left to check
  const tag = `v${version}`;
  const released = run("git", ["ls-remote", "--exit-code", "--tags", "origin", `refs/tags/${tag}`]).ok || (env.GH_TOKEN && run("gh", ["release", "view", tag]).ok);
  if (released) {
    say(`${tag} is released already: merging releases nothing, so there is nothing to check.`);
    if (env.GITHUB_STEP_SUMMARY) appendFileSync(env.GITHUB_STEP_SUMMARY, lines.join("\n") + "\n");
    process.exit(0);
  }

  // ---- xtmd migrate: the release before's trees move to this one
  say("\n## xtmd migrate");
  const tags = run("git", ["ls-remote", "--tags", "origin"]).out.split("\n").map((l) => l.split("refs/tags/")[1]).filter((t) => t && !t.endsWith("^{}"));
  const moves = migrationFindings(version, tags, migrationTree());
  moves.errors.forEach(fail);
  if (moves.ok) ok(moves.ok);

  // ---- npm
  say("\n## npm");
  const npmTrusted = env.NPM_TRUSTED === "true";
  const npmToken = !!env.NODE_AUTH_TOKEN;
  const pkgs = packages().map((p) => p.pkg.name);
  const seen = {};
  for (const name of pkgs) {
    const { status, json } = await get(`https://registry.npmjs.org/${name.replace("/", "%2f")}`);
    seen[name] = { status, versions: json?.versions };
  }
  const found = npmFindings(pkgs, seen, { version, trusted: npmTrusted });
  found.errors.forEach(fail);
  found.notes.forEach(note);
  const fresh = found.fresh;
  if (fresh.length) note(`new on npm, staged by the token: ${fresh.join(", ")}`);
  if (npmTrusted) ok(`trusted publishing publishes the ${pkgs.length} packages`);
  else if (!npmToken) note("no trusted publishing and no NPM_TOKEN: npm is skipped");
  else {
    const userconfig = join(env.RUNNER_TEMP ?? "/tmp", "preflight.npmrc");
    // the token by its variable, never written out
    spawnSync("sh", ["-c", `printf '//registry.npmjs.org/:_authToken=\${NODE_AUTH_TOKEN}\\n' > "${userconfig}"`]);
    const npm = (...args) => run("npm", [...args, "--userconfig", userconfig]);
    const who = npm("whoami");
    if (!who.ok) fail(`NPM_TOKEN doesn't sign in to npm (${who.err.split("\n").find((l) => /error/.test(l)) ?? "npm whoami failed"})`);
    else {
      ok(`NPM_TOKEN signs in as ${who.out.trim()}`);
      // the token in use is the one npm saw last: what may it write?
      const list = npm("token", "list", "--json");
      let tokens = [];
      try {
        tokens = JSON.parse(list.out);
      } catch {}
      const mine = [...tokens].sort((a, b) => String(b.accessed ?? "").localeCompare(String(a.accessed ?? "")))[0];
      if (!mine) note("NPM_TOKEN's permissions couldn't be read: not checked");
      else {
        const t = tokenFindings(mine, pkgs);
        t.errors.forEach(fail);
        if (t.ok) ok(t.ok);
      }
    }
  }

  // ---- crates.io
  say("\n## crates.io");
  const cratesTrusted = env.CRATES_TRUSTED === "true";
  const meta = run("cargo", ["metadata", "--no-deps", "--format-version", "1"], { cwd: join(repo, "compiler") });
  if (!meta.ok) fail(`cargo metadata failed: ${meta.err.trim().split("\n").pop()}`);
  else {
    const crates = JSON.parse(meta.out).packages.filter((p) => !(Array.isArray(p.publish) && p.publish.length === 0));
    for (const c of crates) {
      if (c.version !== version) fail(`${c.name} is ${c.version}, not ${version}`);
      const { status, json } = await get(`https://crates.io/api/v1/crates/${c.name}`);
      if (status === 404 && cratesTrusted) fail(`${c.name} is new: trusted publishing is set up per crate, so its first release needs CARGO_REGISTRY_TOKEN`);
      else if (status === 200 && json.versions?.some((v) => v.num === version)) note(`${c.name} ${version} is on crates.io already: left out`);
      else if (status !== 200 && status !== 404) fail(`${c.name}: crates.io answered ${status}`);
    }
    if (cratesTrusted) ok(`trusted publishing publishes the ${crates.length} crates`);
    else if (env.CARGO_REGISTRY_TOKEN) ok(`CARGO_REGISTRY_TOKEN publishes the ${crates.length} crates`);
    else note("no trusted publishing and no CARGO_REGISTRY_TOKEN: crates.io is skipped");
  }

  // ---- the VS Code extension
  const ext = JSON.parse(readFileSync(join(repo, "ide", "vscode", "package.json"), "utf8"));
  if (ext.version !== version) fail(`the VS Code extension is ${ext.version}, not ${version}`);
  say("\n## VS Code Marketplace");
  if (env.VSCE_TRUSTED === "true") ok(`trusted publishing publishes ${ext.publisher}.${ext.name}`);
  else note("no trusted publishing (VSCE_TRUSTED_PUBLISHING): the Marketplace is skipped");
  say("\n## Open VSX");
  const ns = await get(`https://open-vsx.org/api/${ext.publisher}`);
  if (ns.status !== 200) fail(`Open VSX has no namespace ${ext.publisher}: create it (npx ovsx create-namespace ${ext.publisher})`);
  else {
    const has = await get(`https://open-vsx.org/api/${ext.publisher}/${ext.name}/${version}`);
    if (has.status === 200) note(`${ext.publisher}.${ext.name} ${version} is on Open VSX already: passed over`);
    if (env.OVSX_TRUSTED === "true") ok(`trusted publishing publishes ${ext.publisher}.${ext.name}`);
    else if (env.OVSX_PAT) {
      // the token by its variable, as the release passes it
      const v = run("sh", ["-c", `npx --yes ovsx verify-pat ${ext.publisher} -p "$OVSX_PAT"`], { cwd: join(repo, "ide", "vscode") });
      if (v.ok) ok(`OVSX_PAT may publish to ${ext.publisher}`);
      else fail(`OVSX_PAT can't publish to ${ext.publisher}: ${(v.err || v.out).trim().split("\n").pop()}`);
    } else note("no trusted publishing and no OVSX_PAT: Open VSX is skipped");
  }

  say(errors.length ? `\n**${errors.length} problem${errors.length > 1 ? "s" : ""}: a merge into main would fail to release ${version}.**` : `\nEvery registry can take ${version}.`);
  if (env.GITHUB_STEP_SUMMARY) appendFileSync(env.GITHUB_STEP_SUMMARY, lines.join("\n") + "\n");
  process.exit(errors.length ? 1 : 0);
}
