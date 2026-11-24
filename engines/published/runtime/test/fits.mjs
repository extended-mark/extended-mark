// Whether JSON fits one of schemas/'s formats: the part of JSON Schema they are written in.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repo = join(dirname(fileURLToPath(import.meta.url)), "../../../..");
const schemas = {};
const of = (n) => (schemas[n] ??= JSON.parse(readFileSync(join(repo, `schemas/${n}.schema.json`), "utf8")).$defs);

/** What is wrong with a value as a format's definition, or undefined. */
export const fits = (v, format, name) => against(v, of(format)[name], "", format);

function against(v, s, at, home) {
  if (s.$ref) {
    const [file, name] = s.$ref.split("#/$defs/");
    const to = file ? file.replace(".schema.json", "") : home;
    return against(v, of(to)[name], at, to);
  }
  const wrong = (what) => `${at || "top"}: ${what}`;
  if (s.oneOf) {
    const key = v && typeof v === "object" ? Object.keys(v)[0] : undefined;
    const way = s.oneOf.find((w) => key in w.properties);
    return way ? against(v, way, at, home) : wrong(`${JSON.stringify(v)} isn't known`);
  }
  if ("const" in s) return v === s.const ? undefined : wrong(`not ${s.const}`);
  const is = { object: (x) => x && typeof x === "object" && !Array.isArray(x), array: Array.isArray, string: (x) => typeof x === "string", boolean: (x) => typeof x === "boolean", integer: Number.isInteger }[s.type];
  if (!is(v)) return wrong(`not a ${s.type}: ${JSON.stringify(v)}`);
  if (s.type === "array") return v.map((x, k) => against(x, s.items, `${at}/${k}`, home)).find(Boolean);
  if (s.type !== "object") return undefined;
  for (const r of s.required ?? []) if (!(r in v)) return wrong(`${r} is missing`);
  for (const [k, x] of Object.entries(v)) {
    const p = s.properties?.[k] ?? (typeof s.additionalProperties === "object" ? s.additionalProperties : undefined);
    if (!p) return wrong(`${k} isn't known`);
    const e = against(x, p, `${at}/${k}`, home);
    if (e) return e;
  }
  return undefined;
}
