import { fileURLToPath } from "node:url";

// the repository: build-ts.mjs makes React one copy for the renderers, a link out of this package
const root = fileURLToPath(new URL("../../../..", import.meta.url));
export default { turbopack: { root }, outputFileTracingRoot: root };
