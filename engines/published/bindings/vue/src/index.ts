// @extended-mark/vue: Extended Mark documents in Vue 3, the static engine. <XtmdDoc :ir="…"> draws a
// page from an IR built ahead with Vue's own `h` (doc.ts), on a server and in the browser. The
// live engine, which compiles a document's text in the browser, is @extended-mark/vue-live.

export { XtmdDoc, docProps, toVue, type XtmdDocRef } from "./doc.js";
export type { DocError, DocProps, Restored, Snapshot, ViewContext, ViewWidget, WidgetProps } from "@extended-mark/view";
export type { Call, Given, Ir, Outcome, Page, PageValues, Input, ir } from "@extended-mark/view";
