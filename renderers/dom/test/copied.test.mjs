// A widget set copied as a starting point (the sources in a folder of a host's own, renamed and changed), given
// to the renderer as the host's widgets: it passes what ours does. A copy stands on what the packages export and
// nothing else, or it wouldn't build.

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

test("a copied widget set passes the widgets' own tests", () => {
  const dir = mkdtempSync(join(tmpdir(), "xtmd-copied-set-"));
  cpSync(join(here, "../../../widgets/web/src"), join(dir, "set"), { recursive: true });
  // the copy says whose it is on everything it draws, so what drew a page is seen
  const index = join(dir, "set/index.ts");
  writeFileSync(join(dir, "set/mine.ts"), `import { widgets as copied } from "./widgets/index.js";
import type { ViewWidget } from "./draw.js";
export const widgets: Record<string, ViewWidget> = Object.fromEntries(
  Object.entries(copied).map(([name, w]) => [name, (d, ctx) => {
    const v = w(d, ctx);
    return v && typeof v === "object" ? { ...v, attrs: { ...v.attrs, "data-set": "copied" } } : v;
  }]),
);
`);
  writeFileSync(index, readFileSync(index, "utf8").replace('export { widgets } from "./widgets/index.js";', 'export { widgets } from "./mine.js";'));
  const env = { ...process.env, XTMD_TEST_SET: index };
  delete env.NODE_TEST_CONTEXT;
  let out;
  try {
    out = execFileSync(process.execPath, ["--test", "--test-reporter=tap", join(here, "widgets.test.mjs"), join(here, "copied-drawn.mjs")], { env, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  } catch (e) {
    assert.fail(`the copied set fails:\n${e.stdout ?? e}`);
  }
  assert.match(out, /^# fail 0$/m, out);
  assert.match(out, /^# skipped 0$/m, out);
});
