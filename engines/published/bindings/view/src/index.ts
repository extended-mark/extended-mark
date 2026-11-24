// @extended-mark/view: what every framework engine draws an Extended Mark page from. A `Doc` runs the document (its
// IR given or fetched, `$state` controlled or not, every write checked, snapshots), and `render()`
// gives the page as plain elements (`VNode`s) that each engine renders with its own renderer:
// core's widgets are written once here, drawn as @extended-mark/dom draws them.

export { Doc, hash, type DocError, type DocProps, type Restored, type Snapshot } from "./doc.js";
export { Patcher, act, attr, children, cls, css, escapeHtml, h, on, prop, s, slotsOf, textOf, toHtml, toJson, walk, widgets, type Action, type Child, type Component, type Drawing, type Hook, type Host, type VNode, type ViewContext, type ViewWidget, type WidgetProps } from "@extended-mark/widgets";
export type { Call, Input, Node, Outcome, PageValues, ir } from "@extended-mark/runtime";
export type { Ir } from "@extended-mark/embed";
