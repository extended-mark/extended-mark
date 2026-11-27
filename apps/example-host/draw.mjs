// What this host draws: each extern widget it knows as one element, from its node (docs/specs/05 says what
// each is for; docs/specs/06, The host's side, what a node holds). A widget that isn't here shows its
// Markdown, which comes as nodes of the widgets that are.

const inner = (x) => (x === undefined || x.none ? undefined : x.some ? inner(x.some) : x);
const text = (x) => inner(x)?.text ?? inner(x)?.num ?? "";
const on = (x) => inner(x)?.bool === true;
const name = (x) => inner(x)?.case?.name ?? inner(x)?.token?.split(".").pop();
const tone = (x) => (inner(x)?.token ? `color:var(--${name(x).toLowerCase()})` : undefined);
/** The widgets of an input, in order. */
export const nodes = (x) => inner(x)?.nodes ?? [];

/** A web address that can't run script. */
const href = (u) => (/^(https?:|mailto:|[^:]*$)/i.test(u.trim()) ? u.trim() : undefined);

/** Each widget: its element's tag, its attributes, its words, or the input whose widgets go in it. */
export const widgets = {
  "core/paragraph": (n) => ({ tag: "p", holds: "value", attrs: { style: tone(n.inputs.tone) } }),
  "core/heading": (n) => ({ tag: `h${text(n.inputs.level)}`, holds: "value" }),
  "core/bullets": (n) => ({ tag: on(n.inputs.ordered) ? "ol" : "ul", holds: "items" }),
  "core/item": () => ({ tag: "li", holds: "body" }),
  "core/quote": () => ({ tag: "blockquote", holds: "body" }),
  "core/code": (n) => ({ tag: "pre", words: text(n.inputs.src) }),
  "core/divider": () => ({ tag: "hr" }),
  // what a sentence is made of
  "core/words": (n) => ({ tag: "span", words: text(n.inputs.value) }),
  "core/bold": () => ({ tag: "strong", holds: "body" }),
  "core/italic": () => ({ tag: "em", holds: "body" }),
  "core/strike": () => ({ tag: "s", holds: "body" }),
  "core/link": (n) => ({ tag: "a", holds: "body", attrs: { href: href(text(n.inputs.to)) } }),
  "core/code_span": (n) => ({ tag: "code", words: text(n.inputs.src) }),
  "core/line_break": () => ({ tag: "br" }),
  // controls
  "core/label": (n) => ({ tag: "span", words: text(n.inputs.value), attrs: { class: "label", style: tone(n.inputs.tone) } }),
  "core/press": (n) => {
    const check = name(n.inputs.kind) === "CHECK";
    const act = { disabled: on(n.inputs.disabled) ? "" : undefined, "data-click": n.events?.includes("click") ? "" : undefined };
    return { tag: "button", words: text(n.inputs.label), attrs: { type: "button", role: check ? "checkbox" : undefined, "aria-checked": check ? String(on(n.inputs.selected)) : undefined, ...act } };
  },
  "core/field": (n) => ({ tag: "input", attrs: { type: "text", "aria-label": text(n.inputs.label), placeholder: text(n.inputs.placeholder) || undefined, value: text(n.inputs.value), "data-write": "value" } }),
  "core/slider": (n) => {
    // its ends are what a write of it has to fit
    const { range, step } = n.writes?.value ?? {};
    return { tag: "input", attrs: { type: "range", "aria-label": text(n.inputs.label), min: text(range?.low) || "0", max: text(range?.high) || "1", step: text(step) || "any", value: text(n.inputs.value), "data-write": "value" } };
  },
  "core/flex": (n) => ({ tag: "div", holds: "children", attrs: { class: name(n.inputs.axis) === "X" ? "row" : "column" } }),
};

/** A node as an element: the widget's own, or a notice in the place of one that failed. */
export function shape(n) {
  if (n.failed !== undefined) return { tag: "p", words: `${n.widget.split("/").pop()} failed: ${n.failed}`, attrs: { role: "alert" } };
  return widgets[n.widget](n);
}

/** Whether this host draws a widget: the page shows the rest as their Markdown. */
export const supplies = (widget) => Object.hasOwn(widgets, widget);
