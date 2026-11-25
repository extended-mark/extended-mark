# Engines

An engine is what draws a page. A static engine takes the IR `xtmd build` writes and draws it with
its own widgets; a live engine compiles the document's text where it runs (with the crate xtmd or
@extended-mark/wasm) and draws that.

- **published/** is what the release publishes, on npm: `@extended-mark/runtime`, `@extended-mark/widgets` (the
  web's widget set), `@extended-mark/dom` (its plain renderer), `@extended-mark/web` (the engine that builds pages ahead) and the bindings (bindings/: embed, view and live, and React, Next, Vue, Svelte, Solid and Angular,
  each with its live twin). bindings/testing is their tests' shared helper, not a package.
- **examples/** shows what an engine can be. Nothing here is ever published:
  - rust/: xtmd-runtime, the native runtime, and egui and
    egui-live on it;
  - misc/android: Compose on the native runtime, through UniFFI (`android`, `android-live`).

