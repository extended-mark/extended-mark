// What a sentence is made of, drawn as the inline elements they are: text, strong, em, s, a, code, img,
// a footnote's mark and br. Nothing in a value is ever read as HTML.

import type { Drawn } from "@extended-mark/runtime";
import type { Widget, WidgetContext, WidgetView } from "../api.js";
import { attr, h, kit, text } from "../kit.js";
import { noteId, safeHref, safeSrc, texOf } from "../rich.js";
import { drawTex } from "../tex.js";
import { str } from "../values.js";

/** Words are text, with no element of their own. */
export const words: Widget = (d) => {
  const el = document.createTextNode(str(d.props.get("value")));
  return {
    el,
    update(d) {
      text(el, str(d.props.get("value")));
    },
  };
};

function mark(tag: "strong" | "em" | "s"): Widget {
  return (d, ctx) => {
    const el = document.createElement(tag);
    const view: WidgetView = {
      el,
      update(d) {
        ctx.children(el, d.slots.get("body") ?? []);
      },
    };
    view.update(d);
    return view;
  };
}

export const bold = mark("strong");
export const italic = mark("em");
export const strike = mark("s");

/** A link that may be followed is an `a`, and any other its words alone. */
export const link: Widget = (d, ctx) => {
  const safe = (d: Drawn) => safeHref(str(d.props.get("to")));
  const make = (href: string | undefined): HTMLElement => {
    const el = document.createElement(href ? "a" : "span");
    if (!href) el.setAttribute("data-unsafe-link", "");
    return el;
  };
  let was = safe(d);
  let el = make(was);
  const view = {
    get el() {
      return el;
    },
    update(d: Drawn) {
      const href = safe(d);
      if (!href !== !was) {
        kit(ctx).release(el);
        el = make(href);
      }
      was = href;
      if (href) {
        attr(el, "href", href);
        attr(el, "rel", /^https?:/i.test(href) && "noopener noreferrer");
      }
      ctx.children(el, d.slots.get("body") ?? []);
    },
  };
  view.update(d);
  return view;
};

export const codeSpan: Widget = (d) => {
  const el = document.createElement("code");
  const view: WidgetView = {
    el,
    update(d) {
      text(el, str(d.props.get("src")));
    },
  };
  view.update(d);
  return view;
};

export const inlineMath: Widget = (d) => {
  const el = document.createElement("span");
  el.className = "xtmd-math";
  let was: string | undefined;
  const view: WidgetView = {
    el,
    update(d) {
      const tex = texOf(d.props.get("tex"));
      if (tex === was) return;
      was = tex;
      drawTex(el, tex, false);
    },
  };
  view.update(d);
  return view;
};

export const inlineImage: Widget = (d) => {
  const el = document.createElement("img");
  const view: WidgetView = {
    el,
    update(d) {
      attr(el, "alt", str(d.props.get("alt")));
      attr(el, "src", safeSrc(str(d.props.get("src"))));
    },
  };
  view.update(d);
  return view;
};

export const noteMark: Widget = (d, ctx: WidgetContext) => {
  const el = document.createElement("sup");
  el.className = "xtmd-note-ref";
  const a = h("a");
  el.append(a);
  const view: WidgetView = {
    el,
    update(d) {
      const mark = str(d.props.get("mark"));
      const uid = kit(ctx).uid;
      attr(a, "href", `#${noteId(uid, mark)}`);
      attr(a, "id", `${uid}fnref-${encodeURIComponent(mark)}`);
      attr(a, "aria-label", `Footnote ${mark}`);
      text(a, mark);
    },
  };
  view.update(d);
  return view;
};

export const lineBreak: Widget = () => ({ el: document.createElement("br"), update() {} });
