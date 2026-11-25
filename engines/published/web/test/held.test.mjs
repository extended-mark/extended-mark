// What a framework's component holds: every document of ours, run from its IR and as the engine built it for a
// component, given the same props and the same random changes. What a renderer is given to render, the page's data and
// what its host hears are the same after each.

import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import { Doc } from "@extended-mark/widgets/doc";
import { everyNode } from "@extended-mark/runtime";
import { toJson } from "@extended-mark/widgets";
import { buildComponents } from "../dist/components.js";
import { changeOf, random } from "./changes.mjs";
import { documents, irOf, repo, source } from "./side.mjs";

const packs = ["checks/packs/badge", "examples/graphs"].map((p) => join(repo, p));
const folders = ["checks", "examples", "apps", "engines/published/runtime/test/cases", "engines/published/bindings/testing", "engines/published/web/test/pages", "engines/examples/misc/android/compose/sample/pages"];
const plain = { name: "test", external: [], require: createRequire(import.meta.url), code: () => "export const make=(p)=>new BuiltDoc(built,p);", files: () => [] };

/** Documents built, as what makes each. */
async function built(documents, options = {}) {
  const dir = mkdtempSync(join(tmpdir(), "xtmd-held-"));
  const { files } = await buildComponents({ version: 1, documents }, options, plain);
  for (const f of files) {
    mkdirSync(dirname(join(dir, f.path)), { recursive: true });
    writeFileSync(join(dir, f.path), f.text);
  }
  writeFileSync(join(dir, "package.json"), '{"type":"module"}');
  return Promise.all(documents.map(async (d) => (await import(pathToFileURL(join(dir, `${d.name}.js`)).href)).make));
}

const idle = () => new Promise((ok) => setImmediate(ok));
const numbers = (s) => s.replace(/xtmdv\d+-/g, "xtmdv-").replace(/\[\d+\]/g, "[#]");

/** Both ways of one page, and what each host heard. */
function both(ir, make, props = {}) {
  const side = (of) => {
    const heard = [];
    const doc = of({ ...props, onStateChange: (s) => heard.push({ state: s }), onError: (e) => heard.push({ error: e }), onCalls: (c) => heard.push({ calls: c }) });
    return { doc, heard, seen: () => JSON.parse(numbers(JSON.stringify({ drawn: toJson(doc.render()), snapshot: doc.snapshot(), heard: heard.splice(0) }))) };
  };
  return [side((p) => new Doc({ ir, ...p })), side(make)];
}

function act({ doc }, change, at) {
  const [[kind, x]] = Object.entries(change);
  const page = doc.held;
  const id = x.at === undefined ? undefined : at[x.at];
  const name = (v) => v?.case.name;
  if (kind === "event") page.event(id, x.name, x.args);
  else if (kind === "write") page.write(id, x.input, x.value);
  else if (kind === "set") doc.set(x.fields);
  else if (kind === "expose") doc.expose(x.channel, x.name, ...(x.args ?? []));
  else if (kind === "extern") page.setExtern(x.channel, x.values);
  else if (kind === "page") page.setPage({ width: name(x.values.width), theme: name(x.values.theme) });
  else page.restore(x.data);
  doc.report();
}

async function same(id, ir, make, steps = 20) {
  const [a, b] = both(ir, make);
  await idle();
  assert.deepEqual(b.seen(), a.seen(), `${id}, as it starts`);
  const past = [];
  for (let k = 0; k < steps; k++) {
    const rnd = random(id.length * 7919 + k * 31);
    const nodes = everyNode(a.doc.held.tree().nodes).map((n) => n.id);
    const theirs = everyNode(b.doc.held.tree().nodes).map((n) => n.id);
    if (rnd() < 0.3) past.push(a.doc.held.data());
    const change = changeOf(rnd, ir, { state: a.doc.held.state(), nodes: a.doc.held.tree().nodes, ids: nodes, past });
    act(a, change, nodes);
    act(b, change, theirs);
    await idle();
    assert.deepEqual(b.seen(), a.seen(), `${id}, after change ${k + 1}: ${JSON.stringify(change)}`);
  }
}

test("every document of ours is held as the runtime's page is, whatever is done to it", async () => {
  const was = new Set();
  const all = [];
  for (const f of folders.flatMap((d) => documents(join(repo, d)))) {
    const ir = irOf(f, packs) ?? irOf(f);
    if (!ir) continue;
    const h = createHash("md5").update(JSON.stringify(ir)).digest("hex");
    if (was.has(h)) continue;
    was.add(h);
    all.push({ name: `p${all.length}`, ir, id: f.slice(repo.length + 1) });
  }
  const makes = await built(all.map(({ name, ir }) => ({ name, ir })));
  const bad = [];
  for (const [k, d] of all.entries()) await same(d.id, d.ir, makes[k]).catch((e) => bad.push(e.message.slice(0, 1200)));
  assert.deepEqual(bad, []);
  assert.ok(all.length >= 70, `${all.length} pages`);
});

const COUNTER = join(repo, "engines/published/bindings/testing/counter.xtmd");
const n = (x) => ({ n: { num: x } });

test("props: $state controlled and refused, externs, the theme, snapshots of either way put back in the other", async () => {
  const ir = irOf(COUNTER);
  const [make] = await built([{ name: "counter", ir }]);
  const given = { state: n("2"), externs: { app: { unit: { text: "items" } } }, page: { theme: "DARK" } };
  const [a, b] = both(ir, make, given);
  await idle();
  assert.deepEqual(b.seen(), a.seen());
  for (const next of [{ ...given, state: n("99") }, { ...given, state: n("5") }, { ...given, state: n("5"), externs: { app: { unit: { text: "pts" } } } }, { ...given, page: { theme: "LIGHT" } }]) {
    for (const s of [a, b]) s.doc.update({ ...next, onStateChange: (x) => s.heard.push({ state: x }), onError: (e) => s.heard.push({ error: e }) });
    await idle();
    assert.deepEqual(b.seen(), a.seen(), JSON.stringify(next));
  }
  assert.deepEqual(b.doc.restore(a.doc.snapshot()), { restored: "all", dropped: [] });
  assert.deepEqual(a.doc.restore({ ...b.doc.snapshot(), state: n("7") }), { restored: "all", dropped: [] });
  assert.deepEqual(a.doc.state(), n("7"));
  const [c, d] = both(ir, make, { defaultSnapshot: a.doc.snapshot() });
  await idle();
  assert.deepEqual(d.seen(), c.seen());
  assert.deepEqual(d.doc.state(), n("7"));
  assert.equal(b.doc.restore({ xtmd: "9.9.9" }).restored, "none");
});

test("a pack's widget is the host's component, when the engine is told it is", async () => {
  const ir = source("import badge;\n@@@\n@badge(label = \"x\");\n", [join(repo, "checks/packs/badge")]);
  const widget = ir.declarations.map((d) => d.declare.extern?.widget?.name).find((w) => w && !w.startsWith("core/"));
  const [make] = await built([{ name: "badge", ir }], { components: [widget] });
  const [a, b] = both(ir, make, { components: { [widget]: () => null } });
  await idle();
  assert.deepEqual(b.seen(), a.seen());
  assert.ok(JSON.stringify(toJson(b.doc.render())).includes(`"component":"${widget}"`));
});
