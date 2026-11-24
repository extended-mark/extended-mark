// What a page does, case by case: each is built, changed by its steps, and looked at after each. `settle`
// lays its planes out, when whoever runs it can, with fixed sizes (sizes.mjs), so a case says exactly
// where a spot lands.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { HostPage, Num, coreOf, pageBlocks, toIr, childrenOf } from "../dist/internal.js";
import { engineFns } from "./sizes.mjs";
import { xtmdBinary, workdir } from "./xtmd.mjs";

const here = join(dirname(fileURLToPath(import.meta.url)), "cases");
const dir = workdir();
let n = 0;

/** Each case, and whether its page has planes to lay out. */
export const cases = readdirSync(here)
  .filter((x) => x.endsWith(".json"))
  .map((f) => JSON.parse(readFileSync(join(here, f), "utf8")))
  .map((c) => ({ ...c, planes: /@(plane|graph)\b/.test(readFileSync(join(here, c.source), "utf8")) }));

function boxes(nodes, out = {}) {
  for (const d of nodes) {
    const r = d.widget === "core/spot" && d.props.get("box");
    const fields = r?.kind === "opt" && r.value ? toIr(r.value).record.fields : undefined;
    if (fields) out[d.path] = Object.fromEntries(["from", "to"].map((k) => [k, { x: fields[k].record.fields.x, y: fields[k].record.fields.y }]));
    for (const kids of childrenOf(d)) boxes(kids, out);
  }
  return out;
}

/** A shader's name: a named fn is lowered to a lambda that calls it with its own parameters. */
function shaderName(f) {
  const call = f.kind === "fn" && f.body.length === 1 ? f.body[0].return?.value?.call : undefined;
  return call && (call.args ?? []).length === f.params.length && (call.args ?? []).every((a, k) => a.get?.local === f.params[k].name) ? call.name : "";
}

/** Every shade drawn, by path: its shader's name, its arguments, and the colour the shader works out at the
 * area's two corners and its middle. */
function shaded(page) {
  const out = {};
  const num = (n) => ({ kind: "num", n });
  const point = (x, y) => ({ kind: "record", ty: "core/point", fields: [["x", num(x)], ["y", num(y)]] });
  const of = (v, k) => v.fields.find(([f]) => f === k)[1];
  const walk = (nodes) => {
    for (const n of nodes) {
      if (n.widget === "core/shade") {
        const [fn, args, area] = ["shader", "args", "area"].map((k) => n.props.get(k));
        const entry = (out[n.path] = { shader: fn ? shaderName(fn) : "", args: args ? toIr(args) : { none: {} } });
        if (fn && area) {
          const [fx, fy, tx, ty] = [["from", "x"], ["from", "y"], ["to", "x"], ["to", "y"]].map(([p, k]) => of(of(area, p), k).n);
          const half = Num.parse("0.5");
          const px = num(tx.sub(fx).abs().div(Num.int(64)));
          entry.samples = [point(fx, fy), point(fx.add(tx).mul(half), fy.add(ty).mul(half)), point(tx, ty)].map((at) => {
            page.machine.resetSteps();
            return { at: toIr(at), pixel: toIr(page.machine.apply(fn, [{ value: at }, { value: px }, { value: args }])) };
          });
        }
      }
      for (const kids of childrenOf(n)) walk(kids);
    }
  };
  walk(page.tree());
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
        within(w, shaded(page), `${at} shaded`);
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

export function run(c, settle = () => {}) {
  const packs = (c.packs ?? []).map((p) => join(here, p));
  const out = join(dir, `case${n++}.ir.json`);
  execFileSync(xtmdBinary(), ["build", join(here, c.source), ...(packs.length ? ["--pack", packs.join(",")] : []), "--out", out], { stdio: ["ignore", "pipe", "pipe"] });
  let page;
  const supplies = (w) => w.startsWith("core/") || w === "badge/badge";
  const host = new HostPage(JSON.parse(readFileSync(out, "utf8")), { page: c.page ?? {}, state: c.state, externs: c.externs, fns: engineFns(() => page), supplies });
  page = coreOf(host);
  settle(host, page);
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
    settle(host, page);
    expect(page, s.expect, `step ${k + 1} (${what})`);
  });
}
