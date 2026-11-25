// The vscode renderer's widgets: core's 40 extern widgets, drawn as VS Code draws its own UI. The
// controls are VS Code's components: its buttons, checkboxes, switches, toggles, tabs and radio
// lists, its input boxes, a slider drawn from its switch, and its hover and dialog widgets, in the
// editor's theme colours and with its focus ring, and `icon` is a codicon. They keep the web
// renderer's behaviour (its events, keyboard and roles); the look is VS Code's. Markdown's blocks are drawn as VS Code's
// Markdown preview draws them, and the plane, the layout and the graphics in the editor's colours.

import { cls, css, attr, h, kase, str, widgets as web, type VNode, type ViewWidget } from "@extended-mark/widgets";
import type { Node as Drawn } from "@extended-mark/runtime";
import { CODICONS } from "./codicons.js";

/** A web widget, with VS Code's look: its element gets the renderer's classes, and `decorate` adds
 * what VS Code's component has that the web one doesn't. */
function look(name: string, classes: string[], decorate?: (v: VNode, d: Drawn) => void): ViewWidget {
  const base = web[`core/${name}`];
  return (d, ctx) => {
    const v = base(d, ctx) as VNode;
    for (const c of ["vsc", ...classes]) cls(v, c);
    decorate?.(v, d);
    return v;
  };
}

/** A press's kind, as VS Code's component for it: a button (primary or secondary), a checkbox, a
 * radio, a toggle switch, a tab or a list option. */
function pressLook(v: VNode, d: Drawn): void {
  const kind = (kase(d.inputs.kind) ?? "BUTTON").toLowerCase();
  cls(v, "vsc-button", kind === "button" || kind === "toggle");
  // a plain press is VS Code's secondary button; a button, its primary one
  cls(v, "vsc-secondary", kind === "plain");
  for (const k of ["check", "radio", "switch", "tab", "option"]) cls(v, `vsc-${k}`, kind === k);
}

/** The slider's track is filled up to its value, which the track's own colours can't say: the share
 * goes to the stylesheet as --vsc-fill, at each redraw and as it is dragged. */
function sliderLook(v: VNode): void {
  const fill = (root: Element) => {
    const input = root.querySelector<HTMLInputElement>("input[type=range]");
    if (!input) return;
    const lo = Number(input.min || 0);
    const hi = Number(input.max || 100);
    const share = hi > lo ? ((Number(input.value) - lo) / (hi - lo)) * 100 : 0;
    input.style.setProperty("--vsc-fill", `${Math.min(100, Math.max(0, share))}%`);
  };
  const own = v.hook;
  v.hook = {
    ...own,
    mount(el) {
      own?.mount?.(el);
      el.addEventListener("input", () => fill(el));
      fill(el);
    },
    update(el) {
      own?.update?.(el);
      fill(el);
    },
  };
}

/** `icon`: a codicon, VS Code's own glyph for the idea. The web's drawing of the glyph is inside it
 * too, hidden by the preview's stylesheet once the codicon font is there, so a page without that
 * stylesheet (the checks' own) still shows the icon. Its role and name are this element's. */
const icon: ViewWidget = (d, ctx) => {
  // the drawing is decoration inside this element, which has the name
  const drawn = web["core/icon"](d, ctx) as VNode;
  for (const a of ["role", "aria-label"]) attr(drawn, a, undefined);
  attr(drawn, "aria-hidden", "true");
  const glyph = kase(d.inputs.name) ?? "";
  const codicon = CODICONS[glyph];
  const label = str(d.inputs.label);
  const el = h("span", undefined, {
    class: codicon ? `vsc vsc-icon codicon codicon-${codicon}` : "vsc vsc-icon",
    "data-glyph": glyph,
    "data-size": (kase(d.inputs.size) ?? "NORMAL").toLowerCase(),
    role: label ? "img" : undefined,
    "aria-label": label || undefined,
    "aria-hidden": label ? undefined : "true",
  }, [drawn]);
  return css(el, "color", ctx.tokens.color(d.inputs.tone));
};

/** Every widget the vscode renderer supplies, by full name. */
export const widgets: Record<string, ViewWidget> = {
  ...web,
  "core/press": look("press", ["vsc-press"], pressLook),
  "core/field": look("field", ["vsc-field"]),
  "core/slider": look("slider", ["vsc-slider"], sliderLook),
  "core/label": look("label", ["vsc-label"]),
  "core/image": look("image", ["vsc-image"]),
  "core/icon": icon,
  "core/overlay": look("overlay", ["vsc-overlay"]),
  "core/paragraph": look("paragraph", ["vsc-md"]),
  "core/heading": look("heading", ["vsc-md"]),
  "core/bullets": look("bullets", ["vsc-md"]),
  "core/item": look("item", ["vsc-md"]),
  "core/quote": look("quote", ["vsc-md"]),
  "core/code": look("code", ["vsc-md", "vsc-code"]),
  "core/math": look("math", ["vsc-md"]),
  "core/table": look("table", ["vsc-md", "vsc-table"]),
  "core/footnote": look("footnote", ["vsc-md"]),
  "core/divider": look("divider", ["vsc-md"]),
};

/** A widget that failed, in VS Code's look. */
export const failed: ViewWidget = (d) => h("div", "notice", { role: "alert", class: "xtmd-notice vsc vsc-notice" }, [`${d.widget.split("/").pop()} failed: ${d.failed}`]);
