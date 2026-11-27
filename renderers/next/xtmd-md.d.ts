// `import page from "./sprint.xtmd"`: the page's IR. Reference it from a .d.ts in the host:
// /// <reference types="@extended-mark/next/xtmd-md" />
declare module "*.xtmd" {
  const page: import("./dist/index.js").ir.File;
  export default page;
}
