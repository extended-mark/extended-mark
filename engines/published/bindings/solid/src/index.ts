// @extended-mark/solid: Extended Mark documents in Solid, the static engine. <XtmdDoc ir={…}> draws a
// page from an IR built ahead with Solid's own fine-grained rendering (doc.ts), on a server and in
// the browser. The live engine is @extended-mark/solid-live.

export { XtmdDoc, XtmdNode, type SolidWidgetProps, type XtmdDocHandle, type XtmdDocProps } from "./doc.js";
export type { DocError, DocProps, Restored, Snapshot, ViewContext, ViewWidget } from "@extended-mark/view";
export type { Call, Given, Ir, Outcome, Page, PageValues, Input, ir } from "@extended-mark/view";
