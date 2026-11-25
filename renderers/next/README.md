# @extended-mark/next

Extended Mark documents in Next.js's host router: `<XtmdDoc>` (@extended-mark/react's, run time) as a client component,
which Next renders on the server and hydrates; `.xtmd` imports compiled by xtmd at build time, as
`withXtmd` sets up; and a page's Markdown as HTML. An editor in a Next host uses
[@extended-mark/react-live](../../live/react)'s `<XtmdLiveDoc>` for real time.

```sh
npm install @extended-mark/next
```

```js
// next.config.mjs: one loader, for Turbopack (Next 16's default) and webpack (--webpack)
import { withXtmd } from "@extended-mark/next/config";
export default withXtmd({}, { packs: ["./packs/maps"] });
```

```tsx
// app/shop.tsx: "use client". Next renders the page on the server, then React hydrates it
import { XtmdDoc } from "@extended-mark/next";
import shop from "./shop.xtmd";

export function Shop() {
  return <XtmdDoc ir={shop} onCalls={(calls) => handle(calls)} />;
}
```

Add `/// <reference types="@extended-mark/next/xtmd-md" />` to a `.d.ts` in the host to type the imports. A
page outside the host's folder needs `turbopack.root` set to a folder that holds both.

## Exports

- `@extended-mark/next`: `XtmdDoc`, as in [@extended-mark/react](../react/README.md), with `"use client"`.
- `@extended-mark/next/config`: `withXtmd(config, { packs, config })`, and `loader`, the loader's path.
- `@extended-mark/next/server`: `pageHtml(ir, { state, page, externs })`, the page's Markdown as HTML, with no DOM.

## Pages built ahead

With [@extended-mark/web](../../engines/web) beside it, this package has an engine, `xtmd-next`: a document known when the app is
built becomes a React component, with no IR, no runtime and no package of ours in it.

```toml
# xtmd.toml
[engines]
next = ["xtmd-next"]
```

```sh
npm install -D @extended-mark/next @extended-mark/web
xtmd build docs --out src/xtmd --engine next
```

```tsx
import Shop from "../xtmd/shop.js";      // in a server component too: it says it is the client's

export default function Page() {
  return <Shop defaultState={saved} />;
}
```

A component has `XtmdDoc`'s props and handle, without `ir`, `widgets`, `measure` and `markdown`. Its snapshots and `XtmdDoc`'s of
the same IR go into each other. Options after the engine's name: `--widgets FILE`, `--only-widgets`, `--tokens FILE`,
`--fns FILE`, `--components a/b,c/d` (the widgets you give React components for) and `--readable`.
