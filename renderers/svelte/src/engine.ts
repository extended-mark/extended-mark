// Extended Mark's engine for Svelte. IR in, a Svelte component a page out, with no runtime and no IR in it: a small
// component beside the page's code, for the host's Svelte compiler, as Svelte's own components are.

import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { PROPS_TYPES, buildComponents, type ComponentOptions, type Files, type Framework, type Job } from "@extended-mark/web/components";

/** What draws a page, as it ships here: with the types it takes from our packages left out, as none is there. */
const ours = (file: string): string =>
  readFileSync(fileURLToPath(new URL(`./${file}`, import.meta.url)), "utf8").replace(/^\s*import type \{([^}]*)\} from "@extended-mark\/view";$/gm, (_, names: string) => names.split(",").map((n) => `  type ${n.trim()} = any;`).join("\n"));

const svelte: Framework = {
  name: "xtmd-svelte",
  external: ["svelte", "svelte/*"],
  require: createRequire(import.meta.url),
  script: (p) => `${p.name}.page`,
  code: () => "export const make=(p)=>new BuiltDoc(built,p);",
  files: (p) => [
    {
      path: `${p.name}.svelte`,
      text: `<script>
  import XtmdPage from "${p.up}assets/XtmdPage.svelte";
  import { make } from "./${p.name.split("/").pop()}.page.js";

  let props = $props();
  let page = $state();

  export const set = (fields) => page.set(fields);
  export const getState = () => page.getState();
  export const expose = (channel, name, ...args) => page.expose(channel, name, ...args);
  export const snapshot = () => page.snapshot();
  export const restore = (snap) => page.restore(snap);
</script>

<XtmdPage bind:this={page} {make} {...props} />
`,
    },
    { path: `${p.name}.page.d.ts`, text: "export declare const make: (props: object) => any;\n" },
    {
      path: `${p.name}.svelte.d.ts`,
      text: `import type { Component } from "svelte";\nimport type { SveltePageHandle, SveltePageProps } from "${p.up}assets/xtmd.js";\ndeclare const ${p.component}: Component<SveltePageProps, SveltePageHandle>;\nexport default ${p.component};\n`,
    },
  ],
  shared: () => [
    { path: "assets/XtmdPage.svelte", text: ours("XtmdPage.svelte") },
    { path: "assets/XtmdNode.svelte", text: ours("XtmdNode.svelte") },
    { path: "assets/node.js", text: ours("node.js") },
    { path: "assets/node.d.ts", text: "export type SvelteWidgetProps = any;\nexport declare function attrsOf(v: any): Record<string, unknown>;\nexport declare function keyOf(c: any, i: number): string;\nexport declare function hook(el: Element, v: any): { update(v: any): void; destroy(): void };\n" },
    {
      path: "assets/xtmd.d.ts",
      text: `import type { Component, Snippet } from "svelte";
${PROPS_TYPES}/** What a component that draws a widget is given. */
export interface WidgetProps {
  node: { id: string; widget: string; inputs: Record<string, Input>; events?: string[]; writes?: Record<string, unknown>; failed?: string };
  event(name: string, args?: Input[]): void;
  write(input: string, value: Input): void;
  call(input: string, args: Input[]): Input;
  /** An input's widgets, a slot's or a rich_text's: {@render slot("body")}. */
  slot: Snippet<[input: string]>;
}
export interface SveltePageProps extends PageProps {
  /** Components as widgets, by full name, for those the engine was told of (\`--components\`). */
  components?: Record<string, Component<WidgetProps>>;
  id?: string;
  class?: string;
}
/** What bind:this gives. */
export interface SveltePageHandle extends Omit<PageHandle, "state"> {
  getState(): Values;
}
`,
    },
  ],
};

/** Pages into Svelte components, as `schemas/build.schema.json` says. */
export const build = (job: Job, options: ComponentOptions = {}): Promise<Files> => buildComponents(job, options, svelte);
