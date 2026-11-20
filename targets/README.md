# Targets

A target is what draws a page. A run-time target takes the IR `xtmd build` writes and draws it with
its own widgets; a real-time target compiles the document's text where it runs (with xtmd-live or
@extended-mark/wasm) and draws that. Any target that passes `xtmd target check`, Extended Mark's
conformance cases (checks/cases) run through the adapter its `target.yaml` names, is an Extended Mark
target, wherever it lives and whoever writes it.

- **published/** is what the release publishes, on npm: `@extended-mark/runtime`, `@extended-mark/dom`
  and the bindings (bindings/: embed, view and live, and React, Next, Vue, Svelte, Solid and Angular,
  each with its live twin). bindings/testing is their tests' shared helper, not a package.
- **examples/** shows what a target can be. CI builds and checks every one, and nothing here is ever
  published:
  - rust/: xtmd-runtime, the native runtime (its `native` target draws with no screen), and egui and
    egui-live on it;
  - misc/android: Compose on the native runtime, through UniFFI (`android`, `android-live`);
  - misc/web: @extended-mark/dom in a real Chromium (`web`);
  - misc/test: @extended-mark/runtime with no screen and fixed sizes, the checks' own (`test`).

`xtmd target check` finds every `target.yaml` here and under ide/ (the VS Code preview, `vscode`):
20 targets. `xtmd target check <name>` runs one.
