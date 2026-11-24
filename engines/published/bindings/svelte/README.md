# @extended-mark/svelte

Extended Mark documents in Svelte 5, the static engine: `XtmdDoc` draws a page from an IR built ahead
(`xtmd build`, or xtmd's bundler plugin), with no compiler and no WebAssembly in the host. Core's
widgets are drawn by Svelte itself (`svelte:element`, recursively), server-rendered with `svelte/server`. The live engine, which compiles a document's text in the
browser, is [@extended-mark/svelte-live](../svelte-live).

```sh
npm install @extended-mark/svelte
```

```svelte
<script>
  import { XtmdDoc } from "@extended-mark/svelte";
  let state = $state.raw(saved);
  let doc;   // doc.snapshot(), doc.restore(…), doc.set(…), doc.getState()
</script>

<XtmdDoc bind:this={doc} ir={shop} {state} onStateChange={(s) => (state = s)}
  externs={{ shop: { currency: { text: "EUR" } } }} onError={console.warn} onCalls={handle} />
```

## Props

The same in every framework (@extended-mark/view's `DocProps`):

| Prop | What it does |
|---|---|
| `ir` | The page's IR, which the host brings. A new one keeps `$state` where its fields still exist. |
| `state` and `onStateChange`, or `defaultState` | `$state` controlled by the host, or started from. A new `state` is a write checked by xtmd; one it refuses leaves the page as it was. |
| `defaultSnapshot` | A snapshot to start from: `$state` and every widget's own variables. |
| `onError` | `{ kind: "refused" }` (the rule, the code, the widget), `"failed"`, `"load"` (an IR that couldn't be had). |
| `onCalls` | The host's extern fns the page called. |
| `externs`, `fns` | Each channel's values, by its name in the document or in full; the host's value fns, by full name. |
| `page`, `widgets`, `measure` | The page's width and theme; a pack's own extern widgets, drawn the way core's are; how the plane's spots are measured. |
| `components` | Svelte components as widgets, by full name. Each is given `node`, `event`, `write` and `call`, and the snippet `slot`: `{@render slot("body")}` is an input's widgets. |
| `failed` | What is drawn where a widget failed, in place of our notice. |

The components ship as source, for the host's Svelte compiler. The handle is `bind:this`: `document()`, `set`, `getState` (`state` is a rune), `markdown`, `expose`, `snapshot`, `restore`. A snapshot is opaque JSON: with the same IR, `restore` puts all of
it back; with another, `$state` only.

## Real time

```svelte
<XtmdLiveDoc source={text} wasm={wasmUrl} onError={(e) => e.kind === "compile" && show(e.diagnostics)} />
<!-- import { XtmdLiveDoc } from "@extended-mark/svelte-live" -->
```

## Tests

`npm test`: `XtmdDoc` rendered on a server, then the shared steps in Chromium (`../testing/browser.mjs`,
`docSteps`): controlled `$state`, refusals, the handle, props, a new IR, widgets' own variables and
snapshots.
