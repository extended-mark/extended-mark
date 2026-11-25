// The component ships as source, for the host's Svelte compiler, with its types beside it.
import { copyFileSync, mkdirSync } from "node:fs";
mkdirSync(new URL("../dist/", import.meta.url), { recursive: true });
for (const f of ["XtmdDoc.svelte", "XtmdDoc.svelte.d.ts", "XtmdPage.svelte", "XtmdPage.svelte.d.ts", "XtmdNode.svelte", "XtmdNode.svelte.d.ts"]) copyFileSync(new URL(`../src/${f}`, import.meta.url), new URL(`../dist/${f}`, import.meta.url));
