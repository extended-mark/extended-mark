// The page's IR, ahead of time, and the browser's side in one file.
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";
import * as cli from "extended-mark";

const here = (f) => fileURLToPath(new URL(f, import.meta.url));
mkdirSync(here("dist"), { recursive: true });
writeFileSync(here("dist/page.ir.json"), JSON.stringify(await cli.build(here("page.xtmd"))));
await esbuild.build({ entryPoints: [here("client.mjs")], bundle: true, format: "esm", target: "es2022", outfile: here("dist/client.js"), logLevel: "warning" });
