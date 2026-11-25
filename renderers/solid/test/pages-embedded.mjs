import { XtmdDoc } from "@extended-mark/solid";

export const page = (which) => ({ of: XtmdDoc, props: { ir: window.__IR[which] } });
