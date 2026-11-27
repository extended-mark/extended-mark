// @extended-mark/angular: Extended Mark documents in Angular, drawn by it. <xtmd-doc [ir]="…">
// draws a page from an IR built ahead through Angular's Renderer2 (doc.ts), on a server and in the
// browser. The live document is @extended-mark/angular-live.

export { XtmdDoc } from "./doc.js";
export type { DocError, Restored, Snapshot, ViewContext, ViewWidget, WidgetProps } from "@extended-mark/widgets";
export type { DocProps } from "@extended-mark/widgets/doc";
export type { Call, Given, Ir, Outcome, Page, PageValues, Input, ir } from "@extended-mark/runtime";
