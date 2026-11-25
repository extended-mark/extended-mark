// @extended-mark/runtime: runs the IR the xtmd compiler writes, on the web, agreeing with the native
// runtime. A host gives a page its IR, draws the tree it gives, and tells it what the reader does.

export { Page, IR_VERSION, apply, everyNode, nodesIn, type PageOptions, type PageValues, type Broken, type Outcome, type Call } from "./host.js";
export type { Change, Child, Contract, Ends, Input, Node, Tree } from "./generated/tree.js";
export type { Data } from "./generated/state.js";
export type { Action } from "./generated/actions.js";
export type { Told } from "./generated/told.js";
export { channelName, channelsOf, irText, makePage, moduleOf, sameIr, stateFields, type Given, type Ir } from "./given.js";
export type { Blocks } from "./generated/blocks.js";
export type { Built, Job } from "./generated/build.js";
export type * as web from "./generated/web.js";
export { Num } from "./eval/num.js";
export { writeMarkdown, writeInline, type Block, type Inline } from "./write.js";
export { lay, place, around, keepInside, type Box, type Places, type Plane, type Pt, type Region, type Size } from "./layout.js";
export type * as ir from "./generated/ir.js";
