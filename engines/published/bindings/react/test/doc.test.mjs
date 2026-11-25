import { test } from "node:test";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { createElement as h } from "react";
import { renderToString } from "react-dom/server";
import * as esbuild from "esbuild";
import { chromium } from "playwright-core";
import * as cli from "extended-mark";
import { XtmdDoc } from "@extended-mark/react";
import { build } from "@extended-mark/react/engine";
import { builtPages, bundle, compilePages, componentSteps, docSteps, launch, open } from "../../testing/browser.mjs";

const irs = await compilePages(cli);
const at = (f) => fileURLToPath(new URL(f, import.meta.url));
await builtPages(build, irs, at("./built"), { components: ["core/press", "core/heading"] });
const app = (pages) => bundle(esbuild, at("./doc-app.mjs"), { alias: { "#pages": at(pages) } });

test("<XtmdDoc> on a server renders the page itself, its widgets drawn by React", () => {
  const html = renderToString(h(XtmdDoc, { ir: irs.v1, defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } }));
  assert.match(html, /^<div class="xtmd-root[^"]*" data-xtmd-root=""/);
  assert.match(html, /<style data-xtmd-dom="">/);
  // React keeps adjacent text apart with empty comments
  assert.ok(html.replace(/<!-- -->/g, "").includes("Count: 2 items"), html);
  assert.match(html, /data-xtmd-widget="core\/press"[^>]*>/);
  assert.match(html, /<span data-part="press\.label" class="xtmd-press-label">Add one<\/span>/);
});

test("<XtmdDoc> in Chromium: controlled $state, refusals, props, snapshots, in React's StrictMode", async () => {
  const js = await app("./pages-embedded.mjs");
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
  const js = await app("./pages-embedded.mjs");
  const browser = await launch(chromium);
  try {
    const { page, errors } = await open(browser, { js, irs, mine: true });
    await componentSteps(page);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});

test("a page the engine built, on a server and in Chromium: the same steps, its HTML taken over", async () => {
  const { default: Counter } = await import("./built/v1.js");
  const html = renderToString(h(Counter, { defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } }));
  assert.ok(html.replace(/<!-- -->/g, "").includes("Count: 2 items"), html);
  assert.equal(html.replace(/xtmdv\d+-/g, ""), renderToString(h(XtmdDoc, { ir: irs.v1, defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } })).replace(/xtmdv\d+-/g, ""), "what <XtmdDoc> writes");
  const browser = await launch(chromium);
  try {
    const { page, errors } = await open(browser, { js: await app("./pages-built.mjs"), irs: {} });
    await docSteps(page, { markdown: false });
    assert.deepEqual(errors, []);
    const hydrated = await bundle(esbuild, at("./hydrate-app.mjs"));
    const two = await browser.newPage();
    const said = [];
    two.on("pageerror", (e) => said.push(String(e)));
    two.on("console", (m) => m.type() === "error" && said.push(m.text()));
    await two.setContent(`<!doctype html><div id="app">${html}</div>`);
    await two.evaluate(() => void (document.querySelector("#app h1").mine = "yes"));
    await two.addScriptTag({ content: hydrated });
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
    const { page, errors } = await open(browser, { js: await app("./pages-built.mjs"), irs: {}, mine: true });
    await componentSteps(page);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});

test("what the engine writes holds no runtime, no IR and no React, and has its types", () => {
  const files = readdirSync(at("./built"), { recursive: true }).map(String);
  assert.ok(files.includes("v1.d.ts") && files.includes(join("assets", "xtmd.d.ts")), files.join(" "));
  const js = files.filter((f) => f.endsWith(".js")).map((f) => readFileSync(join(at("./built"), f), "utf8")).join("\n");
  for (const word of ["fromModule", "this evaluator doesn't know", "declarations.find", "react.element", "ReactCurrentDispatcher"]) assert.ok(!js.includes(word), word);
  assert.match(readFileSync(at("./built/v1.js"), "utf8"), /^"use client";\n/);
});

test("what a server writes has the same ids however many pages it wrote before", async () => {
  const { default: Notes } = await import("./built/notes.js");
  const one = renderToString(h(Notes));
  assert.match(one, /id="xtmd/);
  assert.equal(renderToString(h(Notes)), one);
  assert.equal(renderToString(h(XtmdDoc, { ir: irs.notes })), renderToString(h(XtmdDoc, { ir: irs.notes })));
});
