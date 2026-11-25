import { test } from "node:test";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { compile } from "svelte/compiler";
import * as esbuild from "esbuild";
import { chromium } from "playwright-core";
import * as cli from "extended-mark";
import { build } from "@extended-mark/svelte/engine";
import { builtPages, bundle, compilePages, componentSteps, docSteps, launch, open } from "../../testing/browser.mjs";

const irs = await compilePages(cli);
const at = (f) => fileURLToPath(new URL(f, import.meta.url));
await builtPages(build, irs, at("./built"), { components: ["core/press", "core/heading"] });
const app = (entry, pages) => bundle(esbuild, at(entry), { alias: { "#pages": at(pages) }, plugins: [svelte("client")], conditions: ["svelte", "browser"] });

/** What a server writes of a component, given by the code that exports it as `Page`. */
async function server(page, props) {
  const r = await esbuild.build({
    stdin: { contents: `export { render } from "svelte/server"; ${page}`, resolveDir: at(".") },
    bundle: true, write: false, format: "esm", platform: "node", conditions: ["svelte"], plugins: [svelte("server")], logLevel: "silent",
  });
  const { render, Page } = await import(`data:text/javascript;base64,${Buffer.from(r.outputFiles[0].text).toString("base64")}`);
  return () => render(Page, { props }).body;
}

/** .svelte files compiled by Svelte's own compiler, for the browser or a server. */
const svelte = (generate) => ({
  name: "svelte",
  setup(build) {
    build.onLoad({ filter: /\.svelte$/ }, async (args) => {
      const r = compile(await readFile(args.path, "utf8"), { filename: args.path, generate });
      const warnings = r.warnings.filter((w) => !args.path.includes("/test/"));
      if (warnings.length) return { errors: warnings.map((w) => ({ text: w.message })) };
      return { contents: r.js.code, loader: "js" };
    });
  },
});

test("<XtmdDoc> on a server renders the page itself, its widgets drawn by Svelte", async () => {
  const r = await esbuild.build({
    stdin: { contents: 'export { render } from "svelte/server"; export { XtmdDoc } from "@extended-mark/svelte";', resolveDir: fileURLToPath(new URL(".", import.meta.url)) },
    bundle: true, write: false, format: "esm", platform: "node", conditions: ["svelte"], plugins: [svelte("server")], logLevel: "silent",
  });
  const { render, XtmdDoc } = await import(`data:text/javascript;base64,${Buffer.from(r.outputFiles[0].text).toString("base64")}`);
  const { body } = render(XtmdDoc, { props: { ir: irs.v1, defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } } });
  // Svelte marks its blocks with comments for hydration
  const html = body.replace(/<!--[^>]*-->/g, "");
  assert.match(html, /<div[^>]* class="xtmd-root[^"]*"/);
  assert.ok(html.includes("Count: 2 items"), html);
  assert.match(html, /data-part="press\.label"[^>]*>Add one</);
});

test("<XtmdDoc> in Chromium: controlled $state, refusals, props, snapshots", async () => {
  const js = await app("./doc-app.mjs", "./pages-embedded.mjs");
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
  const js = await app("./doc-app.mjs", "./pages-embedded.mjs");
  const browser = await launch(chromium);
  try {
    const { page, errors } = await open(browser, { js, irs, mine: true });
    await componentSteps(page);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});

const given = { defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } };
const BUILT = 'export { default as Page } from "./built/v1.svelte";';
const EMBEDDED = 'export { XtmdDoc as Page } from "@extended-mark/svelte";';

test("what a server writes has the same ids however many pages it wrote before", async () => {
  for (const [page, props] of [['export { default as Page } from "./built/notes.svelte";', {}], [EMBEDDED, { ir: irs.notes }]]) {
    const write = await server(page, props);
    const one = write();
    assert.match(one, /id="xtmd/);
    assert.equal(write(), one);
  }
});

test("a page the engine built, on a server and in Chromium: the same steps, its HTML taken over", async () => {
  const html = (await server(BUILT, given))();
  assert.ok(html.replace(/<!--[^>]*-->/g, "").includes("Count: 2 items"), html);
  // a component inside a component, so two more of Svelte's markers than <XtmdDoc> writes
  const bare = (s) => s.replace(/<!--[^>]*-->/g, "");
  assert.equal(bare(html), bare((await server(EMBEDDED, { ir: irs.v1, ...given }))()), "what <XtmdDoc> writes");
  const browser = await launch(chromium);
  try {
    const { page, errors } = await open(browser, { js: await app("./doc-app.mjs", "./pages-built.mjs"), irs: {} });
    await docSteps(page, { markdown: false });
    assert.deepEqual(errors, []);
    const two = await browser.newPage();
    const said = [];
    two.on("pageerror", (e) => said.push(String(e)));
    two.on("console", (m) => ["error", "warning"].includes(m.type()) && said.push(m.text()));
    await two.setContent(`<!doctype html><div id="app">${html}</div>`);
    await two.evaluate(() => void (document.querySelector("#app h1").mine = "yes"));
    await two.addScriptTag({ content: await app("./hydrate-app.mjs", "./pages-built.mjs") });
    await two.click('#app [data-xtmd-widget="core/press"]');
    await two.waitForFunction(() => document.querySelector("#app").textContent.includes("Count: 3 items"));
    assert.equal(await two.evaluate(() => document.querySelector("#app h1").mine), "yes", "the element the server wrote");
    assert.deepEqual(said, []);
  } finally {
    await browser.close();
  }
});

test("the host's components as widgets of a page the engine built", async () => {
  const browser = await launch(chromium);
  try {
    const { page, errors } = await open(browser, { js: await app("./doc-app.mjs", "./pages-built.mjs"), irs: {}, mine: true });
    await componentSteps(page);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});

test("what the engine writes holds no runtime and no IR, and has its types", () => {
  const files = readdirSync(at("./built"), { recursive: true }).map(String);
  assert.ok(files.includes("v1.svelte.d.ts") && files.includes(join("assets", "xtmd.d.ts")), files.join(" "));
  const js = files.filter((f) => f.endsWith(".js") || f.endsWith(".svelte")).map((f) => readFileSync(join(at("./built"), f), "utf8")).join("\n");
  for (const word of ["fromModule", "this evaluator doesn't know", "declarations.find"]) assert.ok(!js.includes(word), word);
});
