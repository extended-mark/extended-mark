// @extended-mark/view: what every framework engine draws an Extended Mark page from. A `Doc` runs the document (its
// IR given or fetched, `$state` controlled or not, every write checked, snapshots), and `render()`
// gives the page as plain elements (`VNode`s) that each engine renders with its own renderer:
// core's widgets are written once here, drawn as @extended-mark/dom draws them.

export { Doc, hash, type DocError, type DocProps, type Restored, type Snapshot } from "./doc.js";
export { renderPage, children, type Drawing, type ViewContext, type ViewWidget } from "./render.js";
export { h, s, attr, cls, css, prop, on, slotsOf, textOf, type Child, type Component, type Hook, type VNode, type WidgetProps } from "./vnode.js";
export { toHtml, escapeHtml, walk } from "./html.js";
export { widgets } from "./widgets/index.js";
export { act, type Action } from "./act.js";
export type { Call, Input, Node, Outcome, PageValues, ir } from "@extended-mark/runtime";
export type { Ir } from "@extended-mark/embed";
export { Patcher, type Host } from "./patch.js";
