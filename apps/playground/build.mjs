// Bundles the UI into dist/: app.js (the playground, with Monaco's editor API and Extended Mark's language),
// app.css (ours, and Monaco's), Monaco's codicon font, editor.worker.js, Monaco's web worker, as an
// entry of its own (src/monaco.ts finds it through MonacoEnvironment.getWorkerUrl), and the
// compiler: compiler.worker.js, which runs xtmd.wasm (@extended-mark/wasm's, copied beside it). The three pages are there
// too: built.html, with the files the web engine made of pages/coffee.xtmd (built/); embedded.html, with
// basket.ir.json for its React app; and live.html.
// `npm run build` runs it; the server runs it at startup when dist/app.js is missing.
// `node build.mjs --dev` leaves the code readable and writes source maps.

import { build } from "esbuild";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { gzipSync } from "node:zlib";
import { shell } from "./src/shell.mjs";

const here = dirname(fileURLToPath(import.meta.url));

export async function buildBundle({ minify = true, sourcemap = !minify } = {}) {
  const outdir = join(here, "dist");
  // the compiler the page runs: compiler/wasm/build.sh makes it
  let wasm;
  try {
    wasm = createRequire(join(here, "package.json")).resolve("@extended-mark/wasm/xtmd.wasm");
  } catch {
    wasm = undefined;
  }
  if (!wasm || !existsSync(wasm)) throw new Error("no xtmd.wasm in @extended-mark/wasm: run compiler/wasm/build.sh");
  rmSync(outdir, { recursive: true, force: true });
  mkdirSync(outdir, { recursive: true });
  copyFileSync(wasm, join(outdir, "xtmd.wasm"));
  const { Compiler } = await import("@extended-mark/wasm");
  const compiler = await Compiler.load(pathToFileURL(wasm));
  const ir = (name) => {
    const made = compiler.build(`${name}.xtmd`, readFileSync(join(here, "pages", `${name}.xtmd`), "utf8"));
    if (!made.ir) throw new Error(`pages/${name}.xtmd doesn't compile: ${JSON.stringify(made.diagnostics)}`);
    return made.ir;
  };
  await writePages(outdir, ir);
  const common = {
    absWorkingDir: here,
    outdir,
    bundle: true,
    format: "esm",
    platform: "browser",
    target: "es2022",
    minify,
    sourcemap,
    // Monaco's and its dependencies' licences, beside the bundle rather than in it
    legalComments: "linked",
    logLevel: "warning",
  };
  await Promise.all([
    build({
      ...common,
      entryPoints: { app: join(here, "src/app.ts") },
      // the codicon font Monaco's CSS points at, written beside app.css
      loader: { ".ttf": "file" },
      assetNames: "[name]-[hash]",
    }),
    build({
      ...common,
      // a module: Monaco starts it from a small blob that imports this URL
      entryPoints: { "editor.worker": "monaco-editor/editor/editor.worker.js" },
    }),
    build({
      ...common,
      entryPoints: { "compiler.worker": join(here, "src/compiler.worker.ts") },
    }),
    build({
      ...common,
      entryPoints: { pages: join(here, "src/pages.css"), built: join(here, "src/built.ts"), embedded: join(here, "src/embedded.ts"), live: join(here, "src/live.ts") },
      // a document's text, for the live page
      loader: { ".xtmd": "text" },
      // React's production build, and one copy of it: @extended-mark/react is linked from its own folder, which has a
      // React of its own
      define: { "process.env.NODE_ENV": '"production"' },
      alias: { react: join(here, "node_modules/react"), "react-dom": join(here, "node_modules/react-dom") },
    }),
  ]);
}

/** The HTML of /built, /embedded and /live, and what the first two are built from. */
async function writePages(outdir, ir) {
  const { build: engine } = await import("@extended-mark/web");
  const icon = /<link\s+rel="icon"[\s\S]*?\/>/.exec(readFileSync(join(here, "index.html"), "utf8"))[0];
  let facts;
  const colour = (light, dark) => ({ record: { type: "core/color_value", fields: { light: { text: light }, dark: { text: dark } } } });
  const { files } = await engine(
    { version: 1, documents: [{ name: "coffee", ir: ir("coffee").ir }] },
    {
      tokens: { "core/color.ACCENT": colour("#b4501a", "#f1a36c") },
      shell: (p) => {
        facts = (sizes) =>
          shell({
            path: "/built",
            title: p.title,
            about: "An Extended Mark page turned into HTML, a stylesheet and a small script ahead of time.",
            icon,
            styles: p.styles.map((s) => `/built/${s}`),
            scripts: ["/built.js"],
            head: `<script type="module" src="/built/${p.script}" data-page></script>\n`,
            main: `<div class="wrap">
<div class="two">
<article class="card article">${p.body}</article>
<aside class="aside">
<section class="card" aria-labelledby="ahead">
<h2 id="ahead">Built ahead</h2>
<p class="note">A program turned this document into files before you came. It is all here before any script runs: its script only moves what moves.</p>
<dl class="facts" data-testid="facts">
${sizes}
<dt>Compiler</dt><dd>none</dd>
<dt>IR</dt><dd>none</dd>
</dl>
</section>
<section class="card" aria-labelledby="how">
<h2 id="how">How</h2>
<p class="note"><code>xtmd export docs --out dist --engine web</code></p>
<p class="note">The article is <code>coffee.xtmd</code>. The bar and this column are the site's own: the engine is given the page around a page, and an accent colour of the site's.</p>
</section>
</aside>
</div>
</div>`,
          });
        return "";
      },
    },
  );
  const kb = (n) => `${(n / 1000).toFixed(1)} kB`;
  const zipped = (end) => kb(files.filter((f) => f.path.endsWith(end)).reduce((n, f) => n + gzipSync(f.text).length, 0));
  mkdirSync(join(outdir, "built/assets"), { recursive: true });
  for (const f of files) if (f.path !== "coffee.html") writeFileSync(join(outdir, "built", f.path), f.text);
  writeFileSync(join(outdir, "built.html"), facts(`<dt>Script, zipped</dt><dd>${zipped(".js")}</dd>\n<dt>Styles, zipped</dt><dd>${zipped(".css")}</dd>`));

  writeFileSync(join(outdir, "basket.ir.json"), JSON.stringify(ir("basket")));
  writeFileSync(
    join(outdir, "embedded.html"),
    shell({
      path: "/embedded",
      title: "An embedded page",
      about: "An Extended Mark page as part of a React app, which holds its state and hears what it calls.",
      icon,
      scripts: ["/embedded.js"],
      main: `<div class="wrap">
<div class="lead">
<h1>A page in an app</h1>
<p>A small shop, written in React. Its basket is an Extended Mark document: React draws it, and the app around it holds what is in it.</p>
</div>
<div id="app" data-testid="app"><p class="empty">Loading the page…</p></div>
</div>`,
    }),
  );
  writeFileSync(
    join(outdir, "live.html"),
    shell({
      path: "/live",
      title: "A live page",
      about: "An Extended Mark page compiled in the browser as its text is typed.",
      icon,
      scripts: ["/live.js"],
      main: `<div class="wrap wide">
<div class="lead">
<h1>A page as you write it</h1>
<p>The text on one side is compiled in your browser every time it changes. What you have set on the page stays, and a draft that doesn't compile leaves the last page that did.</p>
</div>
<div class="halves">
<section class="card" aria-labelledby="text">
<div class="head"><h2 id="text">The text</h2>
<select id="picker" aria-label="Which document"><option value="triangle">A triangle</option><option value="coffee">Coffee</option><option value="counter">A counter</option></select></div>
<textarea id="source" spellcheck="false" autocapitalize="off" autocomplete="off" aria-label="The document's text" data-testid="source"></textarea>
<p id="status" class="status" data-state="loading" aria-live="polite" data-testid="status">Loading the compiler…</p>
</section>
<section class="card" aria-label="The page"><div id="page" data-testid="page"></div></section>
</div>
</div>`,
    }),
  );
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  await buildBundle({ minify: !process.argv.includes("--dev") });
  process.stderr.write("built dist/: app.js, app.css, editor.worker.js, compiler.worker.js, xtmd.wasm, the codicon font, and the three pages\n");
}
