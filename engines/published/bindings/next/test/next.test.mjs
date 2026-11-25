import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { withXtmd, loader } from "@extended-mark/next/config";
import { pageHtml } from "@extended-mark/next/server";
import * as cli from "extended-mark";
import { build } from "@extended-mark/next/engine";
import { builtPages } from "../../testing/browser.mjs";

const counter = fileURLToPath(new URL("../../testing/counter.xtmd", import.meta.url));
const require = createRequire(import.meta.url);

/** Runs the loader as webpack and Turbopack do: an async loader with options. */
function runLoader(resourcePath, options = {}) {
  const load = require(loader);
  return new Promise((resolve, reject) => {
    const deps = [];
    load.call({
      resourcePath,
      getOptions: () => options,
      addDependency: (f) => deps.push(f),
      async: () => (err, out) => (err ? reject(err) : resolve({ out, deps })),
    });
  });
}

test("the loader turns a .xtmd file into a module whose default is its IR", async () => {
  const { out } = await runLoader(counter);
  assert.match(out, /^export default \{"ir":/);
  const ir = JSON.parse(out.slice("export default ".length, -1));
  assert.deepEqual(ir, await cli.build(counter));
});

test("a page that doesn't compile fails the build with xtmd's message", async () => {
  const broken = fileURLToPath(new URL("../../../../../compiler/npm/extended-mark/test/vite-broken.xtmd", import.meta.url));
  await assert.rejects(runLoader(broken), /doesn't compile/);
});

test("withXtmd gives the loader to Turbopack and to webpack, keeping the host's config", () => {
  const mine = (c) => ((c.marked = true), c);
  const config = withXtmd({ reactStrictMode: true, turbopack: { rules: { "*.svg": { loaders: ["svgr"] } } }, webpack: mine }, { packs: ["./packs/maps"] });
  assert.equal(config.reactStrictMode, true);
  assert.deepEqual(config.turbopack.rules["*.svg"], { loaders: ["svgr"] });
  assert.deepEqual(config.turbopack.rules["*.xtmd"], { loaders: [{ loader, options: { packs: ["./packs/maps"] } }], as: "*.js" });
  const wp = config.webpack({ module: { rules: [] } }, {});
  assert.equal(wp.marked, true);
  assert.equal(wp.module.rules.length, 1);
  assert.ok(wp.module.rules[0].test.test("shop.xtmd"));
  assert.ok(!wp.module.rules[0].test.test("README.md"));
});

test("pageHtml writes the page's Markdown as HTML, with the host's $state", async () => {
  const ir = await cli.build(counter);
  assert.equal(pageHtml(ir), "<h1>Count: 0 points</h1>\n<p>Add one</p>");
  assert.equal(pageHtml(ir, { state: { n: { num: "7" } } }), "<h1>Count: 7 points</h1>\n<p>Add one</p>");
  // a value that doesn't fit keeps the starting one
  assert.equal(pageHtml(ir, { state: { n: { num: "70" } } }), "<h1>Count: 0 points</h1>\n<p>Add one</p>");
  // a channel by the name the document gives it
  assert.equal(pageHtml(ir, { externs: { app: { unit: { text: "pts" } } } }), "<h1>Count: 0 pts</h1>\n<p>Add one</p>");
});

test("a page the engine built, in a server component of a Next.js app: `next build` writes its HTML", async () => {
  const app = fileURLToPath(new URL("./app", import.meta.url));
  await builtPages(build, { v1: await cli.build(counter) }, `${app}/built`);
  execFileSync(process.execPath, [require.resolve("next/dist/bin/next"), "build"], { cwd: app, stdio: "pipe", env: { ...process.env, NEXT_TELEMETRY_DISABLED: "1" } });
  const html = readFileSync(`${app}/.next/server/app/index.html`, "utf8");
  assert.ok(html.replace(/<!-- -->/g, "").includes("Count: 2 items"), html.slice(0, 2000));
});
