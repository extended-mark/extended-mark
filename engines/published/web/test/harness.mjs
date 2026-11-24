// In the tab: the page the runtime runs, and the one the engine built.

import { run } from "@extended-mark/dom";
import { everyNode, nodesIn } from "@extended-mark/runtime";
import { widgets } from "@extended-mark/widgets";

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

function random(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const pick = (xs, rnd) => xs[Math.floor(rnd() * xs.length)];

/** A value like another, and sometimes one that isn't: a host gives what it likes. */
function like(x, rnd, deep = 0) {
  if (rnd() < 0.06) return pick([{ num: "1.5" }, { text: "?" }, { bool: true }, { list: [] }, { none: {} }], rnd);
  if ("num" in x) return { num: pick(["0", "1", "2", "3", "7", "-1", "10", "0.5", "100"], rnd) };
  if ("text" in x) return { text: pick(["", "a", "lemon", "a *b* <i>", "Ω≈ç"], rnd) };
  if ("bool" in x) return { bool: rnd() < 0.5 };
  if ("list" in x && deep < 4) {
    const of = x.list.map((y) => (rnd() < 0.3 ? like(y, rnd, deep + 1) : y));
    const r = rnd();
    if (r < 0.2) of.pop();
    else if (r < 0.4 && of.length) of.push(like(of[0], rnd, deep + 1));
    else if (r < 0.6) of.reverse();
    return { list: of };
  }
  if ("record" in x && deep < 4) return { record: { type: x.record.type, fields: Object.fromEntries(Object.entries(x.record.fields).map(([k, y]) => [k, rnd() < 0.4 ? like(y, rnd, deep + 1) : y])) } };
  if ("some" in x) return rnd() < 0.3 ? { none: {} } : { some: like(x.some, rnd, deep + 1) };
  return x;
}

/** Something a reader or a host could do now: a node's place among the elements, and the change. */
function changeOf(rnd, ir) {
  const acts = [() => ({ page: { values: { theme: { case: { enum: "core/theme", name: pick(["DARK", "LIGHT"], rnd) } }, width: { case: { enum: "core/width", name: pick(["NARROW", "MEDIUM", "WIDE"], rnd) } } } } })];
  const state = it.page.state();
  for (const k of Object.keys(state)) acts.push(() => ({ set: { fields: { [k]: like(state[k], rnd) } } }));
  if (Object.keys(state).length > 1) acts.push(() => ({ set: { fields: Object.fromEntries(Object.entries(state).map(([k, x]) => [k, like(x, rnd)])) } }));
  const past = kept;
  if (past?.length) acts.push(() => ({ restore: { data: pick(past, rnd) } }));
  for (const d of ir.declarations) {
    const x = d.declare;
    if ("expose" in x) for (const f of x.expose.fns) acts.push(() => ({ expose: { channel: x.expose.channel, name: f.name, args: f.params.map((p) => like({ "core/text": { text: "" }, "core/bool": { bool: true } }[p.type.name] ?? { num: "1" }, rnd)) } }));
    if ("extern" in x && "channel" in x.extern) for (const v of x.extern.channel.values ?? []) if (v.default && !x.extern.channel.name.startsWith("core/")) acts.push(() => ({ extern: { channel: x.extern.channel.name, values: { [v.name]: like(v.default, rnd) } } }));
  }
  const of = sites(it.root);
  for (const node of everyNode(it.page.tree().nodes)) {
    const at = of.indexOf(node.id);
    if (at < 0) continue;
    const decl = ir.declarations.map((d) => d.declare.extern?.widget).find((w) => w?.name === node.widget);
    for (const e of node.events ?? []) {
      const params = decl?.events?.find((x) => x.name === e)?.params ?? [];
      const arg = (p) => ({ "core/text": { text: "x" }, "core/point": { record: { type: "core/point", fields: { x: { num: "1" }, y: { num: "2" } } } } })[p.type.name] ?? { num: "1" };
      for (let k = 0; k < 2; k++) acts.push(() => ({ event: { at, name: e, args: params.map(arg) } }));
    }
    for (const w of Object.keys(node.writes ?? {})) {
      const now = node.inputs[w];
      if (now && w !== "box" && w !== "drawn") acts.push(() => ({ write: { at, input: w, value: like(now, rnd) } }));
    }
  }
  return pick(acts, rnd)();
}

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
    return changeOf(rnd, ir);
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
void nodesIn;
void widgets;
