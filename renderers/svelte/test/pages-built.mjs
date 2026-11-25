import v1 from "./built/v1.svelte";
import v2 from "./built/v2.svelte";
import notes from "./built/notes.svelte";

export const page = (which) => ({ of: { v1, v2, notes }[which], props: {} });
