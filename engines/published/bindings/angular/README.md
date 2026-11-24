# @extended-mark/angular

Extended Mark documents in Angular, the static engine: `XtmdDoc` draws a page from an IR built ahead
(`xtmd build`, or xtmd's bundler plugin), with no compiler and no WebAssembly in the host. Core's
widgets are drawn by Angular itself, through `Renderer2`, so it renders with `@angular/platform-server` and runs zoneless. The live engine, which compiles a document's text in the
browser, is [@extended-mark/angular-live](../angular-live).

```sh
npm install @extended-mark/angular
```

```ts
import { Component, ViewChild, signal } from "@angular/core";
import { XtmdDoc } from "@extended-mark/angular";

@Component({
  standalone: true,
  imports: [XtmdDoc],
  template: `<xtmd-doc #doc src="/pages/shop.ir.json" [state]="state()" (stateChange)="state.set($event)"
    [externs]="{ shop: { currency: { text: 'EUR' } } }" (error)="warn($event)" (calls)="handle($event)" />`,
})
class Shop {
  state = signal(saved);
  @ViewChild("doc") doc!: XtmdDoc;   // doc.snapshot(), doc.restore(…), doc.set(…), doc.getState()
}
```

## Props

The same in every framework (@extended-mark/view's `DocProps`):

| Prop | What it does |
|---|---|
| `src` or `ir` | Where to fetch the IR from, or the IR in hand. A new one keeps `$state` where its fields still exist. |
| `state` and `onStateChange`, or `defaultState` | `$state` controlled by the host, or started from. A new `state` is a write checked by xtmd; one it refuses leaves the page as it was. |
| `defaultSnapshot` | A snapshot to start from: `$state` and every widget's own variables. |
| `onError` | `{ kind: "refused" }` (the rule, the code, the widget), `"failed"`, `"load"` (an IR that couldn't be had). |
| `onCalls` | The host's extern fns the page called. |
| `externs`, `fns` | Each channel's values, by its name in the document or in full; the host's value fns, by full name. |
| `page`, `widgets`, `measure` | The page's width and theme; a pack's own extern widgets, drawn the way core's are; how the plane's spots are measured. |
| `components` | Angular components as widgets, by full name. Each is given the inputs it declares of `node`, `event`, `write` and `call`, and an input's widgets where `<ng-content select="[data-xtmd-slot=body]" />` is. |
| `failed` | What is drawn where a widget failed, in place of our notice. |

Outputs are `(stateChange)`, `(error)` and `(calls)`. The handle is the component: `document`, `set`, `getState`, `markdown`, `expose`, `snapshot`, `restore`. A snapshot is opaque JSON: with the same IR, `restore` puts all of
it back; with another, `$state` only.

## Real time

```html
<xtmd-live-doc [source]="text" [wasm]="wasmUrl" (error)="show($event)" />
<!-- import { XtmdLiveDoc } from "@extended-mark/angular-live" -->
```

## Tests

`npm test`: `XtmdDoc` rendered on a server, then the shared steps in Chromium (`../testing/browser.mjs`,
`docSteps`): controlled `$state`, refusals, the handle, props, a new IR, widgets' own variables and
snapshots.
