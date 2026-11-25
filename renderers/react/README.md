# @extended-mark/react

Extended Mark documents in React 19: the renderer. `XtmdDoc` draws a page from an IR built ahead
(`xtmd build`, or xtmd's bundler plugin), with no compiler and no WebAssembly in the host. Core's
widgets are drawn by React itself, with `createElement`: the page is part of the host's tree, server-rendered with `renderToString` and hydrated like the rest. The live document, which compiles a document's text in the
browser, is [@extended-mark/react-live](../../live/react).

```sh
npm install @extended-mark/react
```

```tsx
import { XtmdDoc, type XtmdDocHandle } from "@extended-mark/react";
import { useRef, useState } from "react";

export function Shop() {
  const [state, setState] = useState(saved);            // the host's $state, or leave it to the page
  const doc = useRef<XtmdDocHandle>(null);
  return (
    <>
      <XtmdDoc ref={doc} ir={shop} state={state} onStateChange={setState}
        externs={{ shop: { currency: { text: "EUR" } } }} onError={(e) => console.warn(e)} onCalls={handle} />
      <button onClick={() => save(doc.current!.snapshot())}>Save</button>
    </>
  );
}
```

## Props

The same in every framework (`DocProps`, in @extended-mark/widgets/doc):

| Prop | What it does |
|---|---|
| `ir` | The page's IR, which the host brings. A new one keeps `$state` where its fields still exist. |
| `state` and `onStateChange`, or `defaultState` | `$state` controlled by the host, or started from. A new `state` is a write checked by xtmd; one it refuses leaves the page as it was. |
| `defaultSnapshot` | A snapshot to start from: `$state` and every widget's own variables. |
| `onError` | `{ kind: "refused" }` (the rule, the code, the widget), `"failed"`, `"load"` (an IR that couldn't be had). |
| `onCalls` | The host's extern fns the page called. |
| `externs`, `fns` | Each channel's values, by its name in the document or in full; the host's value fns, by full name. |
| `page`, `widgets`, `measure` | The page's width and theme; a pack's own extern widgets, drawn the way core's are; how the plane's spots are measured. |
| `components` | React components as widgets, by full name. Each is given `node` (its inputs, events and writes: docs/specs/06, The host's side), `event`, `write` and `call`, and `slot("body")`, an input's widgets. |
| `failed` | What is drawn where a widget failed, in place of our notice. |

`id`, `className`, `style` go on the root. The handle is `ref`: `{ doc, set, state, markdown, expose, snapshot, restore }` (`XtmdDocHandle`). A snapshot is opaque JSON: with the same IR, `restore` puts all of
it back; with another, `$state` only.

## Your own components as widgets

```tsx
import type { ReactWidgetProps } from "@extended-mark/react";

function Chart({ node, event, slot }: ReactWidgetProps) {
  const { title, points } = node.inputs;                  // each input by its name, as JSON in the IR's form
  return <figure onClick={() => event("pick", [{ num: "0" }])}>{slot("legend")}</figure>;
}

<XtmdDoc ir={report} components={{ "acme/chart": Chart }} />
```

A component draws a widget as our own do, through the host's contract and nothing else. One given a
core widget's name (`core/press`) takes its place.

## Real time

```tsx
import { XtmdLiveDoc } from "@extended-mark/react-live";
import wasm from "@extended-mark/wasm/xtmd.wasm?url";            // as the bundler gives the file's URL

<XtmdLiveDoc source={text} wasm={wasm} onError={(e) => e.kind === "compile" && show(e.diagnostics)} />
```

## Pages built ahead

With [@extended-mark/web](../../engines/web) beside it, this package has an engine, `xtmd-react`: a document known when the app is
built becomes a React component, with no IR, no runtime and no package of ours in it.

```toml
# xtmd.toml
[engines]
react = ["xtmd-react"]
```

```sh
npm install -D @extended-mark/react @extended-mark/web
xtmd build docs --out src/xtmd --engine react
```

```tsx
import Shop from "./xtmd/shop.js";

<Shop state={state} onStateChange={setState} onError={(e) => console.warn(e)} />
```

A component has `XtmdDoc`'s props and handle, without `ir`, `widgets`, `measure` and `markdown`. Its snapshots and `XtmdDoc`'s of
the same IR go into each other. Options after the engine's name: `--widgets FILE`, `--only-widgets`, `--tokens FILE`,
`--fns FILE`, `--components a/b,c/d` (the widgets you give React components for) and `--readable`.

## Tests

`npm test`: `XtmdDoc` rendered on a server, then the shared steps in Chromium (`../testing/browser.mjs`,
`docSteps`): controlled `$state`, refusals, the handle, props, a new IR, widgets' own variables and
snapshots.
