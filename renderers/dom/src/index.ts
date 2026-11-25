// @extended-mark/dom: Extended Mark's renderer for the plain web. `mount` draws a @extended-mark/runtime page
// into an element; `run` makes the page from its IR first. Its widgets are @extended-mark/widgets', unless the
// host gives its own.

export { mount, run } from "./mount.js";
export { XtmdDoc, type XtmdDocProps } from "./doc.js";
export type { Action, Follows, Measure, MountOptions, Mounted, OwnWidget, Page, PageOptions } from "./api.js";
export type { ViewContext, ViewWidget } from "@extended-mark/widgets";
