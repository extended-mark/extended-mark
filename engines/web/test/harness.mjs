// In the tab: the page the runtime runs, and the one the engine built.

import { run } from "@extended-mark/dom";
import { changeOf, random } from "./changes.mjs";

/** The page in this tab: its element, and what runs it; a page that never changes has nothing to. */
let it;

/** Ids with what differs between two pages out of them: an item's number, and the page's own prefix. */
const plain = (s) => s.replace(/xtmd[a-z]*\d+-/g, "xtmd-").replace(/\[\d+\]/g, "[#]");

/** What isn't compared as it is written: a control's value is what it has, and a shade is painted in its own time. */
const OWN = new Set(["value", "checked", "data-samples", "data-state"]);

function html(root) {
  const copy = root.cloneNode(true);
  for (const s of copy.querySelectorAll("style")) s.remove();
  // a canvas's and a shade's pixels, and what is measured, are the widgets' own
  const line = (n, depth) => {
    const pad = "  ".repeat(depth);
    if (n.nodeType === 3) return n.nodeValue ? `${pad}${JSON.stringify(n.nodeValue)}\n` : "";
    if (n.nodeType !== 1) return "";
    const attrs = [...n.attributes].filter((a) => !OWN.has(a.name) && a.name !== "style" && !(n.localName === "canvas" && (a.name === "width" || a.name === "height"))).map((a) => `${a.name}=${JSON.stringify(a.value)}`).sort();
    // as the browser reads it, however it was written
    if (n.style?.length) attrs.push(`style=${JSON.stringify(n.style.cssText.split(";").map((x) => x.trim()).filter(Boolean).sort().join("; "))}`);
    return `${pad}<${n.localName} ${attrs.join(" ")}>\n${[...n.childNodes].map((c) => line(c, depth + 1)).join("")}`;
  };
  copy.normalize();
  return plain([...copy.childNodes].map((c) => line(c, 1)).join("")) + [...root.querySelectorAll("input, textarea")].map((e) => `${e.value}|${e.checked}`).join("\n");
}

const sortKeys = (x) => (Array.isArray(x) ? x.map(sortKeys) : x && typeof x === "object" ? Object.fromEntries(Object.keys(x).sort().map((k) => [k, sortKeys(x[k])])) : x);
const seen = (html_, data, told) => ({ html: html_, data: sortKeys(data), told: JSON.parse(plain(JSON.stringify(sortKeys(told)))) });

/** What there is to compare: what is on the page, its data, and what it told. */
const now = () => (it.page ? seen(html(it.root), it.page.data(), it.page.told()) : { html: html(it.root) });

const sites = (root) => [...root.querySelectorAll("[data-xtmd-site]")].map((e) => e.getAttribute("data-xtmd-site"));

window.side = {
  /** The page as the runtime runs it. */
  run(ir, given) {
    const root = document.querySelector("#page");
    it = { root, page: run({ ir }, root, given).page };
  },
  /** The page as the engine built it. */
  async take(script, given) {
    const root = document.querySelector("#page > *");
    it = { root, page: script ? (await import(new URL(script, location.href).href)).mount(root, given) : undefined };
  },
  now,
  change(ir, seed) {
    const rnd = random(seed);
    if (rnd() < 0.3) kept.push(it.page.data());
    return changeOf(rnd, ir, { state: it.page.state(), nodes: it.page.tree().nodes, ids: sites(it.root), past: kept });
  },
  /** A change, with this page's own ids. */
  act(a) {
    const [[kind, x]] = Object.entries(a);
    const { at, ...rest } = x;
    it.page?.act(at === undefined ? a : { [kind]: { ...rest, id: sites(it.root)[at] } });
    return now();
  },
};
const kept = [];
