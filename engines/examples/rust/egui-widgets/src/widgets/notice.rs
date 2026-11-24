//! Notices: a widget that failed, and one nobody supplies. Each says so
//! where the widget would be, as an alert.

use egui::accesskit::Role;
use egui::{Ui, UiBuilder};

use super::kit::{self, Lay};
use crate::{a11y, Ctx, Props};

/// A notice with a message, in the `BAD` colour, framed.
pub fn show(ui: &mut Ui, ctx: &mut Ctx<'_>, message: &str) {
    let bad = ctx.core_color("BAD");
    let font = ctx.font("BODY");
    let msg = message.to_string();
    let label = msg.clone();
    ctx.root(ui, UiBuilder::new().accessibility_role(Role::Alert), |ui, ctx| {
        a11y::edit(ui, ui.unique_id(), |n| n.set_label(label));
        egui::Frame::new().stroke(egui::Stroke::new(1.0, bad)).corner_radius(4.0).inner_margin(6.0).show(ui, |ui| {
            kit::leaf(ui, ctx.sub_id("message"), kit::plain(&msg, kit::font_id(&font), bad), &msg, Lay::default());
        });
    });
}

/// A widget that failed: which, and why.
pub fn failed(ui: &mut Ui, props: &Props<'_>, ctx: &mut Ctx<'_>) {
    let d = props.drawn();
    let m = format!("{} failed: {}", a11y::short(&d.widget), d.failed.as_deref().unwrap_or(""));
    show(ui, ctx, &m);
}
