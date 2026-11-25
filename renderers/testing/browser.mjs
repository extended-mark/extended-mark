// What the bindings' tests share: the test pages compiled, a test host bundled for the browser, a
// real Chromium (jsdom can't lay out @extended-mark/dom's pages), and the steps every binding must pass.
//
// Each package passes in its own esbuild, playwright-core and xtmd, so this folder needs no
// node_modules of its own.
//
// A test host sets `window.__test` (see `steps` below for what it holds) and draws into #host.

import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));

export const pages = {
  v1: join(here, "counter.xtmd"),
  v2: join(here, "counter-v2.xtmd"),
  notes: join(here, "notes.xtmd"),
};

/** The test pages' IR, compiled by the xtmd binary through xtmd. */
export async function compilePages(cli) {
  return { v1: await cli.build(pages.v1), v2: await cli.build(pages.v2), notes: await cli.build(pages.notes) };
}

/** The test pages as an engine's components, written into `dir`: each page's file there, by its name. */
export async function builtPages(build, irs, dir, options = {}) {
  rmSync(dir, { recursive: true, force: true });
  const job = { version: 1, documents: Object.entries(irs).map(([name, ir]) => ({ name, ir: ir.ir })) };
  for (const f of (await build(job, options)).files) {
    mkdirSync(dirname(join(dir, f.path)), { recursive: true });
    writeFileSync(join(dir, f.path), f.text);
  }
}

/** What TypeScript says of a host's use of an engine's components: `code`, in a file beside them. */
export function typeErrors(ts, dir, code, options = {}) {
  const file = join(dir, "host-use.ts");
  writeFileSync(file, code);
  const program = ts.createProgram([file], { noEmit: true, strict: true, skipLibCheck: false, target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler, lib: ["lib.es2022.d.ts", "lib.dom.d.ts", "lib.dom.iterable.d.ts"], types: [], ...options });
  const found = ts.getPreEmitDiagnostics(program).filter((d) => !d.file || d.file.fileName.startsWith(dir.replace(/\\/g, "/")));
  rmSync(file);
  return found.map((d) => `${d.file ? d.file.fileName.slice(dir.length) : ""}: ${ts.flattenDiagnosticMessageText(d.messageText, " ")}`);
}

/** Chromium from the system: XTMD_CHROMIUM, or /usr/bin/chromium. Never downloaded. */
export function chromiumPath() {
  const p = process.env.XTMD_CHROMIUM || "/usr/bin/chromium";
  if (!existsSync(p)) throw new Error(`no Chromium at ${p}: set XTMD_CHROMIUM`);
  return p;
}

export async function launch(chromium) {
  return chromium.launch({ executablePath: chromiumPath(), args: ["--no-sandbox"] });
}

/** A test host bundled into one script for the browser. */
export async function bundle(esbuild, entry, opts = {}) {
  const r = await esbuild.build({
    entryPoints: [entry],
    bundle: true,
    write: false,
    format: "iife",
    platform: "browser",
    target: "es2022",
    logLevel: "silent",
    ...opts,
    define: { "process.env.NODE_ENV": '"development"', ...(opts.define ?? {}) },
  });
  return r.outputFiles.map((f) => f.text).join("\n");
}

/** A browser tab with the IR as `window.__IR`, and the bundle run in it (@extended-mark/dom puts its own
 * stylesheet in when it mounts). */
export async function open(browser, { js, irs, mine = false }) {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e));
  await page.setContent(`<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div></body></html>`);
  await page.evaluate(([irs, mine]) => {
    window.__mine = mine;
    window.__IR = irs;
  }, [irs, mine]);
  await page.addScriptTag({ content: js });
  await page.waitForFunction(() => window.__test !== undefined, null, { timeout: 10000 });
  return { page, errors };
}

const text = (page) => page.evaluate(() => document.querySelector("#app")?.textContent ?? "");

async function until(page, fn, arg, what) {
  try {
    await page.waitForFunction(fn, arg, { timeout: 10000 });
  } catch (e) {
    throw new Error(`timed out waiting for ${what}; #app reads: ${JSON.stringify(await text(page))}`);
  }
}

/**
 * The steps every binding's <XtmdDoc> passes, through the test host's `window.__test`:
 * - `heard`: { states: $state[], errors: DocError[], calls: Call[][] }, from onStateChange,
 *   onError and onCalls;
 * - `handle()`: the component's handle (snapshot, restore, set, state, markdown, expose);
 * - `setState(state)`: gives it a new controlled `state` prop (the host also follows onStateChange);
 * - `setTheme(theme)`, `setUnit(text)` (`externs.app.unit`, starting as "items"), `setIr("v1" |
 *   "v2" | "notes")` and `unmount()`, which gives the component a new `page`, `externs`, `ir`, or takes it away.
 *
 * A page an engine built has no Markdown (`markdown` false), and another page is another component.
 */
export async function docSteps(page, { markdown = true } = {}) {
  const app = () => page.evaluate(() => document.querySelector("#app")?.textContent ?? "");
  await until(page, () => document.querySelector("#app")?.textContent?.includes("Count: 0 items"), null, "the page to render");
  assert.equal(await page.evaluate(() => document.querySelectorAll("[data-xtmd-root]").length), 1, "one page drawn");
  assert.ok(await page.evaluate(() => document.querySelector('#app [data-xtmd-widget="core/press"] [data-part="press.label"]') !== null), "core's press, drawn with its parts");
  assert.ok(await page.evaluate(() => document.querySelector("#app style[data-xtmd-dom]") !== null), "the stylesheet, in the page");

  // a reader's click: onStateChange and onCalls hear it
  await page.click('#app [data-xtmd-widget="core/press"]');
  await until(page, () => document.querySelector("#app").textContent.includes("Count: 1 items"), null, "the click");
  await until(page, () => window.__test.heard.calls.length === 1, null, "onCalls");
  let heard = await page.evaluate(() => window.__test.heard);
  assert.deepEqual(heard.calls, [[{ call: { name: "doc/app.saved", args: [{ num: "1" }] } }]]);
  assert.deepEqual(heard.states.at(-1), { n: { num: "1" } });

  // controlled: the host's `state` prop is a checked write, refused when it doesn't fit
  await page.evaluate(() => window.__test.setState({ n: { num: "20" } }));
  await until(page, () => window.__test.heard.errors.some((e) => e.kind === "refused"), null, "the refused state prop");
  assert.ok((await app()).includes("Count: 1 items"), "a refused state leaves the page as it was");
  await page.evaluate(() => window.__test.setState({ n: { num: "4" } }));
  await until(page, () => document.querySelector("#app").textContent.includes("Count: 4 items"), null, "the state prop");

  // the handle: a write refused and done, an exposed fn, the Markdown
  const errors = await page.evaluate(() => window.__test.heard.errors.length);
  await page.evaluate(() => window.__test.handle().set({ n: { num: "20" } }));
  await until(page, (k) => window.__test.heard.errors.length > k, errors, "onError for the handle's write");
  assert.deepEqual((await page.evaluate(() => window.__test.heard.errors)).at(-1), { kind: "refused", code: "contracts.outside", site: "$state.n" });
  await page.evaluate(() => window.__test.handle().expose("app", "reset"));
  await until(page, () => document.querySelector("#app").textContent.includes("Count: 0 items"), null, "expose");
  await page.evaluate(() => window.__test.handle().set({ n: { num: "3" } }));
  await until(page, () => document.querySelector("#app").textContent.includes("Count: 3 items"), null, "the handle's write");
  if (markdown) assert.equal(await page.evaluate(() => window.__test.handle().markdown()), "# Count: 3 items\n\nAdd one\n");

  // props: externs, the theme, a new IR with $state kept where its fields still exist
  await page.evaluate(() => window.__test.setUnit("pts"));
  await until(page, () => document.querySelector("#app").textContent.includes("Count: 3 pts"), null, "the externs prop");
  await page.evaluate(() => window.__test.setTheme("DARK"));
  await until(page, () => document.querySelector("#app [data-xtmd-root]")?.getAttribute("data-xtmd-theme") === "dark", null, "the theme");
  await page.evaluate(() => window.__test.setIr("v2"));
  await until(page, () => document.querySelector("#app").textContent.includes("Total: 3 pts, by 2"), null, "the new IR");
  assert.deepEqual(await page.evaluate(() => window.__test.handle().state()), { n: { num: "3" }, stride: { num: "2" } });

  // widgets' own variables, kept by the page across redraws, and a snapshot that brings them back
  await page.evaluate(() => window.__test.setIr("notes"));
  await until(page, () => document.querySelector("#app").textContent.includes("Apples: 0"), null, "the notes page");
  const field = '#app [data-part="field.input"]';
  await page.fill(field, "call at 5");
  await page.click('#app [data-xtmd-widget="core/press"]');
  await page.click('#app [data-xtmd-widget="core/press"]');
  await until(page, () => document.querySelector("#app").textContent.includes("Apples: 2"), null, "the tally's own count");
  assert.equal(await page.inputValue(field), "call at 5", "a redraw keeps what was typed");
  // what a person is composing (an IME) stays, though the field is drawn again under it, and is written once done
  const composing = (what, value) => page.evaluate(([sel, what, value]) => {
    const el = document.querySelector(sel);
    if (value !== undefined) el.value = value;
    el.dispatchEvent(new CompositionEvent(what, { bubbles: true }));
  }, [field, what, value]);
  await composing("compositionstart", "call at 5か");
  await page.evaluate(() => document.querySelector('#app [data-xtmd-widget="core/press"]').click());
  await until(page, () => document.querySelector("#app").textContent.includes("Apples: 3"), null, "a change while composing");
  assert.equal(await page.inputValue(field), "call at 5か", "what is being composed stays");
  await composing("compositionend", "call at 5 か");
  await until(page, () => Object.values(window.__test.handle().snapshot().own).some((o) => o.note?.text === "call at 5 か"), null, "the composed text to be written");
  await page.fill(field, "call at 5");
  await page.click('#app [data-part="overlay.anchor"]');
  await until(page, () => document.querySelector('#app [data-part="overlay.body"]')?.matches(":popover-open"), null, "the overlay to open");
  const snap = await page.evaluate(() => window.__test.handle().snapshot());
  // the release a snapshot is taken in is the packages' version
  assert.equal(snap.xtmd, JSON.parse(readFileSync(new URL("../../widgets/web/package.json", import.meta.url), "utf8")).version);
  assert.deepEqual(Object.values(snap.own).flatMap((o) => Object.entries(o)).sort(), [["k", { num: "3" }], ["note", { text: "call at 5" }], ["open", { bool: true }]].sort());
  await page.fill(field, "never mind");
  await page.click('#app [data-xtmd-widget="core/press"]');
  await until(page, () => document.querySelector("#app").textContent.includes("Apples: 4"), null, "one more click");
  const all = await page.evaluate((s) => window.__test.handle().restore(s), snap);
  assert.deepEqual(all, { restored: "all", dropped: [] });
  await until(page, () => document.querySelector("#app").textContent.includes("Apples: 3"), null, "the restored count");
  assert.equal(await page.inputValue(field), "call at 5", "the restored note");
  // from another IR, only $state can go back: its places may be other widgets now
  const other = await page.evaluate((s) => window.__test.handle().restore({ ...s, ir: "another", state: { n: { num: "7" } } }), snap);
  assert.deepEqual(other, { restored: "state", dropped: ["own"] });
  await until(page, () => document.querySelector("#app").textContent.includes("Notes: 7"), null, "the restored $state");
  // one this xtmd can't read: nothing restored
  assert.equal((await page.evaluate(() => window.__test.handle().restore({ xtmd: "9.9.9" }))).restored, "none");

  // gone: unmounted, and nothing left in the element
  await page.evaluate(() => window.__test.unmount());
  await until(page, () => document.querySelector("#app [data-xtmd-site]") === null, null, "the unmount");
}

/** The test pages' text, a broken draft of the first among them, for the live documents. */
export function pageSources() {
  const v1 = readFileSync(pages.v1, "utf8");
  return { v1, v2: readFileSync(pages.v2, "utf8"), broken: v1.replace("# Count: \\{$state.n}", "# Count: \\{$state.nope}") };
}

/** Where a live test host finds xtmd.wasm: `window.__WASM_URL`, answered by the tab itself. */
export const WASM_URL = "http://xtmd.test/xtmd.wasm";
const wasmFile = join(here, "../../compiler/npm/wasm/xtmd.wasm");

/** A tab for a live document's test host: the sources as `window.__SRC`, xtmd.wasm served at
 * `window.__WASM_URL`, and the bundle run in it. */
export async function openLive(browser, { js }) {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e));
  const wasm = readFileSync(wasmFile);
  await page.route(WASM_URL, (r) => r.fulfill({ status: 200, contentType: "application/wasm", headers: { "access-control-allow-origin": "*" }, body: wasm }));
  await page.setContent(`<!doctype html><html><head><meta charset="utf-8"></head><body><div id="app"></div></body></html>`);
  await page.evaluate(([src, url]) => {
    window.__SRC = src;
    window.__WASM_URL = url;
  }, [pageSources(), WASM_URL]);
  await page.addScriptTag({ content: js });
  await page.waitForFunction(() => window.__test !== undefined, null, { timeout: 10000 });
  return { page, errors };
}

/**
 * The steps every live document's <XtmdLiveDoc> passes, through the test host's `window.__test`:
 * - `heard`: { states, errors, calls }, from onStateChange, onError and onCalls;
 * - `handle()`: the component's handle;
 * - `setSource(text)`: gives it a new `source`, starting from `window.__SRC.v1`, with
 *   `externs.app.unit` "items" and `wasm` `window.__WASM_URL`.
 */
export async function liveSteps(page) {
  const app = () => page.evaluate(() => document.querySelector("#app")?.textContent ?? "");
  // compiled in the tab, then drawn
  await until(page, () => document.querySelector("#app")?.textContent?.includes("Count: 0 items"), null, "the text to compile and render");
  assert.equal(await page.evaluate(() => document.querySelectorAll("[data-xtmd-root]").length), 1, "one page drawn");
  await page.click('#app [data-xtmd-widget="core/press"]');
  await until(page, () => document.querySelector("#app").textContent.includes("Count: 1 items"), null, "the click");
  assert.deepEqual((await page.evaluate(() => window.__test.heard.states)).at(-1), { n: { num: "1" } });

  // an edit that compiles: the new page, with $state kept where its fields still exist
  await page.evaluate(() => window.__test.setSource(window.__SRC.v2));
  await until(page, () => document.querySelector("#app").textContent.includes("Total: 1 items, by 2"), null, "the edited text");

  // a draft that doesn't compile: the last page stays, and its errors reach onError
  await page.evaluate(() => window.__test.setSource(window.__SRC.broken));
  await until(page, () => window.__test.heard.errors.some((e) => e.kind === "compile"), null, "the compile error");
  const e = (await page.evaluate(() => window.__test.heard.errors)).find((x) => x.kind === "compile");
  assert.match(e.message, /^page\.xtmd doesn't compile \(\d+ errors?\): page\.xtmd:\d+:\d+: /);
  assert.ok(e.diagnostics.some((d) => d.severity === "error" && d.file === "page.xtmd"), JSON.stringify(e.diagnostics));
  assert.ok((await app()).includes("Total: 1 items, by 2"), "a broken draft leaves the last page");
  assert.ok(await page.evaluate(() => document.querySelector('#app [data-xtmd-widget="core/press"]') !== null), "and it still works");

  // fixed: the page follows again, and the handle reaches it
  await page.evaluate(() => window.__test.setSource(window.__SRC.v1));
  await until(page, () => document.querySelector("#app").textContent.includes("Count: 1 items"), null, "the fixed text");
  await page.evaluate(() => window.__test.handle().set({ n: { num: "3" } }));
  await until(page, () => document.querySelector("#app").textContent.includes("Count: 3 items"), null, "the handle's write");
  assert.equal(await page.evaluate(() => window.__test.handle().markdown()), "# Count: 3 items\n\nAdd one\n");
}

/** What an app with `components` for `core/press` (a `button.mine`) and `core/heading` (a `section.title`
 * around its sentence) shows and does: the host's own components, on the host's contract. */
export async function componentSteps(page) {
  await until(page, () => document.querySelector("section.title")?.textContent.includes("Count: 0 items"), null, "the heading's own component, its sentence in it");
  assert.equal(await page.evaluate(() => document.querySelectorAll("button.mine").length), 1);
  assert.equal(await page.evaluate(() => document.querySelector("button.mine").textContent), "Add one");
  assert.equal(await page.evaluate(() => document.querySelector("button.mine").closest("[data-xtmd-widget]").getAttribute("data-xtmd-widget")), "core/press");
  await page.evaluate(() => (document.querySelector("section.title").dataset.kept = "yes"));
  await page.click("button.mine");
  await until(page, () => document.querySelector("section.title")?.textContent.includes("Count: 1 items"), null, "an event from a component");
  assert.equal(await page.evaluate(() => document.querySelector("section.title").dataset.kept), "yes", "the component is kept");
  await page.evaluate(() => window.__test.unmount());
  await until(page, () => document.querySelector("button.mine") === null, null, "the unmount");
}
