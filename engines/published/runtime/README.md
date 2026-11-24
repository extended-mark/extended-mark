# @extended-mark/runtime

Extended Mark's runtime for the web: it runs the IR `xtmd build` writes, and gives the host a tree to draw
(docs/specs/06, The host's side). It has no widgets and needs no DOM: [@extended-mark/dom](../dom) draws
with it, and so can a host of its own ([apps/example-host](../../../apps/example-host)).

```ts
import { Page, type Node } from "@extended-mark/runtime";
import ir from "./page.ir.json";

const page = new Page(ir, { supplies: (widget) => widget in mine });  // the rest shows its Markdown
draw(page.tree().nodes);
page.onChange((changes) => follow(changes));      // by node id: inputs, children kept or new, events, writes
page.event(id, "click");                          // what the reader did
page.write(id, "value", { num: "7" });            // checked: done, or refused in page.takeOutcome()
```

| | |
|---|---|
| `tree()`, `node(id)`, `onChange(f)` | The page now, as plain JSON, and what changed. A node nothing changed in is the same object. `apply(tree, changes)` makes the next tree from the last; `everyNode(nodes)` and `nodesIn(input)` walk one. |
| `event(id, name, args?)`, `write(id, input, value)`, `call(id, input, args)` | What the reader and the widgets do. |
| `set(fields)`, `state()`, `data()`, `restore(data)`, `restoreOwn(own)` | `$state`, and everything the page holds. `new Page(ir, { data })` starts from it, drawn once. |
| `expose(channel, name, args?)`, `setExtern(channel, values)`, `setPage(values)`, `setSupplies(f)`, `given()` | What the host gives. |
| `markdown()`, `blocks()`, `token(input)` | The page's Markdown, and what a token stands for. |
| `lacking()`, `missing()`, `takeCalls()`, `takeOutcome()`, `takeWarnings()`, `errors` | What nobody gives yet, and what happened. |
| `layout(page, measure)` | A plane's layout, from each spot's size as the host drew it. |

It runs IR 4 (`IR_VERSION`), and refuses another: `xtmd migrate page.ir.json` brings an earlier one here.
A change works out only what reads what changed: `npm test` counts it.
