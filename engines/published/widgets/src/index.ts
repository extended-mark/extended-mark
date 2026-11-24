// @extended-mark/widgets: Extended Mark's widget set for the web. A widget is a fn from a node of the drawn
// tree to plain elements (`VNode`s, `schemas/web.schema.json`), so a renderer of any kind, or an engine that
// writes files, puts them on a page. `children` draws nodes with a set of widgets; `Patcher` follows elements
// onto a live tree through whatever makes its elements.

export { widgets } from "./widgets/index.js";
export { children, drawn, type Drawing, type ViewContext, type ViewWidget } from "./draw.js";
export { h, s, attr, cls, css, prop, on, slotsOf, textOf, type Child, type Component, type Hook, type VNode, type WidgetProps } from "./vnode.js";
export { toHtml, toJson, escapeHtml, walk } from "./html.js";
export { Patcher, type Host } from "./patch.js";
export { act, type Action } from "./act.js";
export { Tokens, tokenVar, type Theme, type TokenSource } from "./tokens.js";
export { layoutPlanes, type Measure } from "./planes.js";
export { webFns } from "./fns.js";
export { css as styleText } from "./style.js";
export { composing } from "./widgets/controls.js";
export { field, flag, inner, kase, list, nodes, num, point, pointInput, range, region, regionExact, sig, str, exact, placesFor, toNum, type XY } from "./values.js";
export { paint, MAX_SHADE_PIXELS, type Shader } from "./shade.js";
export { GLYPHS, FILLED } from "./icons.js";
export { safeHref, safeSrc, templateText, texOf, noteId } from "./rich.js";
export { texToMathML, drawTex } from "./tex.js";
