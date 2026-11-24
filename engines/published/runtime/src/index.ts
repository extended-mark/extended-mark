// @extended-mark/runtime: runs the IR the xtmd compiler writes, on the web, agreeing with the native
// runtime. A host gives a page its IR, draws the tree it gives, and tells it what the reader does.

export { Page, IR_VERSION, apply, everyNode, nodesIn, type PageOptions, type PageValues, type Broken, type Outcome, type Call } from "./host.js";
export type { Change, Child, Contract, Ends, Input, Node, Tree } from "./generated/tree.js";
export type { Data } from "./generated/state.js";
export type { Action } from "./generated/actions.js";
export type { Told } from "./generated/told.js";
export type { Blocks } from "./generated/blocks.js";
export { Num } from "./eval/num.js";
export { writeMarkdown, writeInline, type Block, type Inline } from "./markdown.js";
export { layout, place, around, keepInside, MAX_LAYOUT_PASSES, type Measure, type Size, type Pt, type Region } from "./layout.js";
export type * as ir from "./generated/ir.js";
