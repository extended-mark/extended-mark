// @extended-mark/solid: Extended Mark documents in Solid, drawn by it. <XtmdDoc ir={…}> draws a
// page from an IR built ahead with Solid's own fine-grained rendering (doc.ts), on a server and in
// the browser. The live document is @extended-mark/solid-live.

export { XtmdDoc, XtmdNode, type SolidWidgetProps, type XtmdDocHandle, type XtmdDocProps } from "./doc.js";
export type { DocError, Restored, Snapshot, ViewContext, ViewWidget } from "@extended-mark/widgets";
export type { DocProps } from "@extended-mark/widgets/doc";
export type { Call, Given, Ir, Outcome, Page, PageValues, Input, ir } from "@extended-mark/runtime";
