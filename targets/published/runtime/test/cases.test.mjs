// What a page does, case by case: each is built, changed by its steps, and looked at after each. The
// planes are laid out with fixed sizes (sizes.mjs), so a case says exactly where a spot lands.

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { Page, layout, pageBlocks, regionOf, toIr } from "../dist/index.js";
import { measure, targetFns } from "./sizes.mjs";
import { xtmdBinary, workdir } from "./xtmd.mjs";

const here = join(dirname(fileURLToPath(import.meta.url)), "cases");
const dir = workdir();
let n = 0;

function boxes(nodes, out = {}) {
  for (const d of nodes) {
    const r = d.widget === "core/spot" && regionOf(d.props.get("box"));
    if (r) out[d.path] = Object.fromEntries(["from", "to"].map((k) => [k, { x: toIr({ kind: "num", n: r[k].x }), y: toIr({ kind: "num", n: r[k].y }) }]));
    for (const kids of d.slots.values()) boxes(kids, out);
  }
  return out;
}

/** What a step says, and nothing it doesn't. */
function within(want, got, at) {
  if (Array.isArray(want) || want === null || typeof want !== "object") return assert.deepEqual(got, want, at);
  for (const [k, w] of Object.entries(want)) {
    assert.ok(got && k in got, `${at}.${k} is missing`);
    within(w, got[k], `${at}.${k}`);
  }
}

function expect(page, want, at) {
  // what a page reports since the last look is taken at every step, looked at or not
  const [calls, outcome] = [page.takeCalls(), page.takeOutcome()];
  for (const [k, w] of Object.entries(want ?? {})) {
    switch (k) {
      case "diagnostics":
        break;
      case "tree":
        for (const [site, d] of Object.entries(w)) within(d, page.observeTree()[site], `${at} ${site}`);
        break;
      case "state":
        within(w, page.observeState(), `${at} state`);
        break;
      case "calls":
        assert.deepEqual(calls, w, `${at} calls`);
        break;
      case "outcome":
        within(w, outcome, `${at} outcome`);
        break;
      case "boxes":
        assert.deepEqual(boxes(page.tree()), { ...boxes(page.tree()), ...w }, `${at} boxes`);
        break;
      case "shaded":
        within(w, page.observeShaded(true), `${at} shaded`);
        break;
      case "markdown":
        if (w.blocks) assert.deepEqual(JSON.parse(JSON.stringify(pageBlocks(page))), w.blocks, `${at} Markdown`);
        else if (w.has) for (const b of w.has) assert.ok(JSON.parse(JSON.stringify(pageBlocks(page))).some((x) => isDeepStrictEqual(x, b)), `${at} Markdown has ${JSON.stringify(b)}`);
        else if (w.file) assert.equal(page.markdown(), readFileSync(join(here, w.file), "utf8"), `${at} Markdown`);
        else assert.equal(page.markdown(), w, `${at} Markdown`);
        break;
      default:
        assert.fail(`${at}: nothing looks at ${k}`);
    }
  }
}

for (const f of readdirSync(here).filter((x) => x.endsWith(".json"))) {
  const c = JSON.parse(readFileSync(join(here, f), "utf8"));
  test(`${c.id}: ${c.about}`, () => {
    const packs = (c.packs ?? []).map((p) => join(here, p));
    const out = join(dir, `case${n++}.ir.json`);
    execFileSync(xtmdBinary(), ["build", join(here, c.source), ...(packs.length ? ["--pack", packs.join(",")] : []), "--out", out], { stdio: ["ignore", "pipe", "pipe"] });
    let page;
    const supplies = (w) => w.startsWith("core/") || w === "badge/badge";
    page = new Page(JSON.parse(readFileSync(out, "utf8")), { page: c.page ?? {}, state: c.state, externs: c.externs, fns: targetFns(() => page), supplies });
    const settle = () => layout(page, measure(page));
    settle();
    expect(page, c.expect, "at first");
    const v = (x) => page.value(x);
    const rec = (o) => Object.fromEntries(Object.entries(o ?? {}).map(([k, x]) => [k, v(x)]));
    c.steps.forEach((s, k) => {
      const [[what, d]] = Object.entries(s.do ?? { none: {} });
      if (what === "event") page.event(d.site, d.name, (d.args ?? []).map(v));
      else if (what === "write") page.write(d.site, d.input, v(d.value));
      else if (what === "set") page.set(rec(d.$state ?? d["$state"]));
      else if (what === "expose") page.expose(d.channel, d.name, (d.args ?? []).map(v));
      else if (what === "extern") page.setExtern(d.channel, rec(d.values));
      else if (what === "page") page.setPage(d);
      else if (what !== "none") assert.fail(`no step ${what}`);
      settle();
      expect(page, s.expect, `step ${k + 1} (${what})`);
    });
  });
}
