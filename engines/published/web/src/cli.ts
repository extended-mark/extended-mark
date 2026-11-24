#!/usr/bin/env node
// What `xtmd build --engine` runs: what is built on standard input, the files on standard output.

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { build, type Options } from "./index.js";

const HELP = `xtmd-web: pages into HTML, CSS and scripts. Run by \`xtmd build --engine\`.

  --widgets FILE   a module whose \`widgets\` are the host's own, by full name
  --only-widgets   draw with those, and none of ours
  --tokens FILE    token values, as JSON: {"core/color.INK": {...}}
  --shell FILE     a module whose default is the page around a page: a fn of it to HTML
  --fns FILE       a module whose \`fns\` are value fns, by full name
  --readable       scripts that can be read
`;

async function main(args: string[]): Promise<void> {
  const options: Options = {};
  for (let k = 0; k < args.length; k++) {
    const [name, given] = args[k].split(/=(.*)/s);
    const value = () => given ?? args[++k] ?? stop(`${name} takes a file`);
    if (name === "--help") return void process.stdout.write(HELP);
    else if (name === "--widgets") options.widgets = value();
    else if (name === "--only-widgets") options.only = true;
    else if (name === "--tokens") options.tokens = JSON.parse(readFileSync(value(), "utf8"));
    else if (name === "--shell") options.shell = (await import(pathToFileURL(resolve(value())).href)).default;
    else if (name === "--fns") options.fns = value();
    else if (name === "--readable") options.readable = true;
    else stop(`xtmd-web has no ${name}`);
  }
  const job = JSON.parse(readFileSync(0, "utf8"));
  if (job.version !== 1) stop(`xtmd-web reads build 1, and this is ${job.version}`);
  process.stdout.write(JSON.stringify(await build(job, options)));
}

function stop(message: string): never {
  process.stderr.write(`${message}\n`);
  process.exit(2);
}

main(process.argv.slice(2)).catch((e) => {
  process.stderr.write(`xtmd-web: ${e?.stack ?? e}\n`);
  process.exit(1);
});
