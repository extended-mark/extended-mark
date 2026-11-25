// bump.mjs on a small copy of the repository's shapes: each place it owns is raised, everything else is
// left as it was, and --check's drift names what disagrees with VERSION.

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { after, test } from "node:test";
import { bump, drift } from "../bump.mjs";

const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const scratch = mkdtempSync(join(tmpdir(), "bump-"));
after(() => rmSync(scratch, { recursive: true, force: true }));

const json = (o) => JSON.stringify(o, null, 2) + "\n";
const tree = {
  VERSION: "0.3.1\n",
  "compiler/Cargo.toml": '[workspace.package]\nversion = "0.3.1"\nedition = "2021"\n\n[workspace.dependencies]\nserde = { version = "1", features = ["derive"] }\n',
  "compiler/xtmd/Cargo.toml": '[package]\nname = "xtmd"\nversion.workspace = true\n\n[dependencies]\nxtmd-ir = { path = "../ir", version = "0.3.1" }\nanyhow = { version = "0.3.1" }\n',
  "compiler/Cargo.lock": '[[package]]\nname = "anyhow"\nversion = "0.3.1"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n\n[[package]]\nname = "xtmd-ir"\nversion = "0.3.1"\ndependencies = [\n "anyhow",\n]\n',
  "compiler/npm/extended-mark/package.json": json({ name: "extended-mark", version: "0.3.1", optionalDependencies: { "@extended-mark/wasm": "file:../wasm", "@extended-mark/cli-linux-x64": "0.3.1" }, devDependencies: { typescript: "0.3.1" } }),
  "compiler/npm/extended-mark/package-lock.json": json({ name: "extended-mark", version: "0.3.1", lockfileVersion: 3, packages: { "": { name: "extended-mark", version: "0.3.1" }, "../wasm": { version: "0.3.1" }, "../gone": { version: "0.3.0" }, "node_modules/typescript": { version: "0.3.1", resolved: "https://registry.npmjs.org/typescript/-/typescript-0.3.1.tgz" } } }),
  "compiler/npm/wasm/package.json": json({ name: "@extended-mark/wasm", version: "0.3.1" }),
  "engines/published/runtime/package.json": json({ name: "@extended-mark/runtime", version: "0.3.1" }),
  "engines/published/bindings/view/src/snapshot.ts": 'export const RELEASE = "0.3.1";\n',
  "engines/somebody-else/package.json": json({ name: "left-pad", version: "0.3.1" }),
  "examples/web/package.json": json({ name: "@extended-mark/example-web", version: "0.3.1", private: true }),
  "ide/vscode/package.json": json({ name: "xtmd", publisher: "extended-mark", version: "0.3.1" }),
  "ide/vscode/README.md": "npm run package       # xtmd-0.3.1.vsix\n",
  "schemas/examples/counter.ir.json": json({ ir: 3, meta: { xtmd: "0.3.1" } }),
  "tools/release/README.md": "Every package and crate carries the release's version (0.3.1); …\n",
  "compiler/xtmd/src/migrate/mod.rs": 'pub const FIRST: &str = "0.3.0";\n',
};
for (const [file, text] of Object.entries(tree)) {
  mkdirSync(dirname(join(scratch, file)), { recursive: true });
  writeFileSync(join(scratch, file), text);
}
const read = (file) => readFileSync(join(scratch, file), "utf8");

test("the tree says one version: no drift", () => {
  assert.deepEqual(drift(scratch, "0.3.1"), []);
});

test("bump raises every place it owns, and nothing else", () => {
  const changed = bump(scratch, "0.4.0");
  assert.equal(read("VERSION"), "0.4.0\n");
  assert.match(read("compiler/Cargo.toml"), /^version = "0\.4\.0"$/m);
  assert.match(read("compiler/Cargo.toml"), /serde = \{ version = "1"/);
  assert.match(read("compiler/xtmd/Cargo.toml"), /xtmd-ir = \{ path = "\.\.\/ir", version = "0\.4\.0" \}/);
  assert.match(read("compiler/xtmd/Cargo.toml"), /anyhow = \{ version = "0\.3\.1" \}/, "a crates.io dependency of the same number");
  assert.match(read("compiler/Cargo.lock"), /name = "xtmd-ir"\nversion = "0\.4\.0"/);
  assert.match(read("compiler/Cargo.lock"), /name = "anyhow"\nversion = "0\.3\.1"\nsource/, "crates.io's entry in the lock");
  const pkg = JSON.parse(read("compiler/npm/extended-mark/package.json"));
  assert.equal(pkg.version, "0.4.0");
  assert.equal(pkg.optionalDependencies["@extended-mark/cli-linux-x64"], "0.4.0");
  assert.equal(pkg.optionalDependencies["@extended-mark/wasm"], "file:../wasm");
  assert.equal(pkg.devDependencies.typescript, "0.3.1");
  const lock = JSON.parse(read("compiler/npm/extended-mark/package-lock.json"));
  assert.equal(lock.version, "0.4.0");
  assert.equal(lock.packages[""].version, "0.4.0");
  assert.equal(lock.packages["../wasm"].version, "0.4.0");
  assert.equal(lock.packages["../gone"].version, "0.3.0", "an entry for a folder that is no more");
  assert.equal(lock.packages["node_modules/typescript"].version, "0.3.1", "the registry's");
  assert.equal(JSON.parse(read("engines/published/runtime/package.json")).version, "0.4.0");
  assert.equal(read("engines/published/bindings/view/src/snapshot.ts"), 'export const RELEASE = "0.4.0";\n');
  assert.equal(JSON.parse(read("engines/somebody-else/package.json")).version, "0.3.1", "not ours");
  assert.equal(JSON.parse(read("examples/web/package.json")).version, "0.4.0");
  assert.equal(JSON.parse(read("ide/vscode/package.json")).version, "0.4.0");
  assert.equal(read("ide/vscode/README.md"), "npm run package       # xtmd-0.4.0.vsix\n");
  assert.equal(JSON.parse(read("schemas/examples/counter.ir.json")).meta.xtmd, "0.4.0");
  assert.match(read("tools/release/README.md"), /\(0\.4\.0\)/);
  assert.equal(read("compiler/xtmd/src/migrate/mod.rs"), 'pub const FIRST: &str = "0.3.0";\n', "the language's first release");
  assert.equal(new Set(changed).size, changed.length, "each file once");
  assert.deepEqual(drift(scratch, "0.4.0"), []);
  assert.deepEqual(bump(scratch, "0.4.0"), [], "a second bump changes nothing");
});

test("--check's drift names each file that disagrees, and what it says", () => {
  writeFileSync(join(scratch, "engines/published/runtime/package.json"), json({ name: "@extended-mark/runtime", version: "0.3.9" }));
  assert.deepEqual(drift(scratch, "0.4.0"), [{ file: "engines/published/runtime/package.json", says: ["0.3.9"] }]);
});

test("the repository says VERSION everywhere", () => {
  const version = readFileSync(join(repo, "VERSION"), "utf8").trim();
  assert.deepEqual(drift(repo, version), []);
});
