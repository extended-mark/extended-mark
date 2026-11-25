// A framework's engine: pages into components of the framework's. What a component is made of is the web engine's, its
// page's code and the kit, in files that hold all of it but the framework; what is the framework's is its renderer's
// and a few lines written here by whoever gives a `Framework`.

import type { ir } from "@extended-mark/runtime";
import { hash } from "@extended-mark/widgets";
import { generate, type Generated } from "./generate.js";
import { TOKENS, bundle, fnsCode, plugged, run, widgetsCode, type Files, type GivenOptions, type Job, type Options } from "./index.js";

export interface ComponentOptions extends Options {
  /** The widgets, by full name, that the host gives components for as the page runs. */
  components?: string[];
}

/** A page, for what is written around it. */
export interface PageFile {
  /** The document's place among what is built: `docs/shop`. */
  name: string;
  /** Its component's name: `Shop`. */
  component: string;
  /** Where the folder of everything built is, from the page: `./` or `../`. */
  up: string;
}

export interface Framework {
  /** The command: `xtmd-react`. */
  name: string;
  /** What is left outside: the framework's own packages. */
  external: string[];
  /** What finds the framework's renderer's package. */
  require: NodeJS.Require;
  /** The code of a page's script, after `built` and `BuiltDoc` are there. */
  code(p: PageFile): string;
  /** What is written beside a page's script, by its path. */
  files(p: PageFile): { path: string; text: string }[];
  /** What is written once. */
  shared?(): { path: string; text: string }[];
  /** A page's script's name, when it isn't the page's own: what the framework imports is another file. */
  script?(p: PageFile): string;
}

/** A document's name as a component's: `docs/release-flow` is `ReleaseFlow`. */
export function componentName(name: string): string {
  const words = name.split("/").pop()!.split(/[^A-Za-z0-9]+/).filter(Boolean);
  const all = words.map((w) => w[0].toUpperCase() + w.slice(1)).join("");
  return /^[A-Za-z]/.test(all) ? all : `Page${all}`;
}

export async function buildComponents(job: Job, given: ComponentOptions, fw: Framework): Promise<Files> {
  const { options, supplies } = await plugged(given);
  const theirs = new Set(given.components ?? []);
  const files: Files["files"] = [];
  const pages: { name: string; code: string; file: PageFile }[] = [];
  for (const d of job.documents) {
    const file: PageFile = { name: d.name, component: componentName(d.name), up: "../".repeat(d.name.split("/").length - 1) || "./" };
    const g = generate(d.ir, { supplies: (w) => theirs.has(w) || supplies(w) });
    const tokens = (await run(g, options)).tokens(options.tokens ?? {});
    pages.push({ name: fw.script?.(file) ?? d.name, code: pageCode(g, d.ir, tokens, options) + fw.code(file), file });
  }
  await bundle(pages, options, files, { external: fw.external, from: fw.require, named: true });
  for (const p of pages) files.push(...fw.files(p.file));
  files.push(...(fw.shared?.() ?? []));
  return { version: 1, files };
}

function pageCode(g: Generated, m: ir.Module, tokens: Parameters<typeof TOKENS>[0], options: GivenOptions): string {
  const w = widgetsCode(g, options);
  const f = fnsCode(g, options);
  return `${w.imports}${f.imports}import{BuiltDoc${g.calls ? ",withCalls" : ""}}from"@extended-mark/web/kit";${g.planes ? `import{layout}from"@extended-mark/web/layout";` : ""}
${g.code.replace("export const tokens=", "const unused=")}
const built={program,widgets:${w.registry},tokens:{types:${JSON.stringify(TOKENS(tokens))},sheet:""},ir:${JSON.stringify(hash(JSON.stringify(m)))},module:${JSON.stringify(m.module)},${f.fns}${g.planes ? "layout," : ""}${g.calls ? "calls:withCalls," : ""}};
`;
}

/** The props every framework's component takes, as types: no package of ours is needed to read them. */
export const PROPS_TYPES = `/** A value in the IR's form: { num: "3" }, { text: "a" }, { list: [...] }. */
export type Input = Record<string, unknown>;
export type Values = Record<string, Input>;
export interface Snapshot { xtmd: string; ir: string; state: Values; own: Record<string, Values> }
export interface Restored { restored: "all" | "state" | "none"; dropped: string[]; why?: string }
export interface DocError { kind: "refused" | "failed"; rule?: string; code?: string; site?: string }
export interface Call { call: { name: string; args: Input[] } }
export interface PageProps {
  /** \`$state\`, controlled: the page shows this, and every change goes through \`onStateChange\`. */
  state?: Values;
  /** \`$state\` to start from, when the host doesn't control it. */
  defaultState?: Values;
  /** A snapshot to start from. */
  defaultSnapshot?: Snapshot;
  /** The page's width and theme. */
  page?: { width?: "NARROW" | "MEDIUM" | "WIDE"; theme?: "LIGHT" | "DARK" };
  /** Each extern channel's values, by its name in the document or in full. */
  externs?: Record<string, Values>;
  /** Value fns the host supplies, by full name. */
  fns?: Record<string, (args: readonly Input[]) => Input | null>;
  onStateChange?: (state: Values) => void;
  onError?: (error: DocError) => void;
  onCalls?: (calls: Call[]) => void;
}
export interface PageHandle {
  set(fields: Values): void;
  state(): Values;
  expose(channel: string, name: string, ...args: Input[]): void;
  snapshot(): Snapshot;
  restore(snap: Snapshot): Restored;
}
`;
export type { Files, Job } from "./index.js";
