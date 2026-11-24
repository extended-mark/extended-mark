// Puts a release build of xtmd in its platform package: platforms/<os>-<cpu>/bin.
//
//   node scripts/copy-binary.mjs                        this machine's: cargo build --release -p xtmd
//   node scripts/copy-binary.mjs --target <triple>      one cross-built for another platform, as the
//                                                       release workflow builds each with cargo zigbuild
//
// The triples are the ones in TARGETS below; each names the platform package it goes in.

import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** Each platform's Rust target: static on Linux (musl), so one binary runs on every distribution. */
export const TARGETS = {
  "x86_64-unknown-linux-musl": "linux-x64",
  "aarch64-unknown-linux-musl": "linux-arm64",
  "x86_64-apple-darwin": "darwin-x64",
  "aarch64-apple-darwin": "darwin-arm64",
  "x86_64-pc-windows-gnu": "win32-x64",
};

const here = dirname(fileURLToPath(import.meta.url));
const at = process.argv.indexOf("--target");
const target = at > 0 ? process.argv[at + 1] : undefined;
if (at > 0 && !(target in TARGETS)) {
  console.error(`--target takes one of: ${Object.keys(TARGETS).join(", ")}`);
  process.exit(2);
}
const platform = target ? TARGETS[target] : `${process.platform}-${process.arch}`;
const [os, cpu] = platform.split("-");
const exe = os === "win32" ? "xtmd.exe" : "xtmd";
const built = join(here, "..", "..", "..", "target", ...(target ? [target] : []), "release", exe);
if (!existsSync(built)) {
  console.error(`no release build at ${built}: run cargo ${target ? `zigbuild --release -p xtmd --target ${target}` : "build --release -p xtmd"} in compiler`);
  process.exit(1);
}
const dir = join(here, "..", "platforms", platform);
mkdirSync(join(dir, "bin"), { recursive: true });
if (!existsSync(join(dir, "package.json"))) {
  writeFileSync(join(dir, "package.json"), JSON.stringify({ name: `@extended-mark/cli-${platform}`, version: JSON.parse(readFileSync(join(here, "..", "package.json"), "utf8")).version, description: `The xtmd binary for ${os} on ${cpu}`, license: "MIT", homepage: "https://github.com/extended-mark/extended-mark", repository: { type: "git", url: "git+https://github.com/extended-mark/extended-mark.git", directory: `compiler/npm/extended-mark/platforms/${platform}` }, os: [os], cpu: [cpu], files: ["bin"] }, null, 2) + "\n");
}
copyFileSync(built, join(dir, "bin", exe));
chmodSync(join(dir, "bin", exe), 0o755);
console.log(`${platform}: ${join(dir, "bin", exe)}`);
