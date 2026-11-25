// Extended Mark's engine for Vue. IR in, a Vue component a page out, with no runtime and no IR in it: the page's code,
// what runs it and what draws it, and Vue left outside.

import { createRequire } from "node:module";
import { PROPS_TYPES, buildComponents, type ComponentOptions, type Files, type Framework, type Job } from "@extended-mark/web/components";

const vue: Framework = {
  name: "xtmd-vue",
  external: ["vue"],
  require: createRequire(import.meta.url),
  code: (p) => `import{component}from"@extended-mark/vue/built";export default component(${JSON.stringify(p.component)},(p)=>new BuiltDoc(built,p));`,
  files: (p) => [
    {
      path: `${p.name}.d.ts`,
      text: `import type { DefineComponent } from "vue";\nimport type { PageHandle, PageProps } from "${p.up}assets/xtmd.js";\nimport type { Component } from "vue";\ndeclare const ${p.component}: DefineComponent<PageProps & { components?: Record<string, Component> }, PageHandle>;\nexport default ${p.component};\n`,
    },
  ],
  shared: () => [
    {
      path: "assets/xtmd.d.ts",
      text: `${PROPS_TYPES}/** What a component that draws a widget is given. */
export interface WidgetProps {
  node: { id: string; widget: string; inputs: Record<string, Input>; events?: string[]; writes?: Record<string, unknown>; failed?: string };
  event(name: string, args?: Input[]): void;
  write(input: string, value: Input): void;
  call(input: string, args: Input[]): Input;
}
`,
    },
  ],
};

/** Pages into Vue components, as `schemas/build.schema.json` says. */
export const build = (job: Job, options: ComponentOptions = {}): Promise<Files> => buildComponents(job, options, vue);
