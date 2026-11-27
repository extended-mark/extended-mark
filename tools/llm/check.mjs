// Holds CLAUDE.md, .index/ and .knowledge/ (what an agent reads to find its way) to the repository.
//
//   node tools/llm/check.mjs [folder]
//
// It fails, naming the file and line, unless:
//
// - every path they name is a file or folder of git's, or one git would take. A path is from the
//   repository's root; in a file of .index/ it may also be from a folder that file named before it, and
//   in .knowledge/ a name alone (`page.rs`) is any file of that name;
// - everything at the repository's root, and the folder of every crate and package, is named in .index/;
// - both folders hold .txt files only, and .index/00-OVERVIEW.txt lists each other file of .index/;
// - notes are small: at most 30 lines for one in .knowledge/, and 110 characters a line in both.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const OVERVIEW = ".index/00-OVERVIEW.txt";
const LINES = 30;
const WIDTH = 110;
/** A name that ends like a file of ours is one, with no folder before it too. */
const FILE = /\.(rs|ts|tsx|mjs|js|kt|json|xtmd|xtmdpack|md|txt|toml|yml|css|sh|ps1|svelte|html|py|svg)$/;
/** Built, so never git's. */
const BUILT = /(^|\/)(dist|target|node_modules)(\/|$)|\.(ir\.json|wasm)$/;
/** An ending, with no name before it. */
const ENDING = /^\.(rs|ts|json|xtmd|xtmdpack|md|txt)$/;
/** Not a path of this repository's: what a tree of somebody else's has, a library, a widget or value of core's. */
const OTHER = /^(xtmd\.toml|surfaces\/.*|migrations\/.*|decimal\.js|Next\.js|core\/[a-z_]+)$/;

/** Every file git has, or would take: a note and what it names may come in one commit. */
export function tracked(repo) {
  const out = execFileSync("git", ["ls-files", "-z", "--cached", "--others", "--exclude-standard"], { cwd: repo, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  return [...new Set(out.split("\0"))].filter((f) => f && existsSync(join(repo, f)));
}

/** The words of a line that may be paths. */
export function pathsIn(line) {
  const out = [];
  // a name with spaces in it is named in double quotes: "Pre Commit checks.md"
  line = line.replace(/"([\w.-]+(?: [\w.-]+)+\.\w+)"/g, (_, name) => {
    out.push(name);
    return " ";
  });
  for (const raw of line.split(/[\s`"'()[\],;|=]+/)) {
    const word = raw.replace(/[.:]+$/, "");
    // a placeholder (<name>), an address, a package's name, a flag and an ending (.xtmdpack) are none
    if (!/^\.?[\w*][\w.*-]*(\/[\w.*@-]+)*\/?$/.test(word) || ENDING.test(word)) continue;
    if (word.includes("/") || FILE.test(word) || word === "VERSION") out.push(word.replace(/\/$/, ""));
  }
  return out;
}

/** What is wrong, as `file:line: why`. `files` are git's; `read` gives one's text. */
export function problems(files, read) {
  const out = [];
  const isFile = new Set(files);
  const all = new Set(files);
  for (const f of files) for (let d = dirname(f); d !== "."; d = dirname(d)) all.add(d);
  const tops = new Set(files.map((f) => f.split("/")[0]));
  const names = new Set(files.map((f) => f.split("/").pop()));
  const escaped = (p) => p.replace(/[.+?^${}()|[\]\\]/g, "\\$&").replaceAll("*", "[^/]*");
  /** The paths of ours a name stands for, from a folder or the root. */
  const from = (base, path) => {
    const whole = base ? `${base}/${path}` : path;
    if (!whole.includes("*")) return all.has(whole) ? [whole] : [];
    const p = new RegExp(`^${escaped(whole)}$`);
    return [...all].filter((x) => p.test(x));
  };

  const named = new Set();
  const notes = files.filter((f) => f.startsWith(".index/") || f.startsWith(".knowledge/"));
  for (const f of ["CLAUDE.md", ...notes]) {
    if (!all.has(f)) {
      out.push(`${f}: isn't there`);
      continue;
    }
    const inIndex = f.startsWith(".index/");
    if (f !== "CLAUDE.md" && !f.endsWith(".txt")) out.push(`${f}: only .txt files go here`);
    const lines = read(f).replace(/\n$/, "").split("\n");
    if (f.startsWith(".knowledge/") && lines.length > LINES) out.push(`${f}: ${lines.length} lines, over ${LINES}: a note is one subject`);
    const bases = new Set([dirname(f)]);
    lines.forEach((line, k) => {
      if (f !== "CLAUDE.md" && line.length > WIDTH) out.push(`${f}:${k + 1}: over ${WIDTH} characters`);
      for (const path of pathsIn(line)) {
        if (BUILT.test(path) || OTHER.test(path)) continue;
        let found = from("", path);
        if (!found.length && inIndex) found = [...bases].flatMap((b) => from(b, path));
        // a file's name, for one somewhere under a folder named before it
        if (!found.length && inIndex && FILE.test(path)) {
          const end = new RegExp(`/${escaped(path)}$`);
          found = files.filter((x) => end.test(x) && [...bases].some((b) => x.startsWith(`${b}/`)));
        }
        if (found.length) {
          if (inIndex) for (const x of found) named.add(x);
          // a folder, or the folder a file is in, is where the paths after it may be from
          for (const x of found) bases.add(isFile.has(x) ? dirname(x) : x);
          continue;
        }
        if (!inIndex && !path.includes("/") && names.has(path)) continue;
        // words with a slash between them (`tap/click`) are no path, unless a folder of ours starts them
        if (FILE.test(path) || tops.has(path.split("/")[0])) out.push(`${f}:${k + 1}: there is no ${path}`);
      }
    });
  }

  if (!all.has(OVERVIEW)) out.push(`${OVERVIEW}: isn't there`);
  else for (const f of notes) if (f.startsWith(".index/") && f !== OVERVIEW && !read(OVERVIEW).includes(f.slice(7))) out.push(`${OVERVIEW}: doesn't list ${f}`);
  const said = [...named];
  for (const t of tops) if (t !== "CLAUDE.md" && !/^\.(index|knowledge|git|docker)/.test(t) && !said.some((n) => n === t || n.startsWith(`${t}/`))) out.push(`.index: says nothing of ${t}`);
  for (const f of files) {
    if (!/(^|\/)(Cargo\.toml|package\.json)$/.test(f) || /(^|\/)(test|platforms)\//.test(f)) continue;
    const home = dirname(f);
    if (home !== "." && !named.has(home)) out.push(`.index: says nothing of ${home}`);
  }
  return out;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const repo = process.argv[2] ?? join(dirname(fileURLToPath(import.meta.url)), "..", "..");
  const found = problems(tracked(repo), (f) => readFileSync(join(repo, f), "utf8"));
  for (const p of found) console.error(p);
  console.log(found.length ? `${found.length} to put right` : "CLAUDE.md, .index and .knowledge agree with the repository");
  process.exit(found.length ? 1 : 0);
}
