// Extended Mark's npm packages (the compiler's in compiler/npm, the rest in runtime, widgets, renderers, engines and live), each after the ones
// it depends on: @extended-mark/runtime first, the framework bindings last. Shared by the release's build and its publish.

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

/** Every package of Extended Mark's under compiler/npm, the modules' folders and hosts (`extended-mark`, the compiler and
 * its `xtmd` command, and the @extended-mark ones), not a test's or a showcase's;
 * `published` leaves out the private ones (the playground). */
export function packages({ published = true } = {}) {
  const out = [];
  const walk = (dir) => {
    // a package's own tests and fixtures are no package
    const inPackage = existsSync(join(dir, "package.json"));
    for (const name of readdirSync(dir)) {
      // and a Gradle or Cargo build (renderers/compose) holds none
      if (["node_modules", "dist", "build", "target", ".gradle"].includes(name) || (inPackage && ["test", "testing"].includes(name))) continue;
      const path = join(dir, name);
      if (!statSync(path).isDirectory()) continue;
      const file = join(path, "package.json");
      if (existsSync(file)) {
        const text = readFileSync(file, "utf8");
        const pkg = JSON.parse(text);
        if ((pkg.name === "extended-mark" || pkg.name?.startsWith("@extended-mark/")) && !(published && pkg.private)) out.push({ dir: path, file, pkg, text });
      }
      walk(path);
    }
  };
  walk(join(repo, "compiler", "npm"));
  for (const d of ["runtime", "widgets", "renderers", "engines", "live"]) walk(join(repo, d));
  walk(join(repo, "apps"));
  return ordered(out);
}

/** Each package after its neighbours in dependencies, optionalDependencies and
 * peerDependencies; devDependencies are only for its own tests. */
function ordered(all) {
  const byName = new Map(all.map((p) => [p.pkg.name, p]));
  const order = [];
  const placed = new Set();
  const place = (p, trail = []) => {
    if (placed.has(p.pkg.name)) return;
    if (trail.includes(p.pkg.name)) throw new Error(`a cycle: ${[...trail, p.pkg.name].join(" → ")}`);
    for (const section of ["dependencies", "optionalDependencies", "peerDependencies"]) {
      for (const dep of Object.keys(p.pkg[section] ?? {})) {
        const d = byName.get(dep);
        if (d) place(d, [...trail, p.pkg.name]);
      }
    }
    placed.add(p.pkg.name);
    order.push(p);
  };
  for (const p of all) place(p);
  return order;
}
