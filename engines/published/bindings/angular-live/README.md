# @extended-mark/angular-live

Extended Mark documents in Angular, the live engine: `XtmdLiveDoc` compiles a document's text in the
browser with [@extended-mark/wasm](../../../../compiler/npm/wasm) as it is edited, and draws it with [@extended-mark/angular](../angular)'s
`XtmdDoc`. A text that compiles replaces the page, keeping `$state` where its fields still exist.
One that doesn't leaves the last page that did, and its errors reach `onError` as
`{ kind: "compile", message, diagnostics }`. When to change `source` (each key, a pause, a save) is
the host's. The static engine ships no WebAssembly; this one does.

```html
<xtmd-live-doc [source]="text" [wasm]="wasmUrl" (error)="show($event)" />
<!-- import { XtmdLiveDoc } from "@extended-mark/angular-live" -->
```

| Prop | What it does |
|---|---|
| `source` | The document's text. |
| `path` | Its path, for diagnostics: `page.xtmd` when left out. |
| `packs` | The files of the packs it uses, `{ path, text }`. |
| `wasm` or `compiler` | Where xtmd.wasm is (its URL, its bytes or the module), or a compiler in hand (one in a worker, which answers with a promise). Given a compiler already loaded, the first render has the page, on a server too. |

Everything else is `XtmdDoc`'s ([@extended-mark/angular](../angular/README.md#props)), the handle included.

`npm test`: a server render with a compiler in hand, then the shared live steps in Chromium
(`../testing/browser.mjs`, `liveSteps`).
