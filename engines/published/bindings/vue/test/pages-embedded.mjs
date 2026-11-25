import { XtmdDoc } from "@extended-mark/vue";

export const page = (which) => ({ of: XtmdDoc, props: { ir: window.__IR[which] } });
