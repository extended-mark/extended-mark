// What @extended-mark/dom's widgets read values with, shared with the engines that draw core's widgets
// another way (@extended-mark/view and the framework engines on it), so every web engine reads a value,
// a token, an icon and a link the same: no DOM is needed to import these.

export { field, flag, inner, kase, list, nodes, num, point, pointInput, range, region, regionExact, sig, str, exact, placesFor, toNum, type XY } from "./values.js";
export { paint, MAX_SHADE_PIXELS, type Shader } from "./shade.js";
export { Tokens, tokenVar, type Theme } from "./tokens.js";
export { GLYPHS, FILLED } from "./icons.js";
export { safeHref, safeSrc, templateText, texOf, noteId } from "./rich.js";
export { texToMathML, drawTex } from "./tex.js";
export { webFns } from "./fns.js";
export { css as styleText } from "./style.js";
