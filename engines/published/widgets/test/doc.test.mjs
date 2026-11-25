// A document held without a framework or a browser: the page as HTML, and the document's controlled `$state`, refusals and snapshots.

import { test } from "node:test";
import assert from "node:assert/strict";
import * as cli from "extended-mark";
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { toHtml, toJson } from "../dist/index.js";
import { Doc } from "../dist/doc.js";
import { fits } from "../../runtime/test/fits.mjs";
import { compilePages } from "../../bindings/testing/browser.mjs";

const irs = await compilePages(cli);
const tick = () => new Promise((r) => setTimeout(r, 0));
const num = (n) => ({ num: String(n) });
const externs = { app: { unit: { text: "items" } } };

/** Every drawn widget of a page, slots included. */
function drawn(nodes, out = []) {
  for (const d of nodes) {
    out.push(d);
    for (const x of Object.values(d.inputs)) if (x.nodes) drawn(x.nodes, out);
  }
  return out;
}

test("a document renders its page with the host's externs and the `$state` it starts from", () => {
  const doc = new Doc({ ir: irs.v1, externs, defaultState: { n: num(2) } });
  const html = toHtml(doc.render());
  assert.match(html, /^<div class="xtmd-root[^"]*" data-xtmd-root data-xtmd-theme="light"/);
  assert.ok(html.includes("Count: 2 items"), html);
  assert.match(html, /data-part="press\.label"[^>]*>Add one</);
  assert.equal(doc.markdown(), "# Count: 2 items\n\nAdd one\n");
});

test("a controlled `$state` is checked: what fits is shown and heard once, what doesn't is refused and the page keeps the last that fit", async () => {
  const errors = [];
  const states = [];
  const props = { ir: irs.v1, externs, onError: (e) => errors.push(e), onStateChange: (s) => states.push(s) };
  // one the page refuses as it starts: told, and the page starts from its own
  const doc = new Doc({ ...props, state: { n: num(11) } });
  assert.equal(errors.length, 1);
  assert.equal(errors[0].kind, "refused");
  assert.deepEqual(doc.state(), { n: num(0) });
  // one that fits: shown, and heard once
  doc.update({ ...props, state: { n: num(3) } });
  await tick();
  assert.deepEqual(doc.state(), { n: num(3) });
  assert.deepEqual(states, [{ n: num(3) }]);
  // the same again is no change
  doc.update({ ...props, state: { n: num(3) } });
  await tick();
  assert.equal(states.length, 1);
  // one out of range: refused once, and 3 stays
  doc.update({ ...props, state: { n: num(12) } });
  await tick();
  assert.equal(errors.length, 2);
  assert.deepEqual(doc.state(), { n: num(3) });
  assert.equal(states.length, 1);
});

test("set, expose and the host's extern fns go through the page, each heard once its change is done", async () => {
  const states = [];
  const calls = [];
  const doc = new Doc({ ir: irs.v1, externs, onStateChange: (s) => states.push(s), onCalls: (c) => calls.push(...c) });
  let renders = 0;
  doc.subscribe(() => renders++);
  doc.set({ n: num(4) });
  await tick();
  assert.deepEqual(states, [{ n: num(4) }]);
  assert.ok(renders >= 1);
  // the reader's press: its handler writes $state and calls app.saved
  const press = drawn(doc.running.tree().nodes).find((d) => d.widget === "core/press");
  doc.running.event(press.id, "click", []);
  await tick();
  assert.deepEqual(states.at(-1), { n: num(5) });
  assert.equal(calls.length, 1);
  assert.match(JSON.stringify(calls[0]), /saved/);
  doc.expose("app", "reset");
  await tick();
  assert.deepEqual(states.at(-1), { n: num(0) });
});

test("a snapshot brings back `$state` and widgets' own variables in the same IR, `$state` alone in another, and nothing it can't read", async () => {
  const doc = new Doc({ ir: irs.notes });
  doc.set({ n: num(2) });
  const tally = drawn(doc.running.tree().nodes).find((d) => d.widget === "core/press");
  doc.running.event(tally.id, "click", []);
  doc.running.event(tally.id, "click", []);
  const snap = JSON.parse(JSON.stringify(doc.snapshot()));
  assert.deepEqual(snap.state, { n: num(2) });
  assert.ok(Object.values(snap.own).some((o) => JSON.stringify(o.k) === JSON.stringify(num(2))), JSON.stringify(snap.own));
  // the same IR: all of it
  const again = new Doc({ ir: irs.notes });
  assert.deepEqual(again.restore(snap), { restored: "all", dropped: [] });
  assert.deepEqual(again.state(), { n: num(2) });
  assert.ok(again.markdown().includes("Apples: 2"), again.markdown());
  // started from it
  const started = new Doc({ ir: irs.notes, defaultSnapshot: snap });
  assert.ok(started.markdown().includes("Apples: 2"), started.markdown());
  assert.deepEqual(started.snapshot(), snap);
  // and another IR's starts $state only, which this page refuses for what doesn't fit
  const refused = [];
  assert.deepEqual(new Doc({ ir: irs.v1, externs, defaultSnapshot: { ...snap, state: { n: num(11) } }, onError: (e) => refused.push(e.kind) }).state(), { n: num(0) });
  assert.deepEqual(refused, ["refused"]);
  // another IR: $state only, as its places may be other widgets
  const other = new Doc({ ir: irs.v1, externs });
  assert.deepEqual(other.restore({ ...snap, state: { n: num(2) } }), { restored: "state", dropped: ["own"] });
  assert.deepEqual(other.state(), { n: num(2) });
  // a snapshot of another release, or a $state the page refuses
  assert.equal(other.restore({ ...snap, xtmd: "0.2.0" }).restored, "none");
  const errors = [];
  const strict = new Doc({ ir: irs.v1, externs, onError: (e) => errors.push(e) });
  assert.equal(strict.restore({ ...snap, state: { n: num(11) } }).restored, "none");
  assert.equal(errors[0].kind, "refused");
  assert.deepEqual(strict.state(), { n: num(0) });
});

test("a document the host has no IR for yet shows that it waits, then the page", () => {
  const doc = new Doc({ externs });
  assert.match(toHtml(doc.render()), /data-xtmd-loading/);
  assert.equal(doc.running, undefined);
  doc.update({ externs, ir: irs.v1 });
  assert.ok(toHtml(doc.render()).includes("Count: 0 items"), toHtml(doc.render()));
});

test("an IR is told apart by its content: the same page given again goes on", async () => {
  const doc = new Doc({ ir: irs.v1, externs });
  const press = drawn(doc.running.tree().nodes).find((d) => d.widget === "core/press");
  doc.running.event(press.id, "click", []);
  const page = doc.running;
  doc.update({ ir: structuredClone(irs.v1), externs });
  assert.equal(doc.running, page);
  assert.deepEqual(doc.state(), { n: num(1) });
  doc.update({ ir: irs.v2, externs });
  assert.notEqual(doc.running, page);
});

test("what each widget draws fits the widget set's format", () => {
  const repo = join(dirname(fileURLToPath(import.meta.url)), "../../../..");
  const xtmd = ["release", "debug"].map((k) => join(repo, "compiler/target", k, "xtmd")).find(existsSync);
  const dir = join(repo, "engines/published/runtime/test/cases");
  const seen = new Set();
  const walk = (x, f) => (f(x), (x.children ?? []).forEach((c) => c.element && walk(c.element, f)));
  for (const f of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
    const c = JSON.parse(readFileSync(join(dir, f), "utf8"));
    let file;
    try {
      file = JSON.parse(execFileSync(xtmd, ["build", join(dir, c.source), ...(c.packs ?? []).flatMap((p) => ["--pack", join(dir, p)])], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }));
    } catch {
      continue; // a case of a page that doesn't compile
    }
    const doc = new Doc({ ir: file });
    const root = toJson(doc.render());
    assert.equal(fits(root, "web", "drawn"), undefined, c.id);
    walk(root.element, (e) => e.attrs?.["data-xtmd-widget"] && seen.add(e.attrs["data-xtmd-widget"]));
    // every listener and hook is named, with nothing of its code
    assert.deepEqual(JSON.parse(JSON.stringify(root)), root, c.id);
  }
  assert.ok(seen.size > 25, [...seen].join(" "));
});
