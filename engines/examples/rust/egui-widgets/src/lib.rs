//! # xtmd-egui-widgets
//!
//! egui's widget set: one [`Widget`] for every extern widget of core, by full name (`core/press`), in a
//! [`Registry`]. A host's own replace ours by name, or add a pack's. What draws a page (xtmd-egui, or an
//! engine's output) keeps a [`Drawing`] across frames, and gives it the page's nodes and itself as a
//! [`Host`] every frame: a widget knows no page.
//!
//! - A widget is drawn every frame, as egui draws: it reads the inputs already worked out ([`Props`]),
//!   and acts through its [`Ctx`]: events, checked writes to its `mut` inputs, its slots' widgets, and
//!   the tokens.
//! - Every widget is in egui's AccessKit tree with its role, name and state, and each of its parts
//!   carries an author id ([`a11y`]), which is how the tests find them.

pub mod a11y;
pub mod fns;
pub mod look;
pub mod rich;
pub mod values;
pub mod widgets;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use egui::{Id, InnerResponse, Ui, UiBuilder};
use xtmd_runtime::{Drawn, Num, Value};

use crate::look::Look;

/// How wide a plane given no width is, where the ui it is in has no end.
pub const PAGE_WIDTH: i64 = 640;

/// What a variable's type says of the numbers it holds: `int 0..10` is 0, 10 and 1.
#[derive(Clone, Copy, Debug, Default)]
pub struct Contract {
    pub low: Option<Num>,
    pub high: Option<Num>,
    pub step: Option<Num>,
}

/// Whatever draws a page with these widgets: the page behind them. `id` is the widget's, as its node says.
pub trait Host {
    /// The reader's event on a widget: runs the caller's handler, as one change.
    fn event(&mut self, id: &str, name: &str, args: Vec<Value>) -> Result<(), String>;
    /// A widget writing one of its `mut` inputs: checked, then one change. Whether it took.
    fn write(&mut self, id: &str, input: &str, value: Value) -> Result<bool, String>;
    /// Whether an event does something.
    fn listens(&self, d: &Drawn, event: &str) -> bool;
    /// What the type of the variable a `mut` input writes says of its numbers.
    fn contract(&self, d: &Drawn, input: &str) -> Contract;
    /// A fn an input holds, called: what it returns, if anything.
    fn apply(&self, f: &Value, args: Vec<Value>) -> Result<Option<Value>, String>;
    /// What a token case stands for in this build.
    fn token(&self, v: &Value) -> Option<Value>;
    /// Whether two values are equal, as `==` says.
    fn equal(&self, a: &Value, b: &Value) -> bool;
}

/// What the widgets of a frame measured, by path: each spot's size and each plane's width, in points.
#[derive(Clone, Debug, Default)]
pub struct Measured {
    pub spots: HashMap<String, egui::Vec2>,
    pub planes: HashMap<String, f32>,
}

/// What an egui widget implements: it is drawn every frame, as egui draws, and reads the props
/// the runtime worked out once per change.
pub trait Widget {
    fn show(&mut self, ui: &mut Ui, props: &Props<'_>, ctx: &mut Ctx<'_>);
}

impl<F: FnMut(&mut Ui, &Props<'_>, &mut Ctx<'_>)> Widget for F {
    fn show(&mut self, ui: &mut Ui, props: &Props<'_>, ctx: &mut Ctx<'_>) {
        self(ui, props, ctx)
    }
}

/// How a widget is made: one instance per drawn widget, kept while it is on the page, so a
/// widget may hold state of its own in `self` (what is being typed, a drag in progress).
pub type Maker = Rc<dyn Fn() -> Box<dyn Widget>>;

/// The widgets a page is drawn with, by full name: `core/press`, `acme/chart`; and what is drawn
/// where a widget failed.
#[derive(Clone, Default)]
pub struct Registry {
    makers: HashMap<String, Maker>,
    failed: Option<Maker>,
}

impl Registry {
    /// No widgets at all.
    pub fn empty() -> Registry {
        Registry::default()
    }

    /// Ours: one for every core extern widget, and for one that failed.
    pub fn ours() -> Registry {
        widgets::defaults()
    }

    /// Sets the widget for a name: a host's replaces ours.
    pub fn set<W: Widget + 'static>(&mut self, name: &str, make: impl Fn() -> W + 'static) -> &mut Registry {
        self.makers.insert(name.to_string(), Rc::new(move || Box::new(make()) as Box<dyn Widget>));
        self
    }

    /// Sets what is drawn where a widget failed: its drawn node says which and why.
    pub fn set_failed<W: Widget + 'static>(&mut self, make: impl Fn() -> W + 'static) -> &mut Registry {
        self.failed = Some(Rc::new(move || Box::new(make()) as Box<dyn Widget>));
        self
    }

    pub fn has(&self, name: &str) -> bool {
        self.makers.contains_key(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.makers.keys().map(String::as_str)
    }

    fn make(&self, name: &str, failed: bool) -> Box<dyn Widget> {
        if failed {
            return self.failed.as_ref().map_or_else(|| Box::new(widgets::notice::failed) as Box<dyn Widget>, |m| m());
        }
        if let Some(m) = self.makers.get(name) {
            return m();
        }
        let missing = name.to_string();
        Box::new(move |ui: &mut Ui, _: &Props<'_>, ctx: &mut Ctx<'_>| widgets::notice::show(ui, ctx, &format!("Nothing draws {} here: its widget isn't supplied.", missing)))
    }
}

/// A drawn widget's props, as the runtime worked them out.
#[derive(Clone, Copy)]
pub struct Props<'a> {
    d: &'a Drawn,
}

impl<'a> Props<'a> {
    pub fn new(d: &'a Drawn) -> Props<'a> {
        Props { d }
    }

    /// The widget as the page drew it: its path, key, props and slots.
    pub fn drawn(&self) -> &'a Drawn {
        self.d
    }

    pub fn get(&self, name: &str) -> Option<&'a Value> {
        self.d.props.get(name)
    }

    /// A text, a number in its exact form, or a bool, as words.
    pub fn text(&self, name: &str) -> String {
        values::text(self.get(name))
    }

    pub fn flag(&self, name: &str, otherwise: bool) -> bool {
        values::flag(self.get(name), otherwise)
    }

    pub fn num(&self, name: &str) -> Option<f64> {
        values::num(self.get(name))
    }

    pub fn exact(&self, name: &str) -> Option<Num> {
        values::exact(self.get(name))
    }

    /// An enum's or a token's case name.
    pub fn case(&self, name: &str) -> Option<&'a str> {
        values::case(self.get(name))
    }

    pub fn point(&self, name: &str) -> Option<(f64, f64)> {
        values::point(self.get(name))
    }

    pub fn list(&self, name: &str) -> Vec<&'a Value> {
        values::list(self.get(name))
    }

    /// The widgets an input holds, a rich_text's or a view's, in order.
    pub fn nodes(&self, name: &str) -> &'a [Drawn] {
        self.get(name).and_then(|v| self.d.nodes(v)).unwrap_or(&[])
    }

    /// A slot's widgets, in order.
    pub fn slot(&self, name: &str) -> &'a [Drawn] {
        self.d.slots.get(name).map_or(&[], Vec::as_slice)
    }
}

/// What a reader does to a widget, in the checks' terms: an event, or a write to a `mut` input.
/// [`Drawing::act`] asks the widget to do it through its own controls on its next frame, as a
/// person would; one the widget doesn't take goes through the page.
#[derive(Clone, Debug)]
pub enum Act {
    Event { name: String, args: Vec<Value> },
    Write { input: String, value: Value },
}

/// A canvas's or a plane's coordinates, for the shapes drawn in it: its units onto the ui's
/// points, and back.
#[derive(Clone, Copy, Debug)]
pub struct Space {
    /// The drawing's rect, in the ui's points.
    pub rect: egui::Rect,
    /// The units at the rect's left and right, and at its bottom and top.
    pub x: (f64, f64),
    pub y: (f64, f64),
    /// Points per unit along x and y; y points down on the screen, up in the units.
    pub scale: (f64, f64),
    /// Whether points outside `rect` are kept inside it (a canvas) or not (a plane).
    pub bounded: bool,
    /// How many units one point on the screen is now: rounding a write keeps a sub-pixel.
    pub units_per_px: f64,
}

impl Space {
    /// A canvas: x and y ranges spread over a rect, y up.
    pub fn canvas(rect: egui::Rect, x: (f64, f64), y: (f64, f64)) -> Space {
        let sx = rect.width() as f64 / (x.1 - x.0).max(f64::MIN_POSITIVE);
        let sy = rect.height() as f64 / (y.1 - y.0).max(f64::MIN_POSITIVE);
        Space { rect, x, y, scale: (sx, sy), bounded: true, units_per_px: 1.0 / sx }
    }

    /// A unit point on the screen.
    pub fn to_screen(&self, p: (f64, f64)) -> egui::Pos2 {
        egui::pos2(self.rect.left() + ((p.0 - self.x.0) * self.scale.0) as f32, self.rect.bottom() - ((p.1 - self.y.0) * self.scale.1) as f32)
    }

    /// A screen point in units.
    pub fn to_units(&self, p: egui::Pos2) -> (f64, f64) {
        (self.x.0 + (p.x - self.rect.left()) as f64 / self.scale.0, self.y.0 + (self.rect.bottom() - p.y) as f64 / self.scale.1)
    }

    /// A length along x, in points.
    pub fn len(&self, u: f64) -> f32 {
        (u * self.scale.0) as f32
    }

    /// A point kept inside what the space shows.
    pub fn keep(&self, p: (f64, f64)) -> (f64, f64) {
        if !self.bounded {
            return p;
        }
        (p.0.clamp(self.x.0.min(self.x.1), self.x.0.max(self.x.1)), p.1.clamp(self.y.0.min(self.y.1), self.y.0.max(self.y.1)))
    }
}

/// What lives across frames and is lent to every widget while a frame is drawn.
pub struct Drawing {
    registry: Registry,
    instances: HashMap<(String, String), Box<dyn Widget>>,
    used: HashSet<(String, String)>,
    /// The paths drawn this frame, or kept on the page hidden (a closed overlay's body).
    shown: HashSet<String>,
    /// The words and marks of the sentences drawn this frame, which their sentence draws.
    words: Vec<Drawn>,
    acts: HashMap<String, Vec<Act>>,
    left: Vec<(String, Act)>,
    look: Look,
    measured: Measured,
    spaces: Vec<Space>,
    errors: Vec<String>,
    /// The page's own id: every widget's id is made from it, so two pages can share a window.
    page_id: Id,
}

impl Drawing {
    pub fn new(registry: Registry, page_id: Id) -> Drawing {
        Drawing {
            registry,
            instances: HashMap::new(),
            used: HashSet::new(),
            shown: HashSet::new(),
            words: Vec::new(),
            acts: HashMap::new(),
            left: Vec::new(),
            look: Look::default(),
            measured: Measured::default(),
            spaces: Vec::new(),
            errors: Vec::new(),
            page_id,
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Replaces one widget, or adds one: what it drew is made again.
    pub fn set<W: Widget + 'static>(&mut self, name: &str, make: impl Fn() -> W + 'static) {
        self.registry.set(name, make);
        self.instances.retain(|(_, w), _| w != name);
    }

    pub fn set_registry(&mut self, registry: Registry) {
        self.registry = registry;
        self.forget();
    }

    /// Every widget's id is made from this one.
    pub fn set_id(&mut self, id: Id) {
        self.page_id = id;
        self.forget();
    }

    /// Keeps no widget's own state: another page is drawn in this one's place.
    pub fn forget(&mut self) {
        self.instances.clear();
    }

    pub fn set_dark(&mut self, dark: bool) {
        self.look.set_dark(dark);
    }

    /// Draws a page's nodes, one frame of them, under one another.
    pub fn show(&mut self, host: &mut dyn Host, ui: &mut Ui, nodes: &[Drawn]) {
        self.shown.clear();
        self.words.clear();
        self.used.clear();
        self.measured = Measured::default();
        let gap = self.look.space(host, "M");
        ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
        ui.style_mut().interaction.selectable_labels = false;
        for d in nodes {
            draw(host, self, ui, d);
        }
        // a widget that left the page leaves with its instance
        let used = &self.used;
        self.instances.retain(|k, _| used.contains(k));
    }

    /// Asks the widget at a path to do something through its own controls on the next frame.
    pub fn act(&mut self, path: &str, act: Act) {
        self.acts.entry(path.to_string()).or_default().push(act);
    }

    /// Whether acts are waiting for a frame.
    pub fn acting(&self) -> bool {
        !self.acts.is_empty()
    }

    /// The acts the widgets of the last frame didn't do themselves, for whoever draws them to.
    pub fn take_left(&mut self) -> Vec<(String, Act)> {
        let waiting = self.acts.drain().flat_map(|(p, acts)| acts.into_iter().map(move |a| (p.clone(), a)));
        let mut left = std::mem::take(&mut self.left);
        left.extend(waiting);
        left
    }

    /// What the last frame's spots and planes measured.
    pub fn measured(&self) -> &Measured {
        &self.measured
    }

    /// The paths drawn in the last frame, and the ones kept on the page hidden.
    pub fn shown(&self) -> HashSet<String> {
        self.shown.iter().cloned().chain(self.words.iter().map(|d| d.id.clone())).collect()
    }

    /// Something went wrong: kept for the host's log.
    pub fn error(&mut self, message: String) {
        self.errors.push(message);
    }

    pub fn take_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.errors)
    }
}

/// What a widget can do while it is drawn: on the page, with its slots, and with the tokens.
pub struct Ctx<'a> {
    host: &'a mut (dyn Host + 'a),
    frame: &'a mut Drawing,
    at: &'a Drawn,
    id: Id,
    acts: Vec<Act>,
}

impl<'a> Ctx<'a> {
    /// The widget being drawn.
    pub fn drawn(&self) -> &'a Drawn {
        self.at
    }

    /// Its path on the page: `s0/s1[2]/s3`.
    pub fn path(&self) -> &'a str {
        &self.at.id
    }

    /// Its egui id: the same across frames, following its key, so a loop row's widget keeps its
    /// focus and state when the row moves.
    pub fn id(&self) -> Id {
        self.id
    }

    /// An id for one of its parts.
    pub fn sub_id(&self, part: &str) -> Id {
        self.id.with(part)
    }

    /// The reader's event on this widget: runs the caller's handler, as one change.
    pub fn event(&mut self, name: &str, args: Vec<Value>) {
        if let Err(f) = self.host.event(&self.at.id, name, args) {
            self.frame.errors.push(format!("{} {}: {}", self.at.id, name, f));
        }
    }

    /// This widget writing one of its `mut` inputs, as a slider does: checked, then one change.
    /// Whether it took: a refused write leaves the value as it was.
    pub fn write(&mut self, input: &str, value: Value) -> bool {
        match self.host.write(&self.at.id, input, value) {
            Ok(took) => took,
            Err(f) => {
                self.frame.errors.push(format!("{} {}: {}", self.at.id, input, f));
                false
            }
        }
    }

    /// Whether an event does something: a widget is clickable only when its click is.
    pub fn listens(&self, event: &str) -> bool {
        self.host.listens(self.at, event)
    }

    /// What the type of the variable a `mut` input writes says of its numbers: a slider's ends.
    pub fn contract(&self, input: &str) -> Contract {
        self.host.contract(self.at, input)
    }

    /// A fn an input holds, called: what it returns, if anything.
    pub fn apply(&self, f: &Value, args: Vec<Value>) -> Result<Option<Value>, String> {
        self.host.apply(f, args)
    }

    /// Whether two values are equal, as `==` says.
    pub fn equal(&self, a: &Value, b: &Value) -> bool {
        self.host.equal(a, b)
    }

    /// An act a check asked of this widget, to do through its own controls. One it doesn't take
    /// is whoever draws it's to do after the frame.
    pub fn take_act(&mut self) -> Option<Act> {
        if self.acts.is_empty() {
            None
        } else {
            Some(self.acts.remove(0))
        }
    }

    /// Puts back an act this widget can't do itself.
    pub fn decline(&mut self, act: Act) {
        self.frame.left.push((self.at.id.clone(), act));
    }

    /// Draws one widget of the page into a ui.
    pub fn child(&mut self, ui: &mut Ui, d: &Drawn) {
        draw(self.host, self.frame, ui, d);
    }

    /// Draws a slot's widgets into a ui, in order.
    pub fn slot(&mut self, ui: &mut Ui, name: &str) {
        let at = self.at;
        for d in at.slots.get(name).into_iter().flatten() {
            draw(self.host, self.frame, ui, d);
        }
    }

    /// A slot's widgets, kept on the page but not drawn: a closed overlay's body.
    pub fn keep(&mut self, name: &str) {
        let at = self.at;
        self.keep_nodes(at.slots.get(name).map_or(&[], Vec::as_slice));
    }

    /// Widgets drawn by the one that holds them, as a sentence's words are, or not at all.
    pub fn keep_nodes(&mut self, nodes: &[Drawn]) {
        for d in nodes {
            self.keep_node(d);
            let kids: Vec<Drawn> = d.children().cloned().collect();
            self.keep_nodes(&kids);
        }
    }

    /// The words and marks of a sentence, which its widget draws, and not the other widgets in it.
    pub(crate) fn keep_words(&mut self, nodes: &[Drawn]) {
        for d in nodes {
            match d.widget.as_str() {
                "core/words" | "core/code_span" | "core/inline_math" | "core/inline_image" | "core/note_mark" | "core/line_break" => self.frame.words.push(d.clone()),
                "core/bold" | "core/italic" | "core/strike" | "core/link" => {
                    self.frame.words.push(d.clone());
                    self.keep_words(d.slots.get("body").map_or(&[], Vec::as_slice));
                }
                _ => {}
            }
        }
    }

    fn keep_node(&mut self, d: &Drawn) {
        self.frame.shown.insert(d.id.clone());
        self.frame.used.insert((d.id.clone(), d.widget.clone()));
    }

    /// The page's tokens, for its theme.
    pub fn look(&self) -> &Look {
        &self.frame.look
    }

    /// A token case's value, as the build gives it; anything else is itself.
    pub fn token(&self, v: Option<&Value>) -> Option<Value> {
        self.frame.look.value(self.host, v)
    }

    /// A font token by name: `BODY`, `HEADING`, `MONO`.
    pub fn font(&self, name: &str) -> look::Font {
        self.frame.look.font(self.host, name)
    }

    /// A font at a text size.
    pub fn sized(&self, font: Option<&str>, size: Option<&str>) -> look::Font {
        self.frame.look.sized(self.host, font, size)
    }

    /// A colour token, for the page's theme.
    pub fn color(&self, v: Option<&Value>) -> Option<egui::Color32> {
        self.frame.look.color(self.host, v)
    }

    /// One of core's colours by name: `INK`, `ACCENT` …
    pub fn core_color(&self, name: &str) -> egui::Color32 {
        self.frame.look.core_color(self.host, name)
    }

    /// A length token, in points.
    pub fn length(&self, v: Option<&Value>) -> Option<f32> {
        self.frame.look.number(self.host, v)
    }

    /// The root of one of our widgets: a ui with this widget's id, named as its root for the
    /// AccessKit tree (`press@s0/s0`), its bounds set once it is drawn.
    pub fn root<R>(&mut self, ui: &mut Ui, builder: UiBuilder, add: impl FnOnce(&mut Ui, &mut Ctx<'a>) -> R) -> InnerResponse<R> {
        let id = self.id;
        let author = a11y::root_id(&self.at.widget, &self.at.id);
        ui.scope_builder(builder.scope_id(id), |ui| {
            a11y::part(ui, ui.unique_id(), &author);
            let r = add(ui, self);
            a11y::bounds(ui, ui.unique_id(), ui.min_rect());
            r
        })
    }

    /// Names a node as one of this widget's parts: `slider.input`.
    pub fn part(&self, ui: &Ui, id: Id, part: &str) {
        a11y::part(ui, id, &format!("{}.{}", a11y::short(&self.at.widget), part));
    }

    /// The drawing space shapes are in now: the canvas's or the plane's.
    pub fn space(&self) -> Option<Space> {
        self.frame.spaces.last().copied()
    }

    /// Draws with a space for the shapes inside.
    pub fn in_space<R>(&mut self, space: Space, f: impl FnOnce(&mut Ctx<'a>) -> R) -> R {
        self.frame.spaces.push(space);
        let r = f(self);
        self.frame.spaces.pop();
        r
    }

    /// A spot's size as drawn, in the plane's units, for the plane's layout.
    pub fn measured_spot(&mut self, size: egui::Vec2) {
        self.frame.measured.spots.insert(self.at.id.clone(), size);
    }

    /// A plane's width as drawn, in points, for its layout.
    pub fn measured_plane(&mut self, width: f32) {
        self.frame.measured.planes.insert(self.at.id.clone(), width);
    }

    /// Something went wrong drawing: kept for the host's log ([`Drawing::take_errors`]).
    pub fn error(&mut self, message: String) {
        self.frame.errors.push(message);
    }
}

/// Draws one widget: its instance, made the first time its key is seen and kept while it is.
fn draw(host: &mut dyn Host, frame: &mut Drawing, ui: &mut Ui, d: &Drawn) {
    frame.shown.insert(d.id.clone());
    let mut key = d.id.clone();
    let mut n = 2;
    while frame.used.contains(&(key.clone(), d.widget.clone())) {
        key = format!("{}#{}", d.id, n);
        n += 1;
    }
    // a widget that failed is drawn by another, and is another when it draws again
    let k = (key, if d.failed.is_some() { format!("{} failed", d.widget) } else { d.widget.clone() });
    frame.used.insert(k.clone());
    let mut w = match frame.instances.remove(&k) {
        Some(w) => w,
        None => frame.registry.make(&d.widget, d.failed.is_some()),
    };
    let acts = frame.acts.remove(&d.id).unwrap_or_default();
    let id = frame.page_id.with((&k.0, &k.1));
    let props = Props { d };
    let mut ctx = Ctx { host, frame, at: d, id, acts };
    w.show(ui, &props, &mut ctx);
    let left = std::mem::take(&mut ctx.acts);
    for a in left {
        frame.left.push((d.id.clone(), a));
    }
    frame.instances.insert(k, w);
}
