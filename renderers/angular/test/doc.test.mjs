import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import * as esbuild from "esbuild";
import ts from "typescript";
import { chromium } from "playwright-core";
import * as cli from "extended-mark";
import { build } from "@extended-mark/angular/engine";
import { builtPages, bundle, compilePages, componentSteps, docSteps, launch, open, typeErrors } from "../../testing/browser.mjs";

const irs = await compilePages(cli);
const at = (f) => fileURLToPath(new URL(f, import.meta.url));
await builtPages(build, irs, at("./built"), { components: ["core/press", "core/heading"] });
const decorators = { tsconfigRaw: { compilerOptions: { experimentalDecorators: true, useDefineForClassFields: false } } };

const app = (pages) => bundle(esbuild, at("./doc-app.ts"), { ...decorators, alias: { "#pages": at(pages) } });

/** What a server writes of a page, by the module that has it. */
async function server(page, ir) {
  const r = await esbuild.build({ entryPoints: [at("./doc-server.ts")], bundle: true, write: false, format: "esm", platform: "node", logLevel: "silent", ...decorators, alias: { "#pages": at(page) }, banner: { js: "import { createRequire } from 'module'; const require = createRequire(import.meta.url);" } });
  // a file of its own: Angular's server build asks for `require`, which needs the module's path
  const dir = mkdtempSync(join(tmpdir(), "xtmd-angular-"));
  const file = join(dir, "doc-server.mjs");
  writeFileSync(file, r.outputFiles[0].text);
  return (await import(pathToFileURL(file).href)).render(ir);
}

test("<xtmd-doc> on a server renders the page itself, through Angular's Renderer2", async () => {
  const html = await server("./pages-embedded.ts", irs.v1);
  assert.match(html, /<div[^>]*class="xtmd-root[^"]*"/);
  assert.ok(html.includes("Count: 2 items"), html);
  assert.match(html, /data-part="press\.label"[^>]*>Add one</);
});

test("<xtmd-doc> in Chromium: controlled $state, refusals, props, snapshots", async () => {
  const js = await app("./pages-embedded.ts");
  const browser = await launch(chromium);
  try {
    const { page, errors } = await open(browser, { js, irs });
    await docSteps(page);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});

test("the host's own components as widgets, with their slots", async () => {
  const js = await app("./pages-embedded.ts");
  const browser = await launch(chromium);
  try {
    const { page, errors } = await open(browser, { js, irs, mine: true });
    await componentSteps(page);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});

test("a page the engine built, on a server and in Chromium: the same steps", async () => {
  const html = await server("./pages-built.ts", undefined);
  const page_ = (h) => /<div[^>]*class="xtmd-root[\s\S]*/.exec(h)[0].replace(/xtmdv\d+-/g, "").replace(/<\/xtmd-(doc|v1)>[\s\S]*/, "");
  assert.equal(page_(html), page_(await server("./pages-embedded.ts", irs.v1)), "what <xtmd-doc> writes");
  const browser = await launch(chromium);
  try {
    const { page, errors } = await open(browser, { js: await app("./pages-built.ts"), irs: {} });
    await docSteps(page, { markdown: false });
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});

test("the host's components as widgets of a page the engine built", async () => {
  const browser = await launch(chromium);
  try {
    const { page, errors } = await open(browser, { js: await app("./pages-built.ts"), irs: {}, mine: true });
    await componentSteps(page);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});

test("what the engine writes holds no runtime and no IR, and the linker of a host's build takes it", async () => {
  const { createEs2015LinkerPlugin } = await import("@angular/compiler-cli/linker/babel");
  const { NodeJSFileSystem, ConsoleLogger, LogLevel } = await import("@angular/compiler-cli");
  const babel = (await import("@babel/core")).default;
  const files = readdirSync(at("./built"), { recursive: true }).map(String);
  assert.ok(files.includes("v1.d.ts") && files.includes(join("assets", "xtmd.d.ts")), files.join(" "));
  for (const f of files.filter((f) => f.endsWith(".js"))) {
    const js = readFileSync(join(at("./built"), f), "utf8");
    for (const word of ["fromModule", "this evaluator doesn't know", "declarations.find"]) assert.ok(!js.includes(word), word);
    if (!js.includes("ɵɵngDeclare")) continue;
    const linked = babel.transformSync(js, { filename: join(at("./built"), f), compact: false, plugins: [createEs2015LinkerPlugin({ fileSystem: new NodeJSFileSystem(), logger: new ConsoleLogger(LogLevel.error), linkerJitMode: false, sourceMapping: false })] }).code;
    assert.ok(!linked.includes("ngDeclareComponent"), f);
    if (f === "v1.js") assert.match(linked, /ɵɵdefineComponent\(\{[\s\S]*selectors: \[\["xtmd-v1"\]\]/);
  }
});

test("a host's use of a page the engine built is typed", () => {
  assert.deepEqual(typeErrors(ts, at("./built"), `import { V1 } from "./v1.js";\nexport const ok = (p: V1) => { p.state = { n: { num: "1" } }; p.stateChange.subscribe((s) => void s.n); return p.snapshot(); };\n`), []);
  assert.equal(typeErrors(ts, at("./built"), `import { V1 } from "./v1.js";\nexport const bad = (p: V1) => { p.page = { theme: "BLUE" }; };\n`).length, 1);
});
