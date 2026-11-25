// @extended-mark/svelte-live: an Extended Mark document compiled in the browser as it is edited, in a Svelte 5 host.
// The static engine, @extended-mark/svelte, ships no WebAssembly; this one does.

export { default as XtmdLiveDoc } from "./XtmdLiveDoc.svelte";
export { Compiler, compiler, type Builds, type LiveProps, type PackFile, type WasmSource } from "@extended-mark/live";
export type { DocError, Restored, Snapshot } from "@extended-mark/widgets";
export type { DocProps } from "@extended-mark/widgets/doc";
