// A page two ways in Chromium, side by side: run from its IR by the runtime and drawn by the plain renderer, and as
// the engine built it. The same change goes to both, and what is on the page, its data and what it tells are
// compared after each.

import { execFileSync } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, mkdtempSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";
import { chromium } from "playwright-core";
import { build } from "../dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
export const repo = join(here, "../../..");
const dir = mkdtempSync(join(tmpdir(), "xtmd-web-test-"));
let n = 0;

function xtmd() {
  for (const kind of ["release", "debug"]) {
    const p = join(repo, "compiler/target", kind, "xtmd");
    if (existsSync(p)) return p;
  }
  throw new Error("no xtmd binary: run `cargo build --release` in compiler");
}

/** A document's IR, or undefined for one that has errors. */
export function irOf(file, packs = []) {
  const out = join(dir, `${n++}.ir.json`);
  try {
    execFileSync(xtmd(), ["build", file, ...packs.flatMap((p) => ["--pack", p]), "--out", out], { stdio: "pipe" });
  } catch {
    return undefined;
  }
  return JSON.parse(readFileSync(out, "utf8")).ir;
}

export function source(text, packs) {
  const file = join(dir, `page${n++}.xtmd`);
  writeFileSync(file, text);
  const ir = irOf(file, packs);
  if (!ir) throw new Error(`it doesn't compile:\n${text}`);
  return ir;
}

/** Every document under a folder. */
export function documents(folder, out = []) {
  for (const f of readdirSync(folder).sort()) {
    if (f === "node_modules" || f === "dist" || f === "build") continue;
    const p = join(folder, f);
    if (statSync(p).isDirectory()) documents(p, out);
    else if (f.endsWith(".xtmd")) out.push(p);
  }
  return out;
}

let harness;

/** A browser the pages are put in, and a place their files are got from. */
export async function start() {
  const files = new Map();
  const server = createServer((q, r) => {
    const f = files.get(decodeURIComponent(q.url.split("?")[0].slice(1)));
    if (f === undefined) return void r.writeHead(404).end();
    r.writeHead(200, { "content-type": q.url.includes(".js") ? "text/javascript" : q.url.includes(".css") ? "text/css" : "text/html; charset=utf-8" }).end(f);
  }).listen(0);
  harness ??= (await esbuild.build({ entryPoints: [join(here, "harness.mjs")], bundle: true, write: false, format: "iife", platform: "browser", target: "es2022", logLevel: "silent" })).outputFiles[0].text;
  files.set("harness.js", harness);
  const browser = await chromium.launch({ executablePath: process.env.XTMD_CHROMIUM || "/usr/bin/chromium", headless: true, args: ["--no-sandbox", "--disable-gpu"] });
  return {
    /** The two pages of an IR, each in a tab of its own: two of a document's popovers shut each other. */
    async open(ir, { options = {}, given = {} } = {}) {
      const name = `d${n++}`;
      const built = await build({ version: 1, documents: [{ name, ir }] }, { ...options, shell: (p) => JSON.stringify(p) });
      for (const f of built.files) files.set(f.path, f.text);
      const shell = JSON.parse(files.get(`${name}.html`));
      const html = (body) => `<!doctype html><html lang="en"><head><meta charset="utf-8"><style>body{margin:0}#page{width:720px}</style>${shell.styles.map((s) => `<link rel="stylesheet" href="${s}">`).join("")}<script src="harness.js"></script></head><body><div id="page">${body}</div></body></html>`;
      files.set(`${name}.html`, html(shell.body.replace("<main ", "<main data-xtmd-held ")));
      files.set(`${name}-run.html`, html(""));
      const errors = [];
      const tab = async (path) => {
        const page = await browser.newPage({ viewport: { width: 800, height: 900 } });
        page.on("pageerror", (e) => errors.push(String(e?.stack ?? e)));
        await page.goto(`http://localhost:${server.address().port}/${path}.html`);
        return page;
      };
      const [ours, theirs] = [await tab(`${name}-run`), await tab(name)];
      await ours.evaluate(([ir, given]) => window.side.run(ir, given), [ir, given]);
      await theirs.evaluate(([script, given]) => window.side.take(script, given), [shell.script, given]);
      return {
        errors,
        files: built.files,
        shell,
        ours,
        theirs,
        /** Both as they are: undefined when they are the same. */
        async differ(change) {
          const look = (p) => p.evaluate((a) => (a ? window.side.act(a) : window.side.now()), change);
          const [a, b] = [await look(ours), await look(theirs)];
          // a page that never changes has no data to give
          const of = Object.keys(b);
          return of.every((k) => JSON.stringify(a[k]) === JSON.stringify(b[k])) ? undefined : { change, a, b };
        },
        /** Random changes, each compared: the first that differs. */
        async random(seed, steps) {
          let d = await this.differ();
          for (let k = 0; k < steps && !d; k++) {
            const change = await ours.evaluate(([ir, s]) => window.side.change(ir, s), [ir, seed + k * 7]);
            d = await this.differ(change);
            if (d) d.step = k + 1;
          }
          return d;
        },
        async close() {
          await ours.close();
          await theirs.close();
        },
      };
    },
    async close() {
      await browser.close();
      server.close();
    },
  };
}
