# xtmd-egui-live

Extended Mark's desktop engine, real time: a document's text compiled in the host as it is edited, by
[xtmd](../../compiler/xtmd)'s compiler (core compiled once for the host), and drawn by
[xtmd-egui](../../renderers/egui)'s `XtmdDoc`. The static engine, xtmd-egui, carries no compiler; a host that
only draws pages it was given uses that one.

CI builds it and runs its tests, and it isn't published
(`publish = false`); a host depends on it by path or by git.

```rust,no_run
use xtmd_egui::doc::DocOptions;
use xtmd_egui_live::XtmdLiveDoc;

fn host(text: &str) -> XtmdLiveDoc {
    XtmdLiveDoc::new(text, DocOptions::default())
}

fn frame(doc: &mut XtmdLiveDoc, editor_text: &str, ui: &mut egui::Ui) {
    // when to compile is the host's: each key, a pause, a save
    doc.set_text(editor_text);
    let r = doc.show(ui);
    for e in r.errors {
        eprintln!("{}", e); // a draft that doesn't compile is DocError::Compile, with its first error and where
    }
}
```

- A text that compiles replaces the page, keeping `$state` where its fields still exist and fit.
- One that doesn't leaves the last page that did. `diagnostics()` has every diagnostic of the
  latest text, and `why()` gives the first error as one line.
- Before any text has compiled, `show` draws the reason.
- `XtmdLiveDoc::with(compiler, path, packs, text, options)` gives the path, the packs' files as text,
  and a compiler of the host's; `compiler()` is the host's own, made the first time it is asked for.
- Everything else is the `XtmdDoc` it draws: `doc()`, `doc_mut()` (`set`, `snapshot`, `restore` …).

## Its tests

`cargo test -p xtmd-egui-live` runs `tests/live.rs`.
