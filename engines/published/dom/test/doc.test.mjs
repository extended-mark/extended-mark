import { test } from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";
import { chromium } from "playwright-core";
import * as cli from "extended-mark";
import { toHtml } from "@extended-mark/widgets";
import { Doc } from "@extended-mark/widgets/doc";
import { bundle, compilePages, docSteps, launch, open } from "../../bindings/testing/browser.mjs";

const irs = await compilePages(cli);

test("the plain web's XtmdDoc in Chromium: controlled $state, refusals, props, snapshots", async () => {
  const js = await bundle(esbuild, fileURLToPath(new URL("./doc-app.mjs", import.meta.url)));
  const browser = await launch(chromium);
  try {
    const { page, errors } = await open(browser, { js, irs });
    await docSteps(page);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});

test("on a server, the same page as HTML which XtmdDoc takes over in the browser", async () => {
  // what a server writes into the element: no DOM, no framework
  const html = toHtml(new Doc({ ir: irs.v1, defaultState: { n: { num: "2" } }, externs: { app: { unit: { text: "items" } } } }).render());
  assert.match(html, /^<div class="xtmd-root[^"]*" data-xtmd-root/);
  assert.ok(html.includes("Count: 2 items"));
  const js = await bundle(esbuild, fileURLToPath(new URL("./doc-server-app.mjs", import.meta.url)));
  const browser = await launch(chromium);
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (e) => errors.push(e));
    await page.setContent(`<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app">${html}</div></body></html>`);
    assert.ok((await page.textContent("#app")).includes("Count: 2 items"), "readable before any script");
    await page.evaluate((irs) => (window.__IR = irs), irs);
    await page.evaluate(() => void (document.querySelector("#app h1").mine = "yes"));
    await page.addScriptTag({ content: js });
    await page.waitForFunction(() => window.__test?.doc.running);
    // one page, XtmdDoc's, and it works
    assert.equal(await page.evaluate(() => document.querySelectorAll("#app [data-xtmd-widget='core/press']").length), 1);
    await page.click('#app [data-xtmd-widget="core/press"]');
    await page.waitForFunction(() => document.querySelector("#app").textContent.includes("Count: 3 items"));
    assert.ok((await page.evaluate(() => window.__server)).includes("Count: 2 items"));
    assert.equal(await page.evaluate(() => document.querySelector("#app h1").mine), "yes", "the element the server wrote");
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
