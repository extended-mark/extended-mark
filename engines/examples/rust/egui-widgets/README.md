# xtmd-egui-widgets

Extended Mark's egui widget set: one widget for every extern widget of core, each a fn from a drawn node to what egui
draws. It knows no page. What draws one ([xtmd-egui](../egui), or an engine's output) keeps a `Drawing` across frames
and is the `Host` the widgets act on: events, checked writes, tokens.

A host replaces a widget by its full name, or adds a pack's:

```rust,no_run
use xtmd_egui_widgets::{Ctx, Props, Registry};

let mut widgets = Registry::ours();
widgets.set("acme/badge", || |ui: &mut egui::Ui, p: &Props<'_>, _: &mut Ctx<'_>| {
    ui.label(p.text("label"));
});
```

An example (engines/README.md): CI builds it and runs its tests, and it isn't published (`publish = false`).
