"use client";
// @extended-mark/react: Extended Mark documents in React, drawn by it. <XtmdDoc ir={…}> draws a
// page from an IR built ahead with React's own components (doc.ts), on a server and in the browser.
// The live document, which compiles a document's text in the browser, is @extended-mark/react-live.

export { XtmdDoc, toReact, type ReactWidgetProps, type XtmdDocHandle, type XtmdDocProps } from "./doc.js";
export type { DocError, Restored, Snapshot, ViewContext, ViewWidget } from "@extended-mark/widgets";
export type { DocProps } from "@extended-mark/widgets/doc";
export type { Call, Given, Ir, Outcome, Page, PageValues, Input, ir } from "@extended-mark/runtime";
