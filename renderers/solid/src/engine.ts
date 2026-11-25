// Extended Mark's engine for Solid. IR in, a Solid component a page out, with no runtime and no IR in it: the page's code,
// what runs it and what draws it, and Solid left outside.

import { createRequire } from "node:module";
import { PROPS_TYPES, buildComponents, type ComponentOptions, type Files, type Framework, type Job } from "@extended-mark/web/components";

const solid: Framework = {
  name: "xtmd-solid",
  external: ["solid-js", "solid-js/*"],
  require: createRequire(import.meta.url),
  code: (p) => `import{component}from"@extended-mark/solid/built";export default component(${JSON.stringify(p.component)},(p)=>new BuiltDoc(built,p));`,
  files: (p) => [
    {
      path: `${p.name}.d.ts`,
      text: `import type { Component } from "solid-js";\nimport type { SolidPageProps } from "${p.up}assets/xtmd.js";\ndeclare const ${p.component}: Component<SolidPageProps>;\nexport default ${p.component};\n`,
    },
  ],
  shared: () => [
    {
      path: "assets/xtmd.d.ts",
      text: `import type { Component, JSX } from "solid-js";
${PROPS_TYPES}/** What a component that draws a widget is given. */
export interface WidgetProps {
  node: { id: string; widget: string; inputs: Record<string, Input>; events?: string[]; writes?: Record<string, unknown>; failed?: string };
  event(name: string, args?: Input[]): void;
  write(input: string, value: Input): void;
  call(input: string, args: Input[]): Input;
  /** An input's widgets: a slot's, or a rich_text's. */
  slot(input: string): JSX.Element;
}
export interface SolidPageProps extends PageProps {
  /** Components as widgets, by full name, for those the engine was told of (\`--components\`). */
  components?: Record<string, Component<WidgetProps>>;
  ref?: (handle: PageHandle) => void;
  id?: string;
  class?: string;
}
`,
    },
  ],
};

/** Pages into Solid components, as `schemas/build.schema.json` says. */
export const build = (job: Job, options: ComponentOptions = {}): Promise<Files> => buildComponents(job, options, solid);
