// A document's text, as esbuild gives it (build.mjs).
declare module "*.xtmd" {
  const text: string;
  export default text;
}
