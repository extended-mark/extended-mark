// Extended Mark's engine for Angular. IR in, an Angular component a page out, with no runtime and no IR in it: the page's
// code, what runs it and what draws it, and Angular left outside. A component is written as a library's is, for the
// linker of the host's build.

import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { PROPS_TYPES, buildComponents, type ComponentOptions, type Files, type Framework, type Job, type PageFile } from "@extended-mark/web/components";

/** The Angular this package was compiled by, which its declarations are in the words of. */
const version = /version: "([^"]+)"/.exec(readFileSync(new URL("./page.js", import.meta.url), "utf8"))![1];

/** A page's element: `docs/release-flow` is `<xtmd-release-flow>`. */
const selector = (p: PageFile): string => `xtmd-${p.component.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase()}`;

const angular: Framework = {
  name: "xtmd-angular",
  external: ["@angular/*", "rxjs", "rxjs/*"],
  require: createRequire(import.meta.url),
  code: (p) => `import*as i0 from"@angular/core";import{XtmdPage}from"@extended-mark/angular/built";
export class ${p.component} extends XtmdPage{make(props){return new BuiltDoc(built,props)}
static{this.ɵfac=i0.ɵɵngDeclareFactory({minVersion:"12.0.0",version:${JSON.stringify(version)},ngImport:i0,type:${p.component},deps:null,target:i0.ɵɵFactoryTarget.Component})}
static{this.ɵcmp=i0.ɵɵngDeclareComponent({minVersion:"14.0.0",version:${JSON.stringify(version)},type:${p.component},isStandalone:true,selector:${JSON.stringify(selector(p))},host:{styleAttribute:"display: contents"},usesInheritance:true,ngImport:i0,template:"",isInline:true})}}
export default ${p.component};`,
  files: (p) => [
    {
      path: `${p.name}.d.ts`,
      text: `import * as i0 from "@angular/core";
import { XtmdPage } from "${p.up}assets/xtmd.js";
export declare class ${p.component} extends XtmdPage {
  static ɵfac: i0.ɵɵFactoryDeclaration<${p.component}, never>;
  static ɵcmp: i0.ɵɵComponentDeclaration<${p.component}, "${selector(p)}", never, {}, {}, never, never, true, never>;
}
export default ${p.component};
`,
    },
  ],
  shared: () => [
    {
      path: "assets/xtmd.d.ts",
      text: `import { EventEmitter, OnChanges, OnDestroy, OnInit, Type } from "@angular/core";
import * as i0 from "@angular/core";
${PROPS_TYPES}/** What a component that draws a widget is given, each an input it declares. */
export interface WidgetProps {
  node: { id: string; widget: string; inputs: Record<string, Input>; events?: string[]; writes?: Record<string, unknown>; failed?: string };
  event(name: string, args?: Input[]): void;
  write(input: string, value: Input): void;
  call(input: string, args: Input[]): Input;
}
/** A page: its inputs, its outputs, and what a template reference to it gives. */
export declare abstract class XtmdPage implements OnInit, OnChanges, OnDestroy {
  state?: PageProps["state"];
  defaultState?: PageProps["defaultState"];
  defaultSnapshot?: PageProps["defaultSnapshot"];
  page?: PageProps["page"];
  externs?: PageProps["externs"];
  fns?: PageProps["fns"];
  /** Components as widgets, by full name, for those the engine was told of (\`--components\`). */
  components?: Record<string, Type<unknown>>;
  readonly stateChange: EventEmitter<Values>;
  readonly error: EventEmitter<DocError>;
  readonly calls: EventEmitter<Call[]>;
  ngOnInit(): void;
  ngOnChanges(): void;
  ngOnDestroy(): void;
  set(changes: Values): void;
  getState(): Values;
  expose(channel: string, name: string, ...args: Input[]): void;
  snapshot(): Snapshot;
  restore(snap: Snapshot): Restored;
  static ɵfac: i0.ɵɵFactoryDeclaration<XtmdPage, never>;
  static ɵdir: i0.ɵɵDirectiveDeclaration<XtmdPage, never, never, { ${["state", "defaultState", "defaultSnapshot", "page", "externs", "fns", "components"].map((n) => `"${n}": { "alias": "${n}"; "required": false; }`).join("; ")}; }, { "stateChange": "stateChange"; "error": "error"; "calls": "calls"; }, never, never, true, never>;
}
`,
    },
  ],
};

/** Pages into Angular components, as `schemas/build.schema.json` says. */
export const build = (job: Job, options: ComponentOptions = {}): Promise<Files> => buildComponents(job, options, angular);
