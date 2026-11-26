//! # xtmd-egui
//!
//! Extended Mark's desktop target: pages run on the native runtime ([`xtmd_runtime`]) and drawn with egui.
//!
//! ```no_run
//! # fn host(ui: &mut egui::Ui, json: &str) {
//! use xtmd_egui::{XtmdPage, runtime::{Page, PageOptions}};
//! // once: the IR `xtmd compile page.xtmd --out page.ir.json` wrote
//! let mut page = XtmdPage::new(Page::from_json(json, XtmdPage::options()).unwrap());
//! // every frame:
//! page.show(ui);
//! # }
//! ```
//!
//! - [`XtmdPage::new`] takes a [`Page`] and draws it every frame with a widget set
//!   ([`xtmd_egui_widgets`]): one widget per extern widget, by full name (`core/press`). Ours draw
//!   every core extern; a host's own ([`XtmdPage::widget`]) replace ours by name, and add a pack's.
//!   The page is told what is supplied, so an extern nobody draws shows its Markdown instead.
//! - The widgets know no page: this is the [`Host`] they are drawn for.
//! - The plane's layout runs after every frame, from what egui measured, or from fixed sizes
//!   ([`measure::Fixed`]) as the checks do.

pub mod doc;
pub mod measure;

#[cfg(feature = "check")]
pub mod check;

/// A page in a window of its own, with eframe (the `app` feature): scrolled, with a margin. The
/// page's errors go to standard error, and its Markdown to standard output when the window closes.
#[cfg(feature = "app")]
pub fn run_window(title: &str, mut page: XtmdPage) -> eframe::Result {
    let options = eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([720.0, 800.0]).with_title(title), ..Default::default() };
    eframe::run_ui_native(title, options, move |ui, _frame| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            egui::Frame::new().inner_margin(16.0).show(ui, |ui| {
                page.show(ui);
            });
        });
        for e in page.take_errors() {
            eprintln!("{}", e);
        }
        if ui.ctx().input(|i| i.viewport().close_requested()) {
            println!("{}", page.page().markdown().unwrap_or_default());
        }
    })
}

use std::collections::HashSet;
use std::rc::Rc;

use egui::{Id, Ui, UiBuilder};
pub use xtmd_egui_widgets::{a11y, fns, look, rich, values, widgets, Act, Contract, Ctx, Drawing, Host, Maker, Measured, Props, Registry, Space, Widget};
pub use xtmd_runtime as runtime;
use xtmd_planes::{Measure, Planes};
use xtmd_runtime::ir::{self, Expr};
use xtmd_runtime::{Drawn, Page, PageOptions, PageTheme, PageValues, Value};

use crate::measure::FromDrawn;

/// A page as its widgets have it.
struct OnPage<'a>(&'a mut Page);

impl Host for OnPage<'_> {
    fn event(&mut self, id: &str, name: &str, args: Vec<Value>) -> Result<(), String> {
        self.0.event(id, name, args).map_err(|f| f.message)
    }

    fn write(&mut self, id: &str, input: &str, value: Value) -> Result<bool, String> {
        self.0.write(id, input, value.clone()).map_err(|f| f.message)?;
        Ok(match self.0.find(id).and_then(|n| n.props.get(input)) {
            Some(now) => self.0.equal(now, &value).unwrap_or(false),
            None => false,
        })
    }

    fn listens(&self, d: &Drawn, event: &str) -> bool {
        self.0.listens(d, event)
    }

    fn contract(&self, d: &Drawn, input: &str) -> Contract {
        let Some(t) = self.0.write_type(d, input) else { return Contract::default() };
        let n = |x: Option<&Expr>| x.and_then(|x| self.0.value(x).ok()).and_then(|v| values::exact(Some(&v)));
        let c = t.contract.as_ref();
        let range = c.and_then(|c| c.range.as_ref());
        let mut step = n(c.and_then(|c| c.step.as_ref()));
        if step.is_none() && t.name == "core/int" {
            step = Some(runtime::Num::int(1));
        }
        Contract { low: n(range.and_then(|r| r.low.as_ref())), high: n(range.and_then(|r| r.high.as_ref())), step }
    }

    fn apply(&self, f: &Value, args: Vec<Value>) -> Result<Option<Value>, String> {
        self.0.apply(f, args).map_err(|f| f.message)
    }

    fn token(&self, v: &Value) -> Option<Value> {
        self.0.token_value(v).ok()
    }

    fn equal(&self, a: &Value, b: &Value) -> bool {
        self.0.equal(a, b).unwrap_or(false)
    }
}

/// How the plane's layout measures spots.
pub enum Sizes {
    /// From what egui drew: each spot's rect, each plane's width.
    Measured,
    /// From a measure of the host's, such as the checks' fixed sizes.
    Given(Box<dyn Measure>),
}

/// A page drawn with egui.
pub struct XtmdPage {
    page: Page,
    drawing: Drawing,
    tree: Rc<Vec<Drawn>>,
    revision: Option<u64>,
    sizes: Sizes,
    planes: Planes,
    follow_theme: bool,
    id: Id,
    /// The theme last given to the page from egui's.
    pushed: Option<bool>,
    /// Frames in a row whose layout changed the page while nothing outside it did, and the
    /// outside change they counted from.
    streak: (u64, usize),
    /// The outside change a layout that keeps changing is held at.
    held: Option<u64>,
}

impl XtmdPage {
    /// Draws a page with our widgets. Its value fns are egui's ([`fns::egui_fns`]) where the page
    /// was made with [`XtmdPage::options`].
    pub fn new(page: Page) -> XtmdPage {
        let mut p = XtmdPage {
            page,
            drawing: Drawing::new(Registry::ours(), Id::unique("xtmd-page")),
            tree: Rc::new(Vec::new()),
            revision: None,
            sizes: Sizes::Measured,
            planes: Planes::default(),
            follow_theme: true,
            id: Id::unique("xtmd-page"),
            streak: (0, 0),
            held: None,
            pushed: None,
        };
        p.tell_supplies();
        p
    }

    /// Draws another page in this one's place, with the same widgets, measure and id: a new IR for
    /// the same document. Nothing drawn for the last page is kept.
    pub fn replace(&mut self, page: Page) {
        self.page = page;
        self.tree = Rc::new(Vec::new());
        self.revision = None;
        self.pushed = None;
        self.streak = (0, 0);
        self.held = None;
        self.drawing.forget();
        self.tell_supplies();
    }

    /// Page options with egui's value fns: what [`Page::from_json`] wants for a page drawn here.
    pub fn options() -> PageOptions {
        PageOptions { fns: fns::egui_fns(), ..PageOptions::default() }
    }

    /// Replaces our widget for a name, or adds one: a host's own, or a pack's.
    pub fn widget<W: Widget + 'static>(mut self, name: &str, make: impl Fn() -> W + 'static) -> XtmdPage {
        self.drawing.set(name, make);
        self.tell_supplies();
        self
    }

    /// Replaces the whole registry.
    pub fn with_registry(mut self, registry: Registry) -> XtmdPage {
        self.drawing.set_registry(registry);
        self.tell_supplies();
        self
    }

    /// The plane's layout from a measure of the host's (the checks' fixed sizes), not egui's.
    pub fn with_measure(mut self, measure: Box<dyn Measure>) -> XtmdPage {
        self.sizes = Sizes::Given(measure);
        self
    }

    /// Whether the page's theme follows egui's dark mode (the default), or is the host's to set.
    pub fn follow_theme(mut self, follow: bool) -> XtmdPage {
        self.follow_theme = follow;
        self
    }

    /// [`XtmdPage::follow_theme`], on a page already made.
    pub fn set_follow_theme(&mut self, follow: bool) {
        self.follow_theme = follow;
    }

    /// The egui id of the page's own ui, and every widget's in it: two pages in one window each
    /// need one of their own.
    pub fn id(mut self, id: Id) -> XtmdPage {
        self.id = id;
        self.drawing.set_id(id);
        self
    }

    pub fn page(&self) -> &Page {
        &self.page
    }

    pub fn page_mut(&mut self) -> &mut Page {
        &mut self.page
    }

    /// The paths drawn in the last frame, and the ones kept on the page hidden.
    pub fn shown(&self) -> HashSet<String> {
        self.drawing.shown()
    }

    /// Asks the widget at a path to do something through its own controls on the next frame;
    /// one it can't goes through the page after that frame.
    pub fn act(&mut self, path: &str, act: Act) {
        self.drawing.act(path, act);
    }

    /// Whether acts are waiting for a frame.
    pub fn acting(&self) -> bool {
        self.drawing.acting()
    }

    /// What went wrong since the last call: failed handlers, refused drawing.
    pub fn take_errors(&mut self) -> Vec<String> {
        let mut out = self.drawing.take_errors();
        out.extend(self.page.take_errors().into_iter().map(|f| f.message));
        out
    }

    /// Tells the page which extern widgets are drawn here: one that isn't shows its Markdown.
    fn tell_supplies(&mut self) {
        let names: HashSet<String> = self.drawing.registry().names().map(str::to_string).collect();
        if let Err(f) = self.page.set_supplies(Some(Rc::new(move |w: &str| names.contains(w)))) {
            self.drawing.error(f.message);
        }
    }

    /// The page's theme now: `core/page.theme`.
    fn page_dark(&self) -> bool {
        match self.page.eval(&Expr::Get(ir::Get::Extern("core/page.theme".into()))) {
            Ok(v) => values::case(Some(&v)) == Some("DARK"),
            Err(_) => false,
        }
    }

    /// Draws the page, then lays its planes out from what was drawn. Call it every frame.
    pub fn show(&mut self, ui: &mut Ui) -> egui::Response {
        if self.follow_theme {
            let dark = ui.visuals().dark_mode;
            if self.pushed != Some(dark) {
                self.pushed = Some(dark);
                if dark != self.page_dark() {
                    let theme = if dark { PageTheme::Dark } else { PageTheme::Light };
                    if let Err(f) = self.page.set_page(PageValues { width: None, theme: Some(theme) }) {
                        self.drawing.error(f.message);
                    }
                }
            }
        }
        let rev = self.page.revision();
        if self.revision != Some(rev) {
            self.tree = Rc::new(self.page.tree().to_vec());
            self.revision = Some(rev);
            let dark = self.page_dark();
            self.drawing.set_dark(dark);
        }
        let tree = self.tree.clone();
        let (page, drawing) = (&mut self.page, &mut self.drawing);
        let r = ui.scope_builder(UiBuilder::new().scope_id(self.id), |ui| drawing.show(&mut OnPage(page), ui, &tree));
        // what the widgets didn't do themselves goes through the page
        for (path, act) in self.drawing.take_left() {
            let r = match act {
                Act::Event { name, args } => self.page.event(&path, &name, args),
                Act::Write { input, value } => self.page.write(&path, &input, value),
            };
            if let Err(f) = r {
                self.drawing.error(format!("{}: {}", path, f.message));
            }
        }
        // egui lays out every frame: a layout that changes the page frame after frame while
        // nothing outside it does (a spot whose key moves at every redraw, so its own box is
        // never kept) is held until something outside changes the page, or it would never settle
        let outside = self.page.changes();
        if self.held != Some(outside) {
            self.held = None;
            let at = self.page.revision();
            let laid = match &mut self.sizes {
                Sizes::Given(m) => self.planes.lay(&mut self.page, m.as_mut()),
                Sizes::Measured => {
                    let mut m = FromDrawn(self.drawing.measured());
                    self.planes.lay(&mut self.page, &mut m)
                }
            };
            if let Err(f) = laid {
                self.drawing.error(f.message);
            }
            if self.page.revision() == at {
                self.streak = (outside, 0);
            } else {
                self.streak = if self.streak.0 == outside { (outside, self.streak.1 + 1) } else { (outside, 1) };
                if self.streak.1 >= xtmd_runtime::MAX_PASSES {
                    self.held = Some(outside);
                    self.drawing.error(format!("the layout still changed the page after {} frames with nothing else changing it: held until something does", self.streak.1));
                }
            }
        }
        if self.page.revision() != rev {
            ui.ctx().request_repaint();
        }
        r.response
    }
}
