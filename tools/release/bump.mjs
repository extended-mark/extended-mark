// The release's version, everywhere it is written: raised in one command, and checked.
//
//   node tools/release/bump.mjs <version>    raises VERSION and every place below to <version>, and
//                                            writes notes/<version>.md to fill in when there is none
//   node tools/release/bump.mjs --check      fails, naming each place, unless all of them say VERSION
//
// The places: VERSION; each crate's version and its path dependencies' (`version = "…"`); every
// package.json of ours (its version, and the `@extended-mark/…` and `extended-mark` pins that aren't a
// `file:` path); each package-lock.json's own entries (the package, and its neighbours in this
// repository; never a registry's); the Android example's Gradle versions; the schemas' examples'
// `"xtmd"`; the runtime snapshots' release (`RELEASE`) in @extended-mark/view and the Android
// example (egui takes its crate's version); and the version the docs show (the release README, the VS
// Code extension's README, the Android example's README). A 0.3.0 that means the language's first
// release (`xtmd migrate`) is none of these.

import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { template, versionProblem } from "./check-notes.mjs";

const here = dirname(fileURLToPath(import.meta.url));

/** Every file under `root` whose name passes `want`, leaving out what is built or installed. */
function files(root, want) {
  const out = [];
  const walk = (dir) => {
    if (!existsSync(dir)) return;
    for (const name of readdirSync(dir)) {
      // a dot folder (.git, .gradle, an editor's test install) is never ours
      if (name.startsWith(".") || ["node_modules", "target", "dist", "build", "frozen"].includes(name)) continue;
      const path = join(dir, name);
      if (statSync(path).isDirectory()) walk(path);
      else if (want(name)) out.push(path);
    }
  };
  walk(root);
  return out.sort();
}

const ours = (name) => name === "extended-mark" || name.startsWith("@extended-mark/");
const SECTIONS = ["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"];

/** Each place, as a file and the edits that find its version: `read` lists what the file says now,
 * `write` gives the text with every one of them set to `v`. */
function places(repo) {
  const out = [];
  const add = (file, read, write) => out.push({ file, read, write });
  const pattern = (file, re) =>
    add(
      file,
      (t) => [...t.matchAll(re)].map((m) => m[2]),
      (t, v) => t.replace(re, (_, a, _old, b) => `${a}${v}${b}`),
    );
  add(join(repo, "VERSION"), (t) => [t.trim()], (_, v) => `${v}\n`);

  // the crates: the workspace's version, a member's own, and each path dependency's
  for (const f of files(join(repo, "compiler"), (n) => n === "Cargo.toml").concat(files(join(repo, "ide"), (n) => n === "Cargo.toml"), files(join(repo, "engines"), (n) => n === "Cargo.toml"))) {
    pattern(f, /^(version = ")([^"]+)(")/gm);
    pattern(f, /(\bpath = "[^"]+", version = ")([^"]+)(")/g);
  }

  // the workspace's lock: its own crates are the entries with no `source`
  const crate = /(\[\[package\]\]\nname = "[^"]+"\nversion = ")([^"]+)("\n(?!source))/g;
  pattern(join(repo, "compiler", "Cargo.lock"), crate);

  // the package.json files and their lock files
  for (const root of ["compiler", "engines", "apps", "ide", "examples"]) {
    for (const f of files(join(repo, root), (n) => n === "package.json")) {
      const pkg = JSON.parse(readFileSync(f, "utf8"));
      if (!ours(pkg.name ?? "") && !f.startsWith(join(repo, "examples")) && !f.startsWith(join(repo, "ide"))) continue;
      pattern(f, /^(  "version": ")([^"]+)(")/gm);
      for (const s of SECTIONS)
        for (const [dep, spec] of Object.entries(pkg[s] ?? {}))
          if (ours(dep) && !String(spec).startsWith("file:")) pattern(f, new RegExp(`("${dep.replace("/", "\\/")}": ")([^"]+)(")`, "g"));
    }
    for (const f of files(join(repo, root), (n) => n === "package-lock.json")) {
      // the package and its neighbours here, by folder; a registry's packages are left as they are
      const mine = (lock) =>
        Object.entries(lock.packages ?? {}).filter(([k, e]) => !k.includes("node_modules/") && e.version && (k === "" || existsSync(join(dirname(f), k))));
      add(
        f,
        (t) => {
          const lock = JSON.parse(t);
          return [lock.version, ...mine(lock).map(([, e]) => e.version)].filter(Boolean);
        },
        (t, v) => {
          const lock = JSON.parse(t);
          if (lock.version) lock.version = v;
          for (const [, e] of mine(lock)) e.version = v;
          return JSON.stringify(lock, null, 2) + "\n";
        },
      );
    }
  }

  // the Android example, the schemas' examples, the snapshots' release and the docs that show the version
  const android = join(repo, "engines", "examples", "misc", "android", "compose");
  for (const m of ["xtmd-android-runtime", "xtmd-android-widgets", "xtmd-android", "xtmd-android-live"]) pattern(join(android, m, "build.gradle.kts"), /(\bversion = ")([^"]+)(")/g);
  pattern(join(android, "sample", "build.gradle.kts"), /(\bversionName = ")([^"]+)(")/g);
  pattern(join(android, "README.md"), /(xtmd:xtmd-android:)(\d+\.\d+\.\d+)()/g);
  pattern(join(android, "xtmd-android", "src", "main", "kotlin", "xtmd", "android", "XtmdDoc.kt"), /(const val RELEASE = ")([^"]+)(")/g);
  for (const f of files(join(repo, "schemas", "examples"), (n) => n.endsWith(".json"))) pattern(f, /("xtmd": ")([^"]+)(")/g);
  pattern(join(repo, "engines", "published", "widgets", "src", "snapshot.ts"), /(export const RELEASE = ")([^"]+)(")/g);
  pattern(join(repo, "tools", "release", "README.md"), /(carries the release's version \()(\d+\.\d+\.\d+)(\))/g);
  pattern(join(repo, "ide", "vscode", "README.md"), /(# xtmd-)(\d+\.\d+\.\d+)(\.vsix)/g);
  return out.filter((p) => existsSync(p.file));
}

/** Where a place doesn't say `version`: [{ file, says }]. */
export function drift(repo, version) {
  const out = [];
  for (const p of places(repo)) {
    const said = p.read(readFileSync(p.file, "utf8")).filter((s) => s !== version);
    if (said.length) out.push({ file: relative(repo, p.file), says: [...new Set(said)] });
  }
  return out;
}

/** Every place set to `version`; the files it changed. */
export function bump(repo, version) {
  const changed = [];
  for (const p of places(repo)) {
    const t = readFileSync(p.file, "utf8");
    const n = p.write(t, version);
    if (n !== t) {
      writeFileSync(p.file, n);
      changed.push(relative(repo, p.file));
    }
  }
  return [...new Set(changed)];
}

function main(argv) {
  const repo = join(here, "..", "..");
  if (argv[0] === "--check") {
    const version = readFileSync(join(repo, "VERSION"), "utf8").trim();
    const off = drift(repo, version);
    for (const d of off) console.error(`${d.file}: says ${d.says.join(", ")}, not ${version}`);
    if (off.length) {
      console.error(`${off.length} file${off.length > 1 ? "s" : ""} not at VERSION ${version}: node tools/release/bump.mjs ${version}`);
      return 1;
    }
    console.log(`every version says ${version}`);
    return 0;
  }
  const version = argv[0];
  const problem = version && versionProblem(version);
  if (!version || argv.length !== 1 || problem) {
    console.error(problem ? `${version}: ${problem}` : "usage: node tools/release/bump.mjs <version> | --check");
    return 2;
  }
  const changed = bump(repo, version);
  console.log(`${changed.length} files set to ${version}`);
  const note = join(repo, "notes", `${version}.md`);
  if (!existsSync(note)) {
    writeFileSync(note, template(version));
    console.log(`notes/${version}.md written: fill it in`);
  }
  return 0;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) process.exit(main(process.argv.slice(2)));
