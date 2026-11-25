import { test } from "node:test";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";
import { chromium } from "playwright-core";
import { createComponent, generateHydrationScript, renderToString } from "solid-js/web";
import * as cli from "extended-mark";
import { XtmdDoc } from "@extended-mark/solid";
import { build } from "@extended-mark/solid/engine";
import { builtPages, bundle, compilePages, componentSteps, docSteps, launch, open } from "../../testing/browser.mjs";

const irs = await compilePages(cli);
const app = (entry, pages) => bundle(esbuild, at(entry), { alias: { "#pages": at(pages) }, conditions: ["browser"] });
const at = (f) => fileURLToPath(new URL(f, import.meta.url));
await builtPages(build, irs, at("./built"), { components: ["core/press", "core/heading"] });

test("<XtmdDoc> on a server renders the page itself, its widgets drawn by Solid", async () => {
  // Node resolves solid-js to its server build, as Solid's SSR does
  const html = renderToString(() => createComponent(XtmdDoc, { ir: irs.v1, defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } }));
  // Solid keeps a space after a class it sets
  assert.match(html, /^<div[^>]* class="xtmd-root[^"]*"/);
  assert.match(html, /<style[^>]*>/);
  assert.ok(html.replace(/<!--[^>]*-->/g, "").includes("Count: 2 items"), html);
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
const server = async (of, props) => renderToString(() => createComponent(of, props));

test("what a server writes has the same ids however many pages it wrote before", async () => {
  const { default: Notes } = await import("./built/notes.js");
  const one = await server(Notes, {});
  assert.match(one, /id="xtmd/);
  assert.equal(await server(Notes, {}), one);
  assert.equal(await server(XtmdDoc, { ir: irs.notes }), await server(XtmdDoc, { ir: irs.notes }));
});

test("a page the engine built, on a server and in Chromium: the same steps, its HTML taken over", async () => {
  const { default: Counter } = await import("./built/v1.js");
  const html = await server(Counter, given);
  assert.ok(html.replace(/<!--[^>]*-->/g, "").includes("Count: 2 items"), html);
  assert.equal(html, await server(XtmdDoc, { ir: irs.v1, ...given }), "what <XtmdDoc> writes");
  const browser = await launch(chromium);
  try {
    const { page, errors } = await open(browser, { js: await app("./doc-app.mjs", "./pages-built.mjs"), irs: {} });
    await docSteps(page, { markdown: false });
    assert.deepEqual(errors, []);
    const two = await browser.newPage();
    const said = [];
    two.on("pageerror", (e) => said.push(String(e)));
    two.on("console", (m) => ["error", "warning"].includes(m.type()) && said.push(m.text()));
    await two.setContent(`<!doctype html><div id="app">${html}</div>${generateHydrationScript()}`);
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
  assert.ok(files.includes("v1.d.ts") && files.includes(join("assets", "xtmd.d.ts")), files.join(" "));
  const js = files.filter((f) => f.endsWith(".js")).map((f) => readFileSync(join(at("./built"), f), "utf8")).join("\n");
  for (const word of ["fromModule", "this evaluator doesn't know", "declarations.find"]) assert.ok(!js.includes(word), word);
});
