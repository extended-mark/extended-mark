// The runtime's submodules stand alone: numbers, the layout and the Markdown writer know nothing of a page,
// so each can be taken, tested and swapped without it. And the writer gives the compiler's text.

import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { writeMarkdown } from "@extended-mark/runtime/write";
import { Num } from "@extended-mark/runtime/num";
import { lay } from "@extended-mark/runtime/layout";
import { fits } from "./fits.mjs";
import { repo, xtmdBinary } from "./xtmd.mjs";

const dist = join(dirname(fileURLToPath(import.meta.url)), "../dist");

/** Every module a file loads, its own files by their path. */
function loads(file, out = new Set()) {
  for (const [, from] of readFileSync(join(dist, file), "utf8").matchAll(/^(?:import|export)\s[^;]*?from\s+"([^"]+)"/gm)) {
    const name = from.startsWith(".") ? join(dirname(file), from) : from;
    if (out.has(name)) continue;
    out.add(name);
    if (from.startsWith(".")) loads(name, out);
  }
  return [...out].sort();
}

test("numbers, the layout and the Markdown writer load nothing of the page", () => {
  assert.deepEqual(loads("eval/num.js"), []);
  assert.deepEqual(loads("layout.js"), ["eval/num.js"]);
  assert.deepEqual(loads("write.js"), []);
  // and the page draws with no writer
  assert.ok(!loads("page.js").some((m) => /write|markdown|layout/.test(m)), loads("page.js").join(" "));
});

test("each is taken by its own entry", () => {
  assert.equal(Num.parse("0.1").add(Num.parse("0.2")).show(), "0.3");
  assert.equal(lay({ boxes: [], width: Num.int(10), aspect: Num.int(1), center: { x: Num.zero(), y: Num.zero() }, zoom: Num.int(1), fit: false, first: true }).drawn.to.x.show(), "5");
  assert.equal(writeMarkdown([{ heading: { level: 2, inline: [{ text: { value: "a *b*" } }] } }]), "## a \\*b\\*\n");
});

test("the writer gives the text the compiler gives, for every case's blocks", () => {
  let n = 0;
  const walk = (d) => {
    for (const f of readdirSync(d)) {
      const p = join(d, f);
      if (statSync(p).isDirectory()) walk(p);
      else if (f.endsWith(".json")) {
        const c = JSON.parse(readFileSync(p, "utf8"));
        const src = join(d, c.source ?? "");
        if (!c.source?.endsWith(".xtmd") || !existsSync(src)) continue;
        const md = (...more) => spawnSync(xtmdBinary(), ["md", src, ...(c.packs ?? []).flatMap((q) => ["--pack", join(d, q)]), ...more], { encoding: "utf8" });
        const blocks = md("--format", "json");
        if (blocks.status !== 0) continue;
        const list = JSON.parse(blocks.stdout);
        assert.equal(fits({ blocks: list }, "blocks", "blocks"), undefined, c.id);
        assert.equal(writeMarkdown(list), md().stdout.replace(/^---\n[\s\S]*?\n---\n\n/, ""), c.id);
        n++;
      }
    }
  };
  walk(join(repo, "checks/cases"));
  walk(join(repo, "engines/published/runtime/test/cases"));
  assert.ok(n > 60, `${n}`);
});
