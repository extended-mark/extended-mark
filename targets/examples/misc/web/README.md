# Extended Mark's web target

@extended-mark/runtime with @extended-mark/dom's widgets in a real Chromium (`XTMD_CHROMIUM`, or `/usr/bin/chromium`;
none is downloaded), driven by Extended Mark's checks: `xtmd target check web` runs every case in
`checks/cases/` through `check_adapter.mjs` (`target.yaml`'s `check_adapter:`), which loads each page
with `harness.mjs`. Another target drawn in the browser (the VS Code extension's preview) reuses the
adapter with its own entry: `--harness <file>` and `--name <target>`.
