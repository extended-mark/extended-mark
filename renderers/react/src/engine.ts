// Extended Mark's engine for React. IR in, a React component a page out,
// with no runtime and no IR in it: the page's code, what runs it and what draws it, and React left outside.

import { createRequire } from "node:module";
import { PROPS_TYPES, buildComponents, type ComponentOptions, type Files, type Framework, type Job } from "@extended-mark/web/components";

const react: Framework = {
  name: "xtmd-react",
  external: ["react", "react/*"],
  require: createRequire(import.meta.url),
  code: (p) => `import{component}from"@extended-mark/react/built";export default component(${JSON.stringify(p.component)},(p)=>new BuiltDoc(built,p));`,
  files: (p) => [
    {
      path: `${p.name}.d.ts`,
      text: `import type { ComponentType } from "react";\nimport type { ReactPageProps } from "${p.up}assets/xtmd.js";\ndeclare const ${p.component}: ComponentType<ReactPageProps>;\nexport default ${p.component};\n`,
    },
  ],
  shared: () => [
    {
      path: "assets/xtmd.d.ts",
      text: `import type { ComponentType, CSSProperties, ReactNode, Ref } from "react";
${PROPS_TYPES}/** What a component that draws a widget is given. */
export interface WidgetProps {
  node: { id: string; widget: string; inputs: Record<string, Input>; events?: string[]; writes?: Record<string, unknown>; failed?: string };
  event(name: string, args?: Input[]): void;
  write(input: string, value: Input): void;
  call(input: string, args: Input[]): Input;
  /** An input's widgets: a slot's, or a rich_text's. */
  slot(input: string): ReactNode;
}
export interface ReactPageProps extends PageProps {
  /** Components as widgets, by full name, for those the engine was told of (\`--components\`). */
  components?: Record<string, ComponentType<WidgetProps>>;
  ref?: Ref<PageHandle | null>;
  id?: string;
  className?: string;
  style?: CSSProperties;
}
`,
    },
  ],
};

/** Pages into React components, as \`schemas/build.schema.json\` says. */
export const build = async (job: Job, options: ComponentOptions = {}): Promise<Files> => {
  const out = await buildComponents(job, options, react);
  // a component is the client's, where a framework has both (Next)
  const pages = new Set(job.documents.map((d) => `${d.name}.js`));
  return { ...out, files: out.files.map((f) => (pages.has(f.path) ? { ...f, text: `"use client";\n${f.text}` } : f)) };
};
