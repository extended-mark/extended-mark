// The widget set without a page, a browser or a renderer: a widget drawn from a node alone, elements as HTML
// and as JSON, and the patcher that follows them.

import { test } from "node:test";
import assert from "node:assert/strict";
import { Patcher, children, h, toHtml, toJson, widgets, Tokens } from "../dist/index.js";

test("toHtml escapes text and attributes, writes properties and style, and leaves a void element open", () => {
  const v = h("p", "label", { title: 'a "b"' }, ["<i> & </i>", { tag: "input", attrs: {}, props: { value: "x<", disabled: true, checked: false }, children: [] }]);
  v.style = { "text-align": "end" };
  assert.equal(toHtml(v), '<p data-part="label" class="xtmd-label" title="a &quot;b&quot;" style="text-align:end">&lt;i&gt; &amp; &lt;/i&gt;<input value="x&lt;" disabled></p>');
  // a stylesheet's text is its own
  assert.equal(toHtml({ tag: "style", attrs: {}, children: ["a > b {}"] }), "<style>a > b {}</style>");
});

test("the patcher keeps an element by key, sets only what changed, and moves only what is out of place", () => {
  const ops = [];
  let made = 0;
  const host = {
    live: true,
    createElement: (tag) => ({ tag, id: ++made, kids: [], attrs: {} }),
    createText: (text) => ({ text, kids: [] }),
    setText: (n, t) => (n.text = t),
    setAttribute: (n, k, v) => (ops.push(`set ${k}`), (n.attrs[k] = v)),
    removeAttribute: (n, k) => (ops.push(`unset ${k}`), delete n.attrs[k]),
    setStyle: () => {},
    removeStyle: () => {},
    setProperty: () => {},
    listen: () => () => {},
    insertBefore: (p, c, before) => {
      ops.push(`move ${c.tag ?? "text"}${c.id ?? ""}`);
      p.kids = p.kids.filter((x) => x !== c);
      p.kids.splice(before ? p.kids.indexOf(before) : p.kids.length, 0, c);
    },
    removeChild: (p, c) => (ops.push(`remove ${c.id}`), (p.kids = p.kids.filter((x) => x !== c))),
  };
  const li = (key, cls) => ({ ...h("li", undefined, { class: cls }, [key]), key });
  const parent = { kids: [] };
  const p = new Patcher(host, parent);
  p.render(h("ul", undefined, {}, [li("a", "x"), li("b", "x"), li("c", "x")]));
  const [a, b, c] = parent.kids[0].kids;
  ops.length = 0;
  // c moves to the front and b's class changes: a and b stay where they are, and nothing is made
  p.render(h("ul", undefined, {}, [li("c", "x"), li("a", "x"), li("b", "y")]));
  assert.deepEqual(parent.kids[0].kids, [c, a, b]);
  assert.equal(made, 4);
  assert.deepEqual(ops, ["set class", `move li${c.id}`]);
  assert.equal(b.attrs.class, "y");
  // a key gone is an element removed
  ops.length = 0;
  p.render(h("ul", undefined, {}, [li("c", "x"), li("b", "y")]));
  assert.deepEqual(parent.kids[0].kids, [c, b]);
  assert.deepEqual(ops, [`remove ${a.id}`]);
});

test("the patcher leaves an element that is the same one alone, and sets what is live at every render", () => {
  const sets = [];
  const host = {
    live: true,
    createElement: (tag) => ({ tag, kids: [] }),
    createText: (text) => ({ text }),
    setText: () => sets.push("text"),
    setAttribute: (n, k) => sets.push(k),
    removeAttribute: () => {},
    setStyle: () => {},
    removeStyle: () => {},
    setProperty: (n, k, v) => sets.push(`${k}=${v}`),
    listen: () => () => {},
    insertBefore: (p, c) => p.kids.push(c),
    removeChild: () => {},
  };
  const p = new Patcher(host, { kids: [] }, true);
  const field = { ...h("input"), key: "f", props: { value: "a" } };
  const other = { ...h("p", undefined, { title: "t" }, ["x"]), key: "p" };
  p.renderAll([field, other]);
  sets.length = 0;
  // the same objects: nothing is looked at, but a field shows what the page has, whatever was typed in it
  p.renderAll([field, other]);
  assert.deepEqual(sets, ["value=a"]);
  p.renderAll([field, { ...h("p", undefined, { title: "u" }, ["x"]), key: "p" }]);
  assert.deepEqual(sets, ["value=a", "title", "value=a"]);
  p.renderAll([other]);
  sets.length = 0;
  p.renderAll([other]);
  assert.deepEqual(sets, []);
});

/** A drawing with nothing around it, and what its widgets did. */
function drawing(over = {}) {
  const did = [];
  const source = { ir: { declarations: [] }, given: () => ({}), token: () => undefined };
  const dr = {
    registry: widgets,
    tokens: new Tokens(source),
    uid: "x-",
    kept: new WeakMap(),
    node: () => undefined,
    event: (...a) => did.push(["event", ...a]),
    write: (...a) => did.push(["write", ...a]),
    call: () => ({ none: {} }),
    layout: () => {},
    fail: (f) => did.push(["fail", f.message]),
    ...over,
  };
  return { dr, did };
}
const node = (id, widget, inputs, more = {}) => ({ id, widget, inputs, ...more });

test("a widget is drawn from its node alone, and its events go to whoever drew it", () => {
  const { dr, did } = drawing();
  const press = node("s0", "core/press", { label: { text: "Go" }, kind: { case: { enum: "core/press_kind", name: "BUTTON" } } }, { events: ["click"] });
  const [v] = children(dr, [press]);
  assert.match(toHtml(v), /^<button [^>]*data-xtmd-site="s0"[^>]*>.*Go/);
  assert.deepEqual(toJson(v).element.listens, ["click"]);
  v.on.click({ currentTarget: {} });
  assert.deepEqual(did, [["event", "s0", "click", []]]);
  // the same node is drawn once
  assert.equal(children(dr, [press])[0], v);
  assert.notEqual(children(dr, [{ ...press }])[0], v);
});

test("one that throws, one that failed and one nobody draws say so in their place, and the rest is drawn", () => {
  const { dr, did } = drawing({ registry: { ...widgets, "acme/bad": () => { throw new Error("no"); } } });
  const label = (id) => node(id, "core/label", { value: { text: id } });
  const html = children(dr, [label("a"), node("b", "acme/bad", {}), node("c", "core/label", {}, { failed: "too deep" }), node("d", "acme/none", {}), label("e")]).map(toHtml).join("\n");
  assert.equal((html.match(/role="alert"/g) ?? []).length, 3);
  assert.ok(html.includes("bad couldn&#39;t be drawn: no") && html.includes("label failed: too deep") && html.includes("Nothing draws acme/none"), html);
  assert.deepEqual(did, [["fail", "no"]]);
});

test("a widget replaced by its full name is drawn by the host's, with ours around it", () => {
  const mine = { ...widgets, "core/label": (d) => h("mark", undefined, {}, [d.inputs.value.text]) };
  const { dr } = drawing({ registry: mine });
  const html = toHtml(children(dr, [node("q", "core/quote", { body: { nodes: [node("l", "core/label", { value: { text: "hi" } })] } })])[0]);
  assert.match(html, /^<blockquote[^>]*><mark[^>]*>hi<\/mark><\/blockquote>$/);
});
