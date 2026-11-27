import v1 from "./built/v1.js";
import v2 from "./built/v2.js";
import notes from "./built/notes.js";

export const page = (which) => ({ of: { v1, v2, notes }[which], props: {} });
