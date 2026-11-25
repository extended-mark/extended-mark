// @extended-mark/svelte: Extended Mark documents in Svelte 5, the static engine. <XtmdDoc ir={…}> draws
// a page from an IR built ahead with Svelte's own elements (XtmdDoc.svelte, XtmdNode.svelte), on a
// server and in the browser. The live engine is @extended-mark/svelte-live.

export { default as XtmdDoc } from "./XtmdDoc.svelte";
export { default as XtmdNode } from "./XtmdNode.svelte";
export type { SvelteWidgetProps } from "./node.js";
export type { DocError, Restored, Snapshot, ViewContext, ViewWidget } from "@extended-mark/widgets";
export type { DocProps } from "@extended-mark/widgets/doc";
export type { Call, Given, Ir, Outcome, Page, PageValues, Input, ir } from "@extended-mark/runtime";
