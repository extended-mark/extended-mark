// Elements made and changed the DOM's own way, for a renderer that has no other.

import { composing } from "./composing.js";
import type { Host, Reads } from "./patch.js";

const SVG = "http://www.w3.org/2000/svg";

/** How elements are made and changed here: the DOM's own way. */
export const domHost: Host<Node> = {
  live: true,
  createElement: (tag, svg) => (svg ? document.createElementNS(SVG, tag) : document.createElement(tag)),
  createText: (text) => document.createTextNode(text),
  setText: (node, text) => {
    if (node.nodeValue !== text) node.nodeValue = text;
  },
  setAttribute: (el, name, value) => (el as Element).setAttribute(name, value),
  removeAttribute: (el, name) => (el as Element).removeAttribute(name),
  setStyle: (el, prop, value) => (el as HTMLElement).style.setProperty(prop, value),
  removeStyle: (el, prop) => (el as HTMLElement).style.removeProperty(prop),
  // what is typed stays while a person composes it, and a value that is there already is never set again:
  // that would move the caret
  setProperty: (el, name, value) => {
    const live = el as unknown as Record<string, unknown>;
    if (name === "value" && composing(el)) return;
    if (live[name] !== value) live[name] = value;
  },
  listen(el, event, f) {
    el.addEventListener(event, f);
    return () => el.removeEventListener(event, f);
  },
  insertBefore: (parent, child, before) => void parent.insertBefore(child, before),
  removeChild: (_parent, child) => (child as ChildNode).remove(),
};

/** What has focus inside the page, and the caret in it, before a redraw moves anything. */
export function focusOf(root: HTMLElement): (() => void) | undefined {
  const a = root.ownerDocument.activeElement as HTMLElement | null;
  if (!a || a === root || !root.contains(a)) return undefined;
  const typed = a instanceof HTMLInputElement || a instanceof HTMLTextAreaElement ? a : undefined;
  let sel: [number | null, number | null, "forward" | "backward" | "none" | null] | undefined;
  try {
    if (typed) sel = [typed.selectionStart, typed.selectionEnd, typed.selectionDirection];
  } catch {
    // a range input has no caret
  }
  return () => {
    if (!a.isConnected || root.ownerDocument.activeElement === a) return;
    a.focus({ preventScroll: true });
    if (typed && sel && sel[0] !== null && sel[1] !== null) {
      try {
        typed.setSelectionRange(sel[0], sel[1], sel[2] ?? undefined);
      } catch {
        // not every input keeps a caret
      }
    }
  };
}

/** What is there already, as a server wrote it. */
export const domReads: Reads<Node> = {
  tag: (n) => (n.nodeType === 1 ? (n as Element).localName : "#"),
  text: (n) => (n.nodeType === 3 ? (n.nodeValue ?? "") : undefined),
  attribute: (n, name) => (n as Element).getAttribute(name),
  names: (n) => (n as Element).getAttributeNames(),
  children: (n) => [...n.childNodes],
};
