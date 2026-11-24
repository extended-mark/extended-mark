// The engine held to the runtime: every document of ours, run from its IR and as it was built, in two tabs of a real
// browser, with random changes from the reader and the host made to both. What is on the page, its data and what it
// tells are the same after each.

import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { documents, irOf, repo, source, start } from "./side.mjs";

const packs = ["checks/packs/badge", "examples/graphs"].map((p) => join(repo, p));
const folders = ["checks", "examples", "apps", "engines/published/runtime/test/cases", "engines/published/bindings/testing", "engines/published/web/test/pages", "engines/examples/misc/android/compose/sample/pages"];
let side;

before(async () => {
  side = await start();
});
after(() => side?.close());

const said = (d, errors) => {
  if (!d) return errors.join("\n");
  const at = Object.keys(d.b).flatMap((k) => {
    const [x, y] = [d.a[k], d.b[k]].map((v) => (typeof v === "string" ? v : JSON.stringify(v, null, 1)).split("\n"));
    const i = x.findIndex((l, n) => l !== y[n]);
    return i < 0 && x.length === y.length ? [] : [`${k}: the runtime has ${x[i]}\n   the engine ${y[i < 0 ? x.length : i]}`];
  });
  return `after change ${d.step ?? 0} (${JSON.stringify(d.change)}):\n${at.join("\n")}`;
};

/** A document's random runs: the first that differs is an error. */
async function same(ir, id, { seeds = 2, steps = 15, given, options } = {}) {
  const p = await side.open(ir, { given, options });
  try {
    for (let k = 0; k < seeds; k++) {
      const d = await p.random(id.length * 7919 + 3 + k * 104729, steps);
      assert.ok(!d && p.errors.length === 0, `${id}: ${said(d, p.errors)}`);
    }
    return p.shell.script !== undefined;
  } finally {
    await p.close();
  }
}

test("every document of ours ends as the runtime has it, whatever is done to it", async () => {
  const was = new Set();
  let [pages, moving] = [0, 0];
  const bad = [];
  for (const f of folders.flatMap((d) => documents(join(repo, d)))) {
    const ir = irOf(f, packs) ?? irOf(f);
    if (!ir) continue;
    const h = createHash("md5").update(JSON.stringify(ir)).digest("hex");
    if (was.has(h)) continue;
    was.add(h);
    try {
      if (await same(ir, f.slice(repo.length + 1))) moving++;
      pages++;
    } catch (e) {
      bad.push(e.message);
    }
  }
  assert.deepEqual(bad, []);
  assert.ok(pages > 90 && moving > 50, `${pages} pages, ${moving} with a script`);
});

test("what the host starts a page with is checked and drawn as the runtime does, over the HTML written before it", async () => {
  const ir = irOf(join(repo, "engines/published/web/test/pages/everything.xtmd"));
  const task = (title) => ({ record: { type: "doc/task", fields: { title: { text: title }, done: { bool: true } } } });
  for (const given of [
    { state: { n: { num: "7" }, tasks: { list: [task("from the host")] } } },
    { state: { n: { num: "99" }, tasks: { list: [] } } },
    { state: { n: { text: "no" }, nothing: { num: "1" } } },
    { externs: { "doc/host": { unit: { text: "pts" } } }, page: { theme: "DARK", width: "NARROW" } },
    { externs: { "doc/host": { unit: { num: "1" } } } },
  ]) await same(ir, `everything ${JSON.stringify(given)}`, { given, seeds: 1, steps: 10 });
});

test("a page's data taken out of one goes into another", async () => {
  const ir = irOf(join(repo, "engines/published/web/test/pages/everything.xtmd"));
  const one = await side.open(ir);
  await one.random(11, 12);
  const data = await one.theirs.evaluate(() => window.side.now().data);
  await one.close();
  const two = await side.open(ir, { given: { data } });
  const now = await two.theirs.evaluate(() => window.side.now().data);
  assert.deepEqual(now, data);
  assert.equal(await two.differ(), undefined);
  await two.close();
});

test("what never changes is HTML alone, and a page of it has no script", async () => {
  const p = await side.open(source("# Title\n\nA *page* of words and [a link](https://example.com).\n\n| a | b |\n|---|---|\n| 1 | 2 |\n"));
  assert.equal(p.shell.script, undefined);
  assert.deepEqual(p.files.map((f) => f.path).filter((f) => f.endsWith(".js")), []);
  assert.equal(await p.differ(), undefined);
  await p.close();
});

test("the elements written ahead are the ones the script follows, and what doesn't change is never touched", async () => {
  const ir = source('$state {\n    n: int 0..10 = 0;\n}\n\nwidget c() {\n    view {\n        @press(label = "Add", click = fn() { $state.n = ($state.n + 1).clamp(0, 10); });\n        @label(value = "Count \\{$state.n}");\n    }\n    markdown { }\n}\n@@@\n# Title\n\nBefore.\n\n@c;\n\nAfter.\n');
  const p = await side.open(ir, { options: {} });
  const marked = await p.theirs.evaluate(() => {
    const all = [...document.querySelectorAll("#page main > *")];
    all.forEach((e, k) => (e.mine = k));
    return all.map((e) => e.localName);
  });
  assert.deepEqual(marked, ["h1", "p", "button", "span", "p"]);
  await p.theirs.locator("button").click();
  assert.deepEqual(await p.theirs.evaluate(() => [...document.querySelectorAll("#page main > *")].map((e) => [e.mine, e.textContent])), [[0, "Title"], [1, "Before."], [2, "Add"], [3, "Count 1"], [4, "After."]]);
  assert.deepEqual(p.errors, []);
  // and nothing of the page's fixed words is in the script
  const script = p.files.find((f) => f.path === p.shell.script).text;
  assert.ok(!script.includes("Before.") && !script.includes("After."));
  await p.close();
});

test("nothing of the IR and no interpreter is in a page's script", async () => {
  const p = await side.open(irOf(join(repo, "examples/pages/shop.xtmd")));
  const js = p.files.filter((f) => f.path.endsWith(".js")).map((f) => f.text).join("\n");
  for (const word of ["fromModule", "this evaluator doesn't know", "declarations.find", "precision: 120"]) assert.ok(!js.includes(word), word);
  await p.close();
});

test("the cases' own steps give the same page, and what they say is told", async () => {
  const dir = join(repo, "engines/published/runtime/test/cases");
  for (const f of documents(dir)) {
    const c = JSON.parse(readFileSync(f.replace(/\.xtmd$/, ".json"), "utf8"));
    const ir = irOf(f, (c.packs ?? []).map((p) => join(dir, p)));
    if (!ir) continue;
    const p = await side.open(ir, { given: { state: c.state, externs: c.externs, page: c.page } });
    assert.equal(await p.differ(), undefined, c.id);
    for (const [k, s] of c.steps.entries()) {
      const [[what, d]] = Object.entries(s.do ?? { none: {} });
      const at = d.site && (await p.ours.evaluate((x) => [...document.querySelectorAll("[data-xtmd-site]")].findIndex((e) => e.getAttribute("data-xtmd-site") === x), d.site));
      if (d.site && at < 0) continue;
      const change = { event: () => ({ event: { at, name: d.name, args: d.args ?? [] } }), write: () => ({ write: { at, input: d.input, value: d.value } }), set: () => ({ set: { fields: d.$state } }), expose: () => ({ expose: { channel: d.channel, name: d.name, args: d.args ?? [] } }), extern: () => ({ extern: { channel: d.channel, values: d.values } }), page: () => ({ page: { values: Object.fromEntries(Object.entries(d).map(([n, name]) => [n, { case: { enum: `core/${n}`, name } }])) } }) }[what]?.();
      if (!change) continue;
      const diff = await p.differ(change);
      assert.ok(!diff, `${c.id}, step ${k + 1}: ${said(diff, p.errors)}`);
    }
    await p.close();
  }
});
