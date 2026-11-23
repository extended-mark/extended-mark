// What a sentence is made of, as the inline elements @extended-mark/dom draws. Links and pictures are
// checked before use, and nothing in a value is ever read as HTML. TeX is given as its text, and drawn
// as MathML by a hook once the element is live.

import { noteId, safeHref, safeSrc, str, texOf, texToMathML } from "@extended-mark/dom/shared";
import type { ViewWidget } from "../render.js";
import { cls, h, type Hook, type VNode } from "../vnode.js";

/** What a TeX element shows now: the TeX it was drawn for, and its MathML (none: TeX this reader
 * doesn't know, shown as it is written). */
const drawn = new WeakMap<Element, { tex: string; math: Element | undefined }>();

/** Draws an element's `data-tex` as MathML, as @extended-mark/dom's drawTex does: read again only when the
 * TeX changed, and put back when a redraw put the render's text in its place. */
function texHook(display: boolean): Hook {
  const draw = (el: Element) => {
    const tex = el.getAttribute("data-tex") ?? "";
    let was = drawn.get(el);
    if (!was || was.tex !== tex) {
      const math = texToMathML(tex, display);
      math?.setAttribute("aria-hidden", "true");
      drawn.set(el, (was = { tex, math }));
    }
    if (was.math) {
      el.classList.remove("xtmd-math-tex");
      if (el.childNodes.length !== 1 || el.firstChild !== was.math) el.replaceChildren(was.math);
    } else {
      el.classList.add("xtmd-math-tex");
      if (el.childElementCount || el.textContent !== tex) el.textContent = tex;
    }
  };
  return { mount: draw, update: draw, unmount: (el) => drawn.delete(el) };
}

/** Makes an element a TeX one: the TeX as its accessible text and, until its hook draws the
 * MathML (and on a server, which never does), as what it shows. */
export function withTex(v: VNode, tex: string, display: boolean): VNode {
  v.attrs["data-tex"] = tex;
  v.attrs.role = "math";
  v.attrs["aria-label"] = tex;
  cls(v, "xtmd-math-tex");
  v.children = [tex];
  v.hook = texHook(display);
  return v;
}

/** Words are text, with no element of their own. */
export const words: ViewWidget = (d) => str(d.props.get("value"));

const mark = (tag: string): ViewWidget => (d, ctx) => h(tag, undefined, {}, ctx.children(d.slots.get("body") ?? []));

export const bold = mark("strong");
export const italic = mark("em");
export const strike = mark("s");

/** A link that may be followed is an `a`, and any other its words alone. */
export const link: ViewWidget = (d, ctx) => {
  const safe = safeHref(str(d.props.get("to")));
  const body = ctx.children(d.slots.get("body") ?? []);
  return safe ? h("a", undefined, { href: safe, rel: /^https?:/i.test(safe) ? "noopener noreferrer" : undefined }, body) : h("span", undefined, { "data-unsafe-link": "" }, body);
};

export const codeSpan: ViewWidget = (d) => h("code", undefined, {}, [str(d.props.get("src"))]);

export const inlineMath: ViewWidget = (d) => withTex(h("span", undefined, { class: "xtmd-math" }), texOf(d.props.get("tex")), false);

export const inlineImage: ViewWidget = (d) => h("img", undefined, { alt: str(d.props.get("alt")), src: safeSrc(str(d.props.get("src"))) });

export const noteMark: ViewWidget = (d, ctx) => {
  const note = str(d.props.get("mark"));
  return h("sup", undefined, { class: "xtmd-note-ref" }, [h("a", undefined, { href: `#${noteId(ctx.uid, note)}`, id: `${ctx.uid}fnref-${encodeURIComponent(note)}`, "aria-label": `Footnote ${note}` }, [note])]);
};

export const lineBreak: ViewWidget = () => h("br");
