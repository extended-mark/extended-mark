import { fileURLToPath } from "node:url";

// the package these are installed in
export default { turbopack: { root: fileURLToPath(new URL("../..", import.meta.url)) }, outputFileTracingRoot: fileURLToPath(new URL("../..", import.meta.url)) };
