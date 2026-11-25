import { V1 } from "./built/v1.js";
import { V2 } from "./built/v2.js";
import { Notes } from "./built/notes.js";

export const imports = [V1, V2, Notes];
export const template = (given: string) => `@switch (which()) { ${["v1", "v2", "notes"].map((p) => `@case ("${p}") { <xtmd-${p} ${given} /> }`).join(" ")} }`;
