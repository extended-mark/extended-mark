// Elements as HTML, for a server that renders without a framework, and for the tests: escaped,
// with void elements left open and hooks left for the browser.

import type { web } from "@extended-mark/runtime";
import type { Child, VNode } from "./vnode.js";

const VOID = new Set(["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"]);
const RAW = new Set(["style", "script"]);
const ESC: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ESC[c]);
}

/** A DOM property as the attribute that says it in HTML, where there is one. */
function propAttr(name: string, value: unknown): [string, string] | undefined {
  if (value === false || value === undefined || value === null) return undefined;
  if (name === "value") return ["value", String(value)];
  if (name === "checked" || name === "disabled" || name === "hidden" || name === "selected") return [name, ""];
  return undefined;
}

export function toHtml(c: Child): string {
  if (typeof c === "string") return escapeHtml(c);
  const attrs: [string, string][] = Object.entries(c.attrs);
  for (const [k, v] of Object.entries(c.props ?? {})) {
    const a = propAttr(k, v);
    if (a && !(c.tag === "textarea" && k === "value")) attrs.push(a);
  }
  if (c.style && Object.keys(c.style).length) attrs.push(["style", Object.entries(c.style).map(([k, v]) => `${k}:${v}`).join(";")]);
  const open = `<${c.tag}${attrs.map(([k, v]) => (v === "" && k !== "value" && k !== "alt" ? ` ${k}` : ` ${k}="${escapeHtml(v)}"`)).join("")}>`;
  if (VOID.has(c.tag) && !c.svg) return open;
  const inner = c.tag === "textarea" ? escapeHtml(String(c.props?.value ?? "")) : RAW.has(c.tag) ? c.children.map((x) => (typeof x === "string" ? x : toHtml(x))).join("") : c.children.map(toHtml).join("");
  return `${open}${inner}</${c.tag}>`;
}

/** Elements as `schemas/web.schema.json` writes them: what a widget set gives, with nothing to run. */
export function toJson(c: Child): web.Drawn {
  if (typeof c === "string") return { words: c };
  const e: web.Element = { tag: c.tag };
  if (c.svg) e.svg = true;
  if (c.key !== undefined) e.key = c.key;
  if (Object.keys(c.attrs).length) e.attrs = { ...c.attrs };
  if (c.props) e.props = Object.fromEntries(Object.entries(c.props).map(([k, v]): [string, web.Prop] => [k, typeof v === "boolean" ? { bool: v } : { text: String(v) }]));
  if (c.style && Object.keys(c.style).length) e.style = { ...c.style };
  if (c.on && Object.keys(c.on).length) e.listens = Object.keys(c.on);
  if (c.hook) e.hook = true;
  if (c.component) e.component = c.component.props.node.widget;
  if (c.children.length) e.children = c.children.map(toJson);
  return { element: e };
}

/** Walks a tree's elements, children first. */
export function walk(v: VNode, f: (v: VNode) => void): void {
  for (const c of v.children) if (typeof c !== "string") walk(c, f);
  f(v);
}
