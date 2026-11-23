// @extended-mark/angular: Extended Mark documents in Angular, the static engine. <xtmd-doc src="…"> or <xtmd-doc [ir]="…">
// draws a page from an IR built ahead through Angular's Renderer2 (doc.ts), on a server and in the
// browser. The live engine is @extended-mark/angular-live.

export { XtmdDoc } from "./doc.js";
export type { DocError, DocProps, Restored, Snapshot, ViewContext, ViewWidget } from "@extended-mark/view";
export type { Call, Given, Ir, Outcome, Page, PageValues, Value, ir } from "@extended-mark/embed";
