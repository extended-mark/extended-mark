// What a host gives a page: $state that fits and what is refused, and the page's Markdown as HTML.

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { blocksHtml, pageHtml } from "../dist/html.js";
import { coreOf } from "../dist/host.js";
import { makePage } from "../dist/index.js";
import { repo, workdir, xtmdBinary } from "./xtmd.mjs";

const pages = join(repo, "renderers/testing");
const built = (name) => {
  const out = join(workdir(), `given-${name}.json`);
  execFileSync(xtmdBinary(), ["build", join(pages, name), "--out", out], { stdio: ["ignore", "pipe", "pipe"] });
  return JSON.parse(readFileSync(out, "utf8"));
};
const irs = { v1: built("counter.xtmd"), v2: built("counter-v2.xtmd") };

test("makePage keeps the host's $state that fits, and says what it refused", () => {
  const ok = makePage(irs.v2, {}, { n: { num: "4" }, stride: { num: "3" } });
  assert.deepEqual(ok.page.state(), { n: { num: "4" }, stride: { num: "3" } });
  assert.deepEqual(ok.outcome, { refused: [], failures: [] });

  const some = makePage(irs.v2, {}, { n: { num: "40" }, stride: { num: "3" }, gone: { num: "1" } });
  assert.deepEqual(some.page.state(), { n: { num: "0" }, stride: { num: "3" } });
  assert.deepEqual(some.outcome.refused, [
    { code: "contracts.outside", site: "$state.n" },
    { code: "contracts.outside", site: "$state.gone" },
  ]);
  // a remount drops fields the new page doesn't have without a word
  assert.deepEqual(makePage(irs.v1, {}, { n: { num: "2" }, stride: { num: "3" } }, false).outcome.refused, []);
});

test("a page is drawn once, whatever $state it is given, and a host that draws its widgets adds no draw", () => {
  const draws = (given) => {
    const { page } = makePage(irs.v2, { supplies: () => true }, given);
    page.setSupplies((w) => w.startsWith("core/"));
    return coreOf(page).draws;
  };
  assert.equal(draws(undefined), 1);
  assert.equal(draws({ n: { num: "4" }, stride: { num: "3" } }), 1);
  assert.equal(draws({ n: { num: "40" }, stride: { num: "3" } }), 1);
});

test("pageHtml writes the page's Markdown as HTML, escaped", () => {
  assert.equal(pageHtml(irs.v1, { state: { n: { num: "5" } } }), "<h1>Count: 5 points</h1>\n<p>Add one</p>");
  assert.equal(
    blocksHtml([
      { paragraph: [{ text: { value: "<b>&", bold: true } }, { text: { value: "x", link: "javascript:alert(1)" } }, { text: { value: "y", link: "https://e.org/?a=1&b" } }] },
      { bullets: { ordered: true, start: 3, items: [{ done: true, blocks: [{ paragraph: [{ code: "a<b" }] }] }] } },
      { table: { head: [[{ text: { value: "A" } }]], rows: [[[{ math: "x^2" }]]], align: ["END"] } },
      { divider: {} },
    ]),
    '<p><strong>&lt;b&gt;&amp;</strong>x<a href="https://e.org/?a=1&amp;b">y</a></p>\n' +
      '<ol start="3"><li><input type="checkbox" disabled checked> <code>a&lt;b</code></li></ol>\n' +
      '<table><thead><tr><th style="text-align:right">A</th></tr></thead><tbody><tr><td style="text-align:right"><span class="xtmd-math">x^2</span></td></tr></tbody></table>\n' +
      "<hr>",
  );
});
