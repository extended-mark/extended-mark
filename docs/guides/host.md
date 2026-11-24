# Writing a host

A host shows a page: it draws the widgets, hears the reader, and keeps what should be kept. This guide goes
from a document to one drawn by code of your own. Page 6 of the spec
([The host's side](../specs/06-hosts-packs-implementers.md)) is the contract, and
[apps/example-host](../../apps/example-host) is a whole host, server and browser, written from it alone.

You may not need one. The packages for [React, Vue, Svelte, Solid and Angular](../../engines/published/bindings)
and for [the plain web](../../engines/published/bindings/embed) are hosts, and each takes **your own
components** as widgets (`components`, or `widgets` on the plain web). Write a host to draw everything
yourself, or to draw somewhere these don't: a terminal, a native toolkit, a canvas, a test.

## 1. Build the page

```sh
xtmd build page.xtmd --out page.ir.json
xtmd build docs --out dist              # every document, core compiled once
```

The IR is static JSON. It never changes while the page runs: keep it where you keep other assets.

## 2. Make it, and draw the tree

A runtime runs the IR and gives you a **drawn tree**: plain JSON, its schema in
[schemas/tree.schema.json](../../schemas/tree.schema.json). On the web it is `@extended-mark/runtime`; any
engine's gives the same tree.

```js
import { Page } from "@extended-mark/runtime";

const page = new Page(ir, {
  supplies: (widget) => widget in mine,              // the widgets you draw; the rest show their Markdown
  state: saved?.state,                               // $state to start from: checked, and what doesn't fit is refused
  externs: { "doc/clock": { today: { text: "Tuesday" } } },
  page: { width: "WIDE", theme: "DARK" },
});
for (const node of page.tree().nodes) draw(node);
```

A node is a widget that is already worked out. You draw it from its `inputs`, and never ask what they mean:

```json
{
  "id": "m1/s0",
  "widget": "core/press",
  "inputs": { "label": { "text": "Add one" }, "tone": { "token": "core/color.ACCENT" }, "children": { "nodes": [] } },
  "events": ["click"]
}
```

- **Values** are in the IR's form: `{ "num": "3" }` (a number is exact, so it is text), `{ "text": "…" }`,
  `{ "bool": true }`, `{ "case": … }`, `{ "some": … }` or `{ "none": {} }`, `{ "list": […] }`, `{ "record": … }`.
- **A token is a name.** `page.token(value)` says what it stands for, and you may have your own colour for it.
- **Widgets inside** are nodes: `{ "nodes": […] }`, in any input, a slot's or a sentence's. A sentence is
  `core/words`, the marks around them, and any other widget among them.
- **`failed`** means it couldn't be drawn: show a notice with those words in its place.
- **Page 5** of the spec lists every core widget and what each input is for. Draw as few as you like: start
  with paragraphs, headings and words, and the rest is Markdown made of those.

## 3. Hear the reader

```js
page.event(id, "click");                              // a widget's event, one of its node's `events`
page.write(id, "value", { num: "7" });                // a widget writing an input, one of its node's `writes`
page.call(id, "shader", [at, px, args]);              // a fn input's answer
```

A write is checked against what its node's `writes` says (a slider's ends and step are there to draw), and is
done or refused. Make only what is in `events` look as if it can be pressed.

## 4. Follow what changed

```js
page.onChange((changes) => {
  for (const c of changes) update(c.id, c);           // "" is the page's own list of nodes
});
```

A change names a node and only what is different: `inputs` with new values, `children` (each a node kept, by
its id, or one drawn anew), `events`, `writes`. An id follows its item, so move what you drew for it rather
than drawing it again, and focus, scrolling and what is half typed stay. You can also read `page.tree()` again:
a node nothing changed in is the same object. `apply(tree, changes)` is the next tree from the last.

Nothing else is worked out: a change touches only what reads what changed, whatever else is on the page.

## 5. Keep it, and bring it back

```js
const saved = page.data();                            // { state, own }: $state, and each widget's own variables
const again = new Page(ir, { state: saved.state });
again.restoreOwn(saved.own);
```

`state` goes back into any version of the document, checked. `own` belongs to the IR it was taken from. Tell
IRs apart by their content, not by which object or file holds them.

A page that moves to a new xtmd may need its IR moved too: `xtmd migrate page.ir.json`, or build it again.

## 6. What it asks of you, and what went wrong

| | |
|---|---|
| `page.lacking()` | What its IR's `needs` lists that nobody gives: a channel's value, a pack's fn |
| `page.missing()` | The widgets on it you don't draw, each shown as its Markdown |
| `page.takeCalls()` | The fns of yours it called, as actions: `{ name, args }` |
| `page.takeOutcome()` | Writes refused, and failures, with where |
| `page.markdown()` | The whole page as Markdown, with the values it has now: for search, copying and readers of text |

## 7. On a server

The runtime needs no DOM. Draw the same tree to HTML, write each node's id on its element, and send the IR and
the data with it. The browser makes the page from the same two, its ids are the same, and it takes the
elements over instead of drawing them again: `server.mjs` and `client.mjs` in the example do exactly this.
Both themes belong in one stylesheet, since a change of theme changes nothing in the tree that doesn't read
`page.theme`.

## Checking it

- Hold it to the schema: every tree you are given fits `tree.schema.json`.
- `schemas/examples/counter.tree.json` is the tree of `schemas/examples/counter.xtmd`, from any engine.
- Walk a page at random (its events, its writes) and compare what you drew with what drawing `page.tree()`
  from nothing gives. The example's test does.
