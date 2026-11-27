// What `xtmd export --engine` runs: what is built on standard input, the files on standard output.

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import type { Files, Job, Options } from "./index.js";

/** One more option of an engine's, beside what every one takes. */
export interface More<O> {
  help: string;
  take(options: O, value: () => string): void | Promise<void>;
}

/** An engine's command. `more` is its own options, by name. */
export async function command<O extends Options>(name: string, what: string, build: (job: Job, options: O) => Promise<Files>, more: Record<string, More<O>> = {}): Promise<void> {
  const stop = (message: string): never => {
    process.stderr.write(`${message}\n`);
    process.exit(2);
  };
  const all: Record<string, More<O>> = {
    "--widgets": { help: "FILE   a module whose `widgets` are the host's own, by full name", take: (o, v) => void (o.widgets = v()) },
    "--only-widgets": { help: "       draw with those, and none of ours", take: (o) => void (o.only = true) },
    "--tokens": { help: 'FILE    token values, as JSON: {"core/color.INK": {...}}', take: (o, v) => void (o.tokens = JSON.parse(readFileSync(v(), "utf8"))) },
    "--fns": { help: "FILE       a module whose `fns` are value fns, by full name", take: (o, v) => void (o.fns = v()) },
    "--readable": { help: "       scripts that can be read", take: (o) => void (o.readable = true) },
    ...more,
  };
  try {
    const options = {} as O;
    const args = process.argv.slice(2);
    for (let k = 0; k < args.length; k++) {
      const [flag, given] = args[k].split(/=(.*)/s);
      if (flag === "--help") {
        process.stdout.write(`${name}: ${what}. Run by \`xtmd export --engine\`.\n\n${Object.entries(all).map(([n, x]) => `  ${n} ${x.help}`.trimEnd()).join("\n")}\n`);
        return;
      }
      if (!Object.hasOwn(all, flag)) stop(`${name} has no ${flag}`);
      await all[flag].take(options, () => given ?? args[++k] ?? stop(`${flag} takes a value`));
    }
    const job = JSON.parse(readFileSync(0, "utf8")) as Job;
    if (job.version !== 1) stop(`${name} reads build 1, and this is ${job.version}`);
    process.stdout.write(JSON.stringify(await build(job, options)));
  } catch (e) {
    process.stderr.write(`${name}: ${(e as Error)?.stack ?? e}\n`);
    process.exit(1);
  }
}

/** A module's default export, by its file. */
export async function defaultOf<T>(file: string): Promise<T> {
  return (await import(pathToFileURL(resolve(file)).href)).default as T;
}
