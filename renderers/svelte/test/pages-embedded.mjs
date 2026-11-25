import { XtmdDoc } from "@extended-mark/svelte";

export const page = (which) => ({ of: XtmdDoc, props: { ir: window.__IR[which] } });
