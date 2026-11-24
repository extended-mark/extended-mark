// @extended-mark/view: what every framework engine draws an Extended Mark page from. A `Doc` runs the document (its
// IR given, `$state` controlled or not, every write checked, snapshots), and `render()`
// gives the page as plain elements (`VNode`s) that each engine renders with its own renderer:
// core's widgets are written once here, drawn as @extended-mark/dom draws them.

export { Doc, type DocProps } from "./doc.js";
export { channelName, channelsOf, irText, makePage, moduleOf, sameIr, stateFields, type Given, type Ir } from "./page.js";
export { RELEASE, dataOf, hash, type DocError, type Restored, type Snapshot } from "./snapshot.js";
export { Patcher, act, attr, children, cls, css, escapeHtml, h, on, prop, s, slotsOf, textOf, toHtml, toJson, walk, widgets, type Action, type Child, type Component, type Drawing, type Hook, type Host, type VNode, type ViewContext, type ViewWidget, type WidgetProps } from "@extended-mark/widgets";
export type { Call, Input, Node, Outcome, Page, PageValues, ir } from "@extended-mark/runtime";
