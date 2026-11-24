// Markdown's own blocks, as the HTML elements they are: p, h1–h6, ul/ol/li with task boxes,
// blockquote, pre>code, table, footnotes and hr, as @extended-mark/dom draws them (the same parts and
// classes, so the same stylesheet). Props are read as markdown.ts reads them, so the page's words
// and its Markdown agree.

import { flag, inner, kase, list, nodes, noteId, num, str, texOf } from "@extended-mark/dom/shared";
import type { ViewWidget } from "../render.js";
import type { Act } from "../act.js";
import { cls, css, h, prop, type VNode } from "../vnode.js";
import { withTex } from "./rich.js";

const ALIGN: Record<string, string> = { START: "start", CENTER: "center", END: "end", STRETCH: "justify", BASELINE: "start" };

/** An element that holds a sentence. */
const sentence = (v: VNode): VNode => cls(v, "xtmd-sentence");

export const paragraph: ViewWidget = (d, ctx) => {
  const el = sentence(h("p", "paragraph", {}, ctx.children(nodes(d.inputs.value))));
  css(el, "color", ctx.tokens.color(d.inputs.tone));
  const a = kase(d.inputs.align) ?? "START";
  css(el, "text-align", a === "START" ? undefined : ALIGN[a]);
  return el;
};

export const heading: ViewWidget = (d, ctx) => {
  // another level is another element: the renderer puts the new one in the old one's place
  const level = Math.min(6, Math.max(1, Math.round(num(d.inputs.level) ?? 1)));
  return sentence(h(`h${level}`, "heading", {}, ctx.children(nodes(d.inputs.value))));
};

export const bullets: ViewWidget = (d, ctx) => {
  // ul and ol are different elements: the items are drawn again into the new one
  const ordered = flag(d.inputs.ordered);
  const start = num(d.inputs.start) ?? 1;
  const items = nodes(d.inputs.items);
  const el = h(ordered ? "ol" : "ul", "bullets", { start: ordered && start !== 1 ? String(start) : undefined }, ctx.children(items));
  return cls(el, "xtmd-tasks", items.some((i) => i.widget === "core/item" && inner(i.inputs.done) !== undefined));
};

export const item: ViewWidget = (d, ctx) => {
  const done = inner(d.inputs.done);
  const body = h("div", "item.body", {}, ctx.children(nodes(d.inputs.body)));
  // keyed, so the body is kept as the task box comes and goes before it
  body.key = "body";
  const el = h("li", "item", {}, [body]);
  if (done && "bool" in done) {
    // a task box shows the item's state; ticking it is for a press, so it can't be changed here
    const box = h("input", "item.check", { type: "checkbox", "aria-label": done.bool ? "Done" : "Not done" });
    box.key = "check";
    prop(box, "disabled", true);
    prop(box, "checked", done.bool);
    el.children.unshift(box);
    cls(el, "xtmd-task");
  }
  return el;
};

export const quote: ViewWidget = (d, ctx) => h("blockquote", "quote", {}, ctx.children(nodes(d.inputs.body)));

export const code: ViewWidget = (d) => {
  const lang = str(d.inputs.lang).trim().split(/\s+/)[0] ?? "";
  const c = h("code", "code.text", {}, [str(d.inputs.src)]);
  cls(c, `language-${lang.replace(/[^\w+#.-]/g, "")}`, !!lang);
  return h("pre", "code", { tabindex: "0", "data-lang": lang || undefined }, [c]);
};

export const math: ViewWidget = (d) => withTex(h("div", "math"), texOf(d.inputs.tex), true);

export const table: ViewWidget = (d, ctx) => {
  const align = list(d.inputs.align).map((a) => kase(a));
  const place = (cell: VNode, k: number): VNode => {
    const a = align[k];
    return css(cell, "text-align", a === "CENTER" ? "center" : a === "END" ? "end" : undefined);
  };
  const head = h("thead", undefined, {}, [
    h("tr", undefined, {}, list(d.inputs.head).map((c, k) => place(sentence(h("th", "table.header", { scope: "col" }, ctx.children(nodes(c)))), k))),
  ]);
  const body = h(
    "tbody",
    undefined,
    {},
    list(d.inputs.rows).map((r) => h("tr", undefined, {}, list(r).map((c, k) => place(sentence(h("td", "table.cell", {}, ctx.children(nodes(c)))), k)))),
  );
  // the wrapper scrolls a wide table on a narrow screen, the page itself never does
  return h("div", "table", { tabindex: "0" }, [h("table", "table.grid", {}, [head, body])]);
};

export const footnote: ViewWidget = (d, ctx) => {
  const m = str(d.inputs.mark);
  const uid = ctx.uid;
  return h("div", "footnote", { role: "note", id: noteId(uid, m), "aria-label": `Footnote ${m}` }, [
    // back to where the note is marked
    h("a", "footnote.mark", { href: `#${uid}fnref-${encodeURIComponent(m)}` }, [m]),
    h("div", "footnote.body", {}, ctx.children(nodes(d.inputs.body))),
  ]);
};

export const divider: ViewWidget = () => h("hr", "divider");

// ---- acting through the element, for the checks

/** Markdown's blocks take no reader actions of their own: what is in them acts for itself. */
export const acts: Record<string, Act> = {};
