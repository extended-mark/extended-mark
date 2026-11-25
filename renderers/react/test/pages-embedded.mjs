import { XtmdDoc } from "@extended-mark/react";

export const page = (which) => ({ of: XtmdDoc, props: { ir: window.__IR[which] } });
