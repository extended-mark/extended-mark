// Markdown's own blocks, drawn as the HTML elements they are: p, h1–h6, ul/ol/li with task boxes,
// blockquote, pre>code, table, footnotes and hr. Props are read as markdown.ts reads them, so the
// page's words and its Markdown agree.

import type { Node as Drawn } from "@extended-mark/runtime";
import type { Widget, WidgetContext, WidgetView } from "../api.js";
import { attr, css, h, kit, text } from "../kit.js";
import { noteId, texOf } from "../rich.js";
import { drawTex } from "../tex.js";
import { flag, inner, kase, list, nodes, num, str } from "../values.js";

const ALIGN: Record<string, string> = { START: "start", CENTER: "center", END: "end", STRETCH: "justify", BASELINE: "start" };

/** An element that holds a sentence. */
function sentence<E extends HTMLElement>(el: E): E {
  el.classList.add("xtmd-sentence");
  return el;
}

export const paragraph: Widget = (d, ctx) => {
  const el = sentence(h("p", "paragraph"));
  const view: WidgetView = {
    el,
    update(d) {
      ctx.children(el, nodes(d.inputs.value));
      css(el, "color", kit(ctx).tokens.color(d.inputs.tone));
      const a = kase(d.inputs.align) ?? "START";
      css(el, "text-align", a === "START" ? undefined : ALIGN[a]);
    },
  };
  view.update(d);
  return view;
};

export const heading: Widget = (d, ctx) => {
  const level = () => Math.min(6, Math.max(1, Math.round(num(d.inputs.level) ?? 1)));
  let at = level();
  let el: HTMLElement = sentence(h(`h${at}` as "h1", "heading"));
  const view = {
    get el() {
      return el;
    },
    update(next: Drawn) {
      d = next;
      const l = level();
      if (l !== at) {
        // another level is another element: the mount puts the new one in the old one's place
        kit(ctx).release(el);
        el = sentence(h(`h${l}` as "h1", "heading"));
        at = l;
      }
      ctx.children(el, nodes(d.inputs.value));
    },
  };
  view.update(d);
  return view;
};

export const bullets: Widget = (d, ctx) => {
  const ordered = () => flag(d.inputs.ordered);
  let isOl = ordered();
  let el: HTMLElement = h(isOl ? "ol" : "ul", "bullets");
  const view = {
    get el() {
      return el;
    },
    update(next: Drawn) {
      d = next;
      if (ordered() !== isOl) {
        // ul and ol are different elements: the items are drawn again into the new one
        isOl = ordered();
        kit(ctx).release(el);
        el = h(isOl ? "ol" : "ul", "bullets");
      }
      const start = num(d.inputs.start) ?? 1;
      attr(el, "start", isOl && start !== 1 ? String(start) : undefined);
      const items = nodes(d.inputs.items);
      el.classList.toggle("xtmd-tasks", items.some((i) => i.widget === "core/item" && inner(i.inputs.done) !== undefined));
      ctx.children(el, items);
    },
  };
  view.update(d);
  return view;
};

export const item: Widget = (d, ctx) => {
  const el = h("li", "item");
  const body = h("div", "item.body");
  let box: HTMLInputElement | undefined;
  el.append(body);
  const view: WidgetView = {
    el,
    update(d) {
      const done = inner(d.inputs.done);
      if (done && "bool" in done) {
        if (!box) {
          // a task box shows the item's state; ticking it is for a press, so it can't be changed here
          box = h("input", "item.check", { type: "checkbox", disabled: "" });
          box.setAttribute("aria-label", done.bool ? "Done" : "Not done");
          el.prepend(box);
        }
        box.checked = done.bool;
        attr(box, "aria-label", done.bool ? "Done" : "Not done");
        el.classList.add("xtmd-task");
      } else if (box) {
        box.remove();
        box = undefined;
        el.classList.remove("xtmd-task");
      }
      ctx.children(body, nodes(d.inputs.body));
    },
  };
  view.update(d);
  return view;
};

export const quote: Widget = (d, ctx) => {
  const el = h("blockquote", "quote");
  const view: WidgetView = {
    el,
    update(d) {
      ctx.children(el, nodes(d.inputs.body));
    },
  };
  view.update(d);
  return view;
};

export const code: Widget = (d) => {
  const el = h("pre", "code", { tabindex: "0" });
  const c = h("code", "code.text");
  el.append(c);
  const view: WidgetView = {
    el,
    update(d) {
      const lang = str(d.inputs.lang).trim().split(/\s+/)[0] ?? "";
      const cls = lang ? `xtmd-code-text language-${lang.replace(/[^\w+#.-]/g, "")}` : "xtmd-code-text";
      if (c.className !== cls) c.className = cls;
      attr(el, "data-lang", lang || undefined);
      text(c, str(d.inputs.src));
    },
  };
  view.update(d);
  return view;
};

export const math: Widget = (d) => {
  const el = h("div", "math");
  el.classList.add("xtmd-math");
  const view: WidgetView = {
    el,
    update(d) {
      drawTex(el, texOf(d.inputs.tex), true);
    },
  };
  view.update(d);
  return view;
};

export const table: Widget = (d, ctx) => {
  // the wrapper scrolls a wide table on a narrow screen, the page itself never does
  const el = h("div", "table", { tabindex: "0" });
  const t = h("table", "table.grid");
  el.append(t);
  let shape = "";
  let cells: HTMLElement[][] = [];
  const view: WidgetView = {
    el,
    update(d) {
      const rows = [list(d.inputs.head), ...list(d.inputs.rows).map((r) => list(r))];
      const align = list(d.inputs.align).map((a) => kase(a));
      // the same rows and columns keep their cells, and what is in each follows the page
      const now = JSON.stringify([rows.map((r) => r.length), align]);
      if (now !== shape) {
        shape = now;
        kit(ctx).release(t);
        cells = rows.map((r, j) =>
          r.map((_, k) => {
            const cell = sentence(j === 0 ? h("th", "table.header", { scope: "col" }) : h("td", "table.cell"));
            const a = align[k];
            if (a === "CENTER" || a === "END") cell.style.textAlign = a === "CENTER" ? "center" : "end";
            return cell;
          }),
        );
        const head = h("thead");
        const body = h("tbody");
        cells.forEach((r, j) => {
          const tr = h("tr");
          tr.append(...r);
          (j === 0 ? head : body).append(tr);
        });
        t.replaceChildren(head, body);
      }
      rows.forEach((r, j) => r.forEach((c, k) => ctx.children(cells[j][k], nodes(c))));
    },
  };
  view.update(d);
  return view;
};

export const footnote: Widget = (d, ctx) => {
  const el = h("div", "footnote", { role: "note" });
  const mark = h("a", "footnote.mark");
  const body = h("div", "footnote.body");
  el.append(mark, body);
  const view: WidgetView = {
    el,
    update(d) {
      const m = str(d.inputs.mark);
      const uid = kit(ctx).uid;
      attr(el, "id", noteId(uid, m));
      attr(el, "aria-label", `Footnote ${m}`);
      // back to where the note is marked
      attr(mark, "href", `#${uid}fnref-${encodeURIComponent(m)}`);
      text(mark, m);
      ctx.children(body, nodes(d.inputs.body));
    },
  };
  view.update(d);
  return view;
};

export const divider: Widget = () => ({ el: h("hr", "divider"), update() {} });
