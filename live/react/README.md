# @extended-mark/react-live

Extended Mark documents in React 19, the live engine: `XtmdLiveDoc` compiles a document's text in the
browser with [@extended-mark/wasm](../../compiler/npm/wasm) as it is edited, and draws it with [@extended-mark/react](../../renderers/react)'s
`XtmdDoc`. A text that compiles replaces the page, keeping `$state` where its fields still exist.
One that doesn't leaves the last page that did, and its errors reach `onError` as
`{ kind: "compile", message, diagnostics }`. When to change `source` (each key, a pause, a save) is
the host's. The static engine ships no WebAssembly; this one does.

```tsx
import { XtmdLiveDoc } from "@extended-mark/react-live";
import wasm from "@extended-mark/wasm/xtmd.wasm?url";            // as the bundler gives the file's URL

<XtmdLiveDoc source={text} wasm={wasm} onError={(e) => e.kind === "compile" && show(e.diagnostics)} />
```

| Prop | What it does |
|---|---|
| `source` | The document's text. |
| `path` | Its path, for diagnostics: `page.xtmd` when left out. |
| `packs` | The files of the packs it uses, `{ path, text }`. |
| `wasm` or `compiler` | Where xtmd.wasm is (its URL, its bytes or the module), or a compiler in hand (one in a worker, which answers with a promise). Given a compiler already loaded, the first render has the page, on a server too. |

Everything else is `XtmdDoc`'s ([@extended-mark/react](../../renderers/react/README.md#props)), the handle included.

`npm test`: a server render with a compiler in hand, then the shared live steps in Chromium
(`../../renderers/testing/browser.mjs`, `liveSteps`).
