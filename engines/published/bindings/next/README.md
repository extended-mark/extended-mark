# @extended-mark/next

Extended Mark documents in Next.js's host router: `<XtmdDoc>` (@extended-mark/react's, run time) as a client component,
which Next renders on the server and hydrates; `.xtmd` imports compiled by xtmd at build time, as
`withXtmd` sets up; and a page's Markdown as HTML. An editor in a Next host uses
[@extended-mark/react-live](../react-live)'s `<XtmdLiveDoc>` for real time.

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
