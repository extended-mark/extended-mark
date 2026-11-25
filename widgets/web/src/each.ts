// Core's widgets one by one, for what holds only the ones it draws, and the name each has here.

export { bullets, code, divider, footnote, heading, item, math, paragraph, quote, table } from "./widgets/blocks.js";
export { field, icon, image, label, overlay, press, slider } from "./widgets/controls.js";
export { canvas, circle, handle, layer, path, place, shade } from "./widgets/gfx.js";
export { cell, flex, grid, sizing } from "./widgets/layout.js";
export { plane, spot } from "./widgets/plane.js";
export { bold, codeSpan, inlineImage, inlineMath, italic, lineBreak, link, noteMark, strike, words } from "./widgets/rich.js";

/** Each widget's name in this module, by its full name. */
export const NAMES: Readonly<Record<string, string>> = {
  "core/paragraph": "paragraph",
  "core/heading": "heading",
  "core/bullets": "bullets",
  "core/item": "item",
  "core/quote": "quote",
  "core/code": "code",
  "core/math": "math",
  "core/table": "table",
  "core/footnote": "footnote",
  "core/divider": "divider",
  "core/words": "words",
  "core/bold": "bold",
  "core/italic": "italic",
  "core/strike": "strike",
  "core/link": "link",
  "core/code_span": "codeSpan",
  "core/inline_math": "inlineMath",
  "core/inline_image": "inlineImage",
  "core/note_mark": "noteMark",
  "core/line_break": "lineBreak",
  "core/label": "label",
  "core/image": "image",
  "core/icon": "icon",
  "core/press": "press",
  "core/field": "field",
  "core/slider": "slider",
  "core/overlay": "overlay",
  "core/canvas": "canvas",
  "core/layer": "layer",
  "core/circle": "circle",
  "core/path": "path",
  "core/place": "place",
  "core/handle": "handle",
  "core/shade": "shade",
  "core/flex": "flex",
  "core/grid": "grid",
  "core/cell": "cell",
  "core/sizing": "sizing",
  "core/plane": "plane",
  "core/spot": "spot",
};
