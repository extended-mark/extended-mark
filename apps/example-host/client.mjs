// The browser's side: it makes the same page from the same IR and data, takes over the elements the server
// wrote by their ids, hears the reader, and does what each change says to the elements it is about.

import { Page } from "@extended-mark/runtime";
import { nodes, shape, supplies } from "./draw.mjs";

const read = (id) => JSON.parse(document.getElementById(id).textContent);
const root = document.getElementById("page");
const data = read("data");
const page = new Page(read("ir"), { externs: { "doc/clock": { today: { text: "Tuesday" } } }, supplies, state: data.state });
page.restoreOwn(data.own ?? {});

/** The elements by their nodes' ids. */
const byId = new Map([...root.querySelectorAll("[data-id]")].map((el) => [el.dataset.id, el]));
const missing = [];
for (const n of page.tree().nodes) if (!byId.has(n.id)) missing.push(n.id);

/** A node's own element: its attributes and words, and not what it holds. */
function own(el, n) {
  const { attrs = {}, words, holds } = shape(n);
  for (const { name } of [...el.attributes]) if (name !== "data-id" && attrs[name] === undefined) el.removeAttribute(name);
  for (const [k, v] of Object.entries(attrs)) if (v !== undefined && el.getAttribute(k) !== String(v)) el.setAttribute(k, v);
  // what is typed is the element's own, and follows the page's only when they differ
  if (attrs.value !== undefined && el.value !== attrs.value) el.value = attrs.value;
  if (!holds && words !== undefined && el.textContent !== words) el.textContent = words;
}

/** A node drawn anew, and all in it. */
function make(n) {
  const { tag, holds } = shape(n);
  const el = document.createElement(tag);
  el.dataset.id = n.id;
  byId.set(n.id, el);
  own(el, n);
  if (holds) el.append(...nodes(n.inputs[holds]).map(make));
  return el;
}

function forget(el) {
  for (const x of [el, ...el.querySelectorAll("[data-id]")]) if (byId.get(x.dataset.id) === x) byId.delete(x.dataset.id);
}

/** An element's widgets: the ones kept, moved only when out of place, and the ones drawn anew. */
function fill(el, children) {
  const kept = new Set(children.filter((c) => c.keep !== undefined).map((c) => byId.get(c.keep)));
  for (const old of [...el.children]) {
    if (kept.has(old)) continue;
    forget(old);
    old.remove();
  }
  let at = el.firstElementChild;
  for (const c of children) {
    const next = c.keep !== undefined ? byId.get(c.keep) : make(c.node);
    if (next === at) at = at.nextElementSibling;
    else el.insertBefore(next, at);
  }
}

page.onChange((changes) => {
  for (const c of changes) {
    const el = c.id === "" ? root : byId.get(c.id);
    if (!el) continue; // inside a node drawn anew, which has it already
    const n = c.id === "" ? undefined : page.node(c.id);
    if (n) {
      // another level of heading is another element
      const { tag } = shape(n);
      if (tag !== el.tagName.toLowerCase()) {
        el.replaceWith(make(n));
        continue;
      }
      own(el, n);
    }
    for (const children of Object.values(c.children ?? {})) fill(el, children);
  }
  for (const call of page.takeCalls()) document.dispatchEvent(new CustomEvent("host-call", { detail: call }));
});

/** The node an event is about: the nearest element of one. */
const nodeOf = (e) => e.target.closest?.("[data-id]");

root.addEventListener("click", (e) => {
  const el = nodeOf(e);
  if (el && !el.disabled && page.node(el.dataset.id)?.events?.includes("click")) page.event(el.dataset.id, "click");
});
root.addEventListener("input", (e) => {
  const el = nodeOf(e);
  if (el?.dataset.write) page.write(el.dataset.id, el.dataset.write, el.type === "range" ? { num: el.value } : { text: el.value });
});
root.addEventListener("keydown", (e) => {
  const el = nodeOf(e);
  if (e.key === "Enter" && el && page.node(el.dataset.id)?.events?.includes("submit")) page.event(el.dataset.id, "submit");
});

window.host = { page, missing };
