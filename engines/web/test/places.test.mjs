// The places to plug in: a widget set, token values, the page around a page, and the host's fns; and the engine as
// `xtmd export --engine` runs it.

import { after, before, test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "../dist/index.js";
import { repo, source, start } from "./side.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const place = (f) => join(here, "places", f);
const pack = join(repo, "checks/packs/badge");
let side;
before(async () => {
  side = await start();
});
after(() => side?.close());

const COUNT = '$state {\n    n: int 0..10 = 0;\n}\n\nwidget c() {\n    view {\n        @press(label = "Add", click = fn() { $state.n = ($state.n + 1).clamp(0, 10); });\n        @label(value = "Count \\{$state.n}", tone = color.ACCENT);\n    }\n    markdown { }\n}\n@@@\n# Count\n\n@c;\n';
const one = async (ir, options) => (await build({ version: 1, documents: [{ name: "page", ir }] }, options)).files;
const text = (files, path) => files.find((f) => (path instanceof RegExp ? path.test(f.path) : f.path === path))?.text;

test("a host's widget replaces ours by its full name, in the HTML and as it runs", async () => {
  const p = await side.open(source(COUNT), { options: { widgets: place("widgets.mjs") } });
  assert.match(p.shell.body, /<button data-part="mine"[^>]*>&gt; Add<\/button>/);
  await p.theirs.locator("button").click();
  assert.equal(await p.theirs.locator("main").innerText().then((t) => t.includes("Count 1")), true);
  assert.deepEqual(p.errors, []);
  // and the one it replaced isn't in the page
  assert.ok(!p.files.some((f) => f.path.endsWith(".js") && f.text.includes("press.indicator")));
  await p.close();
});

test("a pack's widget is drawn by whoever supplies it, and is its Markdown when nobody does", async () => {
  const ir = source("import badge;\n$state {\n    n: int 0..10 = 3;\n}\n@@@\n@badge(label = \"Votes\", count = $state.n);\n", [pack]);
  assert.match(text(await one(ir, { widgets: place("widgets.mjs") }), "page.html"), /<mark data-part="badge"[^>]*>Votes: 3<\/mark>/);
  const without = text(await one(ir, {}), "page.html");
  assert.ok(!without.includes("<mark") && without.includes("Votes"), without);
});

test("with the host's widgets only, none of ours is drawn or held", async () => {
  const files = await one(source(COUNT), { widgets: place("widgets.mjs"), only: true });
  assert.match(text(files, "page.html"), /data-part="mine"/);
  assert.ok(!text(files, "page.html").includes("xtmd-label"));
});

test("a token's value is the host's, in the stylesheet and nowhere else", async () => {
  const record = (light, dark) => ({ record: { type: "core/color_value", fields: { light: { text: light }, dark: { text: dark } } } });
  const files = await one(source(COUNT), { tokens: { "core/color.ACCENT": record("#123456", "#abcdef") } });
  const sheet = text(files, /^assets\/xtmd-t.*\.css$/);
  assert.ok(sheet.includes("--xtmd-color-accent:#123456") && sheet.includes("--xtmd-color-accent:#abcdef"), sheet);
  assert.ok(!sheet.includes("#2563eb"));
});

test("the page around a page is the host's", async () => {
  const files = await one(source(COUNT), { shell: (await import(place("shell.mjs"))).default });
  assert.match(text(files, "page.html"), /^<!doctype html><title>Count at ours<\/title><body class="site" data-name="page"><main /);
});

test("a fn of the host's is kept in the page, or given as it is mounted", async () => {
  const ir = source("extern host {\n    fn rate(n: int): int;\n}\n\n$state {\n    n: int 0..10 = 3;\n}\n@@@\nRated \\{host.rate($state.n)}.\n");
  const p = await side.open(ir, { options: { fns: place("fns.mjs") } });
  assert.match(await p.theirs.locator("main").innerText(), /Rated 6\./);
  await p.close();
});

test("pages built together share what is the same in them, and each has its own place", async () => {
  const ir = source(COUNT);
  const { files } = await build({ version: 1, documents: [{ name: "a", ir }, { name: "docs/b", ir: source(COUNT.replace("# Count", "# Other")) }] });
  assert.ok(files.some((f) => /^assets\/shared-/.test(f.path)));
  assert.match(text(files, "docs/b.html"), /href="\.\.\/assets\/xtmd\.css"/);
  assert.match(text(files, "docs/b.html"), /src="\.\.\/assets\/docs\/b-[A-Z0-9]+\.js"/);
  for (const f of files.filter((f) => f.path.endsWith(".js"))) for (const [, from] of f.text.matchAll(/from"(\.[^"]+)"/g)) assert.ok(files.some((g) => g.path === join(dirname(f.path), from)), `${f.path} takes ${from}`);
});

test("xtmd export --engine runs it", () => {
  const dir = mkdtempSync(join(tmpdir(), "xtmd-web-cli-"));
  writeFileSync(join(dir, "xtmd.toml"), `[engines]\nweb = ["node", ${JSON.stringify(join(here, "../dist/cli.js"))}]\n`);
  writeFileSync(join(dir, "page.xtmd"), COUNT);
  const bin = ["release", "debug"].map((k) => join(repo, "compiler/target", k, "xtmd")).find(existsSync);
  execFileSync(bin, ["export", "page.xtmd", "--out", "dist", "--engine", "web", "--shell", place("shell.mjs"), "--widgets", place("widgets.mjs")], { cwd: dir, stdio: "pipe" });
  assert.match(readFileSync(join(dir, "dist/page.html"), "utf8"), /at ours.*data-part="mine"/s);
  assert.ok(readdirSync(join(dir, "dist/assets")).some((f) => f.endsWith(".js")));
  const bad = spawnSync(bin, ["export", "page.xtmd", "--out", "dist2", "--engine", "web", "--nothing"], { cwd: dir, encoding: "utf8" });
  assert.notEqual(bad.status, 0);
  assert.match(bad.stderr, /xtmd-web has no --nothing/);
  assert.ok(!existsSync(join(dir, "dist2")));
});
