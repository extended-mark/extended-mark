# @extended-mark/solid

Extended Mark documents in Solid: the renderer. `XtmdDoc` draws a page from an IR built ahead
(`xtmd compile`, or xtmd's bundler plugin), with no compiler and no WebAssembly in the host. Core's
widgets are drawn by Solid itself, with fine-grained updates (`Dynamic`, keyed by `mapArray`), server-rendered with `renderToString`; written without JSX, so it needs no Solid compiler. The live document, which compiles a document's text in the
browser, is [@extended-mark/solid-live](../../live/solid).

```sh
npm install @extended-mark/solid
```

```tsx
import { XtmdDoc, type XtmdDocHandle } from "@extended-mark/solid";
import { createSignal } from "solid-js";

const [state, setState] = createSignal(saved);
let doc: XtmdDocHandle;
<XtmdDoc ref={(h) => (doc = h)} ir={shop} state={state()} onStateChange={setState}
  externs={{ shop: { currency: { text: "EUR" } } }} onError={console.warn} onCalls={handle} />
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
| `components` | Solid components as widgets, by full name. Each is given `node` (followed as it changes), `event`, `write` and `call`, and `slot("body")`, an input's widgets. |
| `failed` | What is drawn where a widget failed, in place of our notice. |

`id` and `class` go on the root. The handle is `ref`: `{ doc, set, state, markdown, expose, snapshot, restore }` (`XtmdDocHandle`). A snapshot is opaque JSON: with the same IR, `restore` puts all of
it back; with another, `$state` only.

## Real time

```tsx
import { XtmdLiveDoc } from "@extended-mark/solid-live";
<XtmdLiveDoc source={text()} wasm={wasmUrl} onError={(e) => e.kind === "compile" && show(e.diagnostics)} />
```

## Pages built ahead

With [@extended-mark/web](../../engines/web) beside it, this package has an engine, `xtmd-solid`: a document known when the app is
built becomes a Solid component, with no IR, no runtime and no package of ours in it.

```toml
# xtmd.toml
[engines]
solid = ["xtmd-solid"]
```

```sh
npm install -D @extended-mark/solid @extended-mark/web
xtmd export docs --out src/xtmd --engine solid
```

```tsx
import Shop from "./xtmd/shop.js";

<Shop state={state()} onStateChange={setState} />
```

A component has `XtmdDoc`'s props and handle, without `ir`, `widgets`, `measure` and `markdown`. Its snapshots and `XtmdDoc`'s of
the same IR go into each other. Options after the engine's name: `--widgets FILE`, `--only-widgets`, `--tokens FILE`,
`--fns FILE`, `--components a/b,c/d` (the widgets you give Solid components for) and `--readable`.

## Tests

`npm test`: `XtmdDoc` rendered on a server, then the shared steps in Chromium (`../testing/browser.mjs`,
`docSteps`): controlled `$state`, refusals, the handle, props, a new IR, widgets' own variables and
snapshots.
