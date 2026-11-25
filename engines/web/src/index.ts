// Extended Mark's web engine: IR in, a page's files out. A page is HTML, written as it starts, with a stylesheet;
// what of it changes has a script, which takes that HTML over. Nothing of the IR is in it.

import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import * as esbuild from "esbuild";
import { NAMES } from "@extended-mark/widgets/each";
import { Tokens, children, styleText, textOf, toHtml, walk, type Child, type VNode, type ViewWidget } from "@extended-mark/widgets";
import type { Input, ir } from "@extended-mark/runtime";
import type { Kit, Program } from "./kit/kit.js";
import type { Built as BuiltPage } from "./kit/mount.js";
import { generate, type Generated } from "./generate.js";
import { moving } from "./static.js";

/** What is built (schemas/build.schema.json). */
export interface Job {
  version: 1;
  documents: { name: string; ir: ir.Module }[];
}

export interface Files {
  version: 1;
  files: { path: string; text: string }[];
}

/** A page, for the shell around it. */
export interface Shell {
  /** The document's place among what is built. */
  name: string;
  /** Its first heading, or its name. */
  title: string;
  /** The page's element, with everything in it. */
  body: string;
  /** Where its stylesheets and its script are, from the page. A page that never changes has no script. */
  styles: string[];
  script?: string;
  frontMatter?: string;
}

/** The places to plug in. */
export interface Options {
  /** A module whose `widgets` are the host's own, by full name: each replaces ours, or is a pack's. */
  widgets?: string;
  /** Draw with the host's widgets only. */
  only?: boolean;
  /** Token values in place of the IR's, by the token's full name (`core/color.INK`). */
  tokens?: Record<string, Input>;
  /** The page around a page: its HTML. Left out, a plain one that starts its page. */
  shell?: (page: Shell) => string;
  /** A module whose `fns` are value fns of the host's, by full name: kept in the page's script. */
  fns?: string;
  /** Leaves the scripts as they are read, for looking at them. */
  readable?: boolean;
}

/** The options, with which of ours the host's widgets replace. */
export interface GivenOptions extends Options {
  replaced?: ReadonlySet<string>;
}

const require = createRequire(import.meta.url);
/** Where a package of ours is, as this one finds it. */
const here = dirname(require.resolve("@extended-mark/widgets")).replace(/dist$/, "");

/** What is plugged in, and the widgets there are then. */
export async function plugged(given: Options): Promise<{ options: GivenOptions; supplies: (widget: string) => boolean }> {
  const mine: Record<string, ViewWidget> = given.widgets ? ((await import(pathToFileURL(resolve(given.widgets)).href)).widgets ?? {}) : {};
  const options: GivenOptions = { ...given, replaced: new Set(Object.keys(mine)) };
  return { options, supplies: (w) => Object.hasOwn(mine, w) || (!options.only && Object.hasOwn(NAMES, w)) };
}

export async function build(job: Job, given: Options = {}): Promise<Files> {
  const { options, supplies } = await plugged(given);
  const pages: { name: string; page: Page; title: string; frontMatter?: string }[] = [];
  for (const d of job.documents) pages.push({ name: d.name, ...(await page(d.name, d.ir, supplies, options)) });

  const files: Files["files"] = [];
  const scripts = pages.filter((p) => p.page.script !== undefined);
  const paths = await bundle(scripts.map((p) => ({ name: p.name, code: p.page.script! })), options, files);
  const styles = new Map<string, string>();
  for (const p of pages) styles.set(p.page.sheet.name, p.page.sheet.css);
  files.push({ path: "assets/xtmd.css", text: options.readable ? styleText : (await esbuild.transform(styleText, { loader: "css", minify: true })).code });
  for (const [n, css] of styles) files.push({ path: `assets/${n}.css`, text: css });
  for (const p of pages) {
    const up = "../".repeat(p.name.split("/").length - 1);
    const shell: Shell = {
      name: p.name,
      title: p.title,
      body: p.page.html,
      styles: [`${up}assets/xtmd.css`, `${up}assets/${p.page.sheet.name}.css`],
      ...(paths.has(p.name) ? { script: `${up}${paths.get(p.name)}` } : {}),
      ...(p.frontMatter !== undefined ? { frontMatter: p.frontMatter } : {}),
    };
    files.push({ path: `${p.name}.html`, text: (options.shell ?? plain)(shell) });
  }
  return { version: 1, files };
}

export const escape = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

/** The page around a page, when the host has none. */
function plain(p: Shell): string {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escape(p.title)}</title>
${p.styles.map((s) => `<link rel="stylesheet" href="${s}">`).join("\n")}
<style>body{margin:0 auto;padding:16px;max-width:720px}</style>
${p.script ? `<script type="module" src="${p.script}"></script>\n` : ""}</head>
<body>
${p.body}
</body>
</html>
`;
}

// ---- one page

interface Page {
  html: string;
  sheet: { name: string; css: string };
  script?: string;
}

async function page(name: string, m: ir.Module, supplies: (w: string) => boolean, options: GivenOptions): Promise<{ page: Page; title: string; frontMatter?: string }> {
  const full = generate(m, { supplies });
  const made = await run(full, options);
  const doc = m.templates.find((t) => t.name === "doc")!;
  const tokens = made.tokens(options.tokens ?? {});
  const drawn = made.draw(tokens);
  const moves = moving(m);
  // what was told as it started is told again by whatever runs it
  const quiet = Object.keys(drawn.told).length === 0;
  const still = doc.view.map((n, k) => quiet && !moves[k] && ("declare" in n || drawn.groups[k].every(plainElement)));
  const parts = new Set(doc.view.flatMap((_, k) => (still[k] ? [] : [k])));
  const runs = doc.view.flatMap((n, k): (true | number)[] => ("declare" in n ? [0] : still[k] ? drawn.groups[k].map(() => true as const) : [drawn.groups[k].length]));
  const body = drawn.groups.flat().map(toHtml).join("");
  const alive = parts.size > 0 || m.declarations.some((d) => "$state" in d.declare || "expose" in d.declare);
  const sheet = made.sheet(tokens);
  const html = `<main class="xtmd-root ${sheet.name}" data-xtmd-root data-xtmd-page="${escape(name)}" data-xtmd-theme="light">${body}</main>`;
  let script: string | undefined;
  if (alive) {
    const slim = generate(m, { supplies, parts });
    script = pageScript(name, slim, tokens, sheet.name, runs, options);
  }
  let title = name.split("/").pop()!;
  for (const c of drawn.groups.flat()) {
    if (typeof c !== "string" && /^h[1-6]$/.test(c.tag)) {
      title = textOf(c);
      break;
    }
  }
  return { page: { html, sheet, script }, title, frontMatter: m.front_matter };
}

/** An element nothing has to be done for once it is there. */
function plainElement(c: Child): boolean {
  if (typeof c === "string") return false;
  let is = true;
  walk(c, (v: VNode) => {
    if ((v.on && Object.keys(v.on).length) || v.hook || v.component) is = false;
  });
  return is;
}

export const TOKENS = (types: Record<string, Record<string, Input>>) => Object.entries(types).map(([name, values]) => ({ name, values }));

/** The page's widgets and the host's, imported. */
export function widgetsCode(g: Generated, options: GivenOptions): { imports: string; registry: string } {
  const ours = options.only ? [] : g.widgets.filter((w) => Object.hasOwn(NAMES, w) && !options.replaced?.has(w));
  const imports = [ours.length ? `import{${ours.map((w) => `${NAMES[w]} as W${NAMES[w]}`).join(",")}}from"@extended-mark/widgets/each";` : "", options.widgets ? `import{widgets as mine}from${JSON.stringify(resolve(options.widgets))};` : ""].join("");
  return { imports, registry: `{${[...ours.map((w) => `${JSON.stringify(w)}:W${NAMES[w]}`), ...(options.widgets ? ["...mine"] : [])].join(",")}}` };
}

export function fnsCode(g: Generated, options: GivenOptions): { imports: string; fns: string } {
  const imports = [g.local ? `import{webFns}from"@extended-mark/widgets";` : "", options.fns ? `import{fns as given}from${JSON.stringify(resolve(options.fns))};` : ""].join("");
  const of = [g.local ? "...webFns()" : "", options.fns ? "...given" : ""].filter(Boolean);
  return { imports, fns: of.length ? `fns:()=>({${of.join(",")}}),` : "" };
}

function pageScript(name: string, g: Generated, tokens: Record<string, Record<string, Input>>, sheet: string, html: (true | number)[], options: GivenOptions): string {
  const w = widgetsCode(g, options);
  const f = fnsCode(g, options);
  return `${w.imports}${f.imports}import{mount as start${g.calls ? ",withCalls" : ""}}from"@extended-mark/web/kit";${g.planes ? `import{layout}from"@extended-mark/web/layout";` : ""}
${g.code}
const built={program,widgets:${w.registry},tokens:{types:${JSON.stringify(TOKENS(tokens))},sheet:${JSON.stringify(sheet)}},html:${JSON.stringify(html)},${f.fns}${g.planes ? "layout," : ""}${g.calls ? "calls:withCalls," : ""}};
/** Takes over the page's element, as it was written. */
export const mount=(root,options)=>start(root,built,options);
const root=document.querySelector(${JSON.stringify(`[data-xtmd-page="${name}"]:not([data-xtmd-held])`)});
/** The page, where its element is in the document it is loaded in. */
export const page=root?mount(root):undefined;
`.replace("export const tokens=", "const unused=");
}

interface Run {
  tokens(over: Record<string, Input>): Record<string, Record<string, Input>>;
  sheet(tokens: Record<string, Record<string, Input>>): { name: string; css: string };
  draw(tokens: Record<string, Record<string, Input>>): { groups: Child[][]; told: object };
}

/** The whole page's code, run here: what it starts as. */
export async function run(g: Generated, options: GivenOptions): Promise<Run> {
  const w = widgetsCode(g, options);
  const f = fnsCode(g, options);
  const code = `${w.imports}${f.imports}import{kitOf,Nodes,drawingOf${g.calls ? ",withCalls" : ""}}from"@extended-mark/web/kit";
${g.code}
export const built=tokens=>({program,widgets:${w.registry},tokens,html:[],${f.fns}${g.calls ? "calls:withCalls," : ""}});
export {kitOf,Nodes,drawingOf};`;
  const out = await esbuild.build({ stdin: { contents: code, resolveDir: here, loader: "js" }, bundle: true, write: false, format: "esm", platform: "node", plugins: [ours()], logLevel: "silent" });
  const mod = (await import(`data:text/javascript;base64,${Buffer.from(out.outputFiles[0].text).toString("base64")}`)) as {
    built(tokens: BuiltPage["tokens"]): BuiltPage;
    tokens(m: Kit): Record<string, Record<string, Input>>;
    program: Program;
    kitOf(b: BuiltPage, g: object): Kit;
    Nodes: new (m: Kit) => import("./kit/nodes.js").Nodes;
    drawingOf: typeof import("./kit/mount.js").drawingOf;
  };
  const empty = { types: [], sheet: "" };
  return {
    tokens(over) {
      const m = mod.kitOf({ ...mod.built(empty), program: { ...mod.program, v: [] } }, {});
      const all = mod.tokens(m);
      for (const [full, v] of Object.entries(over)) {
        const dot = full.lastIndexOf(".");
        if (all[full.slice(0, dot)]) all[full.slice(0, dot)][full.slice(dot + 1)] = v;
      }
      return all;
    },
    sheet(tokens) {
      const types = TOKENS(tokens);
      const all = Object.fromEntries(types.flatMap((t) => Object.entries(t.values).map(([n, v]) => [`${t.name}.${n}`, v])));
      return new Tokens({ ir: { declarations: types.map((t) => ({ declare: { token: t } })) } as never, given: () => ({}), token: (of) => ("token" in of ? all[of.token] : undefined) }).sheet();
    },
    draw(tokens) {
      const built = mod.built({ types: TOKENS(tokens), sheet: "" });
      const m = mod.kitOf(built, {});
      const nodes = new mod.Nodes(m);
      const drawing = mod.drawingOf(m, nodes, built, "xtmd1-");
      const all = nodes.all();
      return { groups: m.ends.map((end, k) => children(drawing, all.slice(k ? m.ends[k - 1] : 0, end))), told: m.told() };
    },
  };
}

/** Our own packages, wherever the engine is run: this package's, or those of the one whose engine it is. */
const ours = (theirs: NodeJS.Require = require): esbuild.Plugin => ({
  name: "xtmd",
  setup(b) {
    b.onResolve({ filter: /^@extended-mark\// }, (a) => {
      for (const from of [require, theirs]) {
        try {
          const path = from.resolve(a.path);
          if (existsSync(path)) return { path, sideEffects: false };
        } catch {
          // not this one's
        }
      }
      return undefined;
    });
    // numbers' library, which is only its classes
    b.onResolve({ filter: /^decimal\.js$/ }, async (a) => (a.pluginData === "asked" ? undefined : { path: (await b.resolve(a.path, { resolveDir: a.resolveDir, kind: a.kind, pluginData: "asked" })).path, sideEffects: false }));
  },
});

/** The pages' scripts, sharing what they have in common. `more` says what is left outside, whose packages are found, and
 * whether a page's script keeps its page's name. */
export async function bundle(pages: { name: string; code: string }[], options: GivenOptions, files: Files["files"], more: { external?: string[]; from?: NodeJS.Require; named?: boolean } = {}): Promise<Map<string, string>> {
  const out = new Map<string, string>();
  if (!pages.length) return out;
  const entries: esbuild.Plugin = {
    name: "pages",
    setup(b) {
      b.onResolve({ filter: /^page:/ }, (a) => ({ path: a.path.slice(5), namespace: "page" }));
      b.onLoad({ filter: /.*/, namespace: "page" }, (a) => ({ contents: pages[Number(a.path)].code, loader: "js", resolveDir: here }));
    },
  };
  const r = await esbuild.build({
    entryPoints: pages.map((p, k) => ({ in: `page:${k}`, out: p.name })),
    bundle: true,
    splitting: true,
    write: false,
    format: "esm",
    platform: "browser",
    target: "es2022",
    minify: !options.readable,
    outdir: more.named ? "." : "assets",
    entryNames: more.named ? "[dir]/[name]" : "[dir]/[name]-[hash]",
    chunkNames: more.named ? "assets/shared-[hash]" : "shared-[hash]",
    external: more.external,
    plugins: [ours(more.from), entries],
    metafile: true,
    legalComments: "none",
    // as it is written: what reads a script looks for names in it (Angular's linker)
    charset: "utf8",
    logLevel: "silent",
  });
  const cwd = process.cwd();
  for (const f of r.outputFiles) {
    const path = f.path.slice(cwd.length + 1).split("\\").join("/");
    files.push({ path, text: f.text });
    const entry = r.metafile.outputs[path]?.entryPoint;
    if (entry) out.set(pages[Number(entry.replace(/^page:/, ""))].name, path);
  }
  return out;
}

export { generate, type Generated };
