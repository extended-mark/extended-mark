//! The widget set copied as a starting point: its sources compiled here, as a host's own, against what
//! xtmd-egui-widgets gives anyone, and one of them replaced. It passes what ours does.

// what the copied sources take from their crate, which is this one now
pub use xtmd_egui_widgets::{a11y, look, rich, values, Act, Contract, Ctx, Props, Registry, Space, Widget, PAGE_WIDTH};

#[path = "../../../widgets/egui/src/widgets/mod.rs"]
#[allow(dead_code)]
mod widgets;

#[path = "widgets.rs"]
mod suite;

use std::cell::Cell;

thread_local! {
    static DRAWN: Cell<usize> = const { Cell::new(0) };
}

/// The copy, its label the host's own: it draws as the copy's does, and counts that it did.
pub fn set() -> Registry {
    let mut r = widgets::defaults();
    r.set("core/label", || {
        |ui: &mut egui::Ui, p: &Props<'_>, ctx: &mut Ctx<'_>| {
            DRAWN.with(|n| n.set(n.get() + 1));
            widgets::controls::label(ui, p, ctx);
        }
    });
    r
}

#[test]
fn the_copy_is_what_draws() {
    let page = "@label(value = \"Hi\");\n";
    let before = DRAWN.with(Cell::get);
    let h = suite::common::mount(page);
    drop(h);
    assert!(DRAWN.with(Cell::get) > before);
}
