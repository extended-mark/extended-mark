//! A page running: the lowered IR mounted, redrawn after every change, with every change made
//! through a transaction that is checked and undone when it fails (page 6, plan tab 11). A port
//! of @extended-mark/runtime's `page.ts`, kept to its order of evaluation, its messages and its sites, so
//! the native and web engines observe the same page.
//!
//! No drawing: a page is drawn by whoever reads its tree (the egui and Android engines). Each widget is identified by its path: the sites of the template
//! instances it belongs to, each loop row as `[position]`, then its own site. Its key is who it is
//! across redraws: a loop row's widgets follow its item, not its position.
//!
//! The tree holds every prop already worked out, so an engine that draws every frame (egui) only
//! reads it: nothing is evaluated per frame, only once per change.

use std::cell::{Cell, RefCell, RefMut};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use xtmd_eval::interp::{Outer, Place, PlaceRoot, Step, R};
use xtmd_eval::value::{Case, Closure, Content, ContentOf, List, Record};
use xtmd_eval::{Binding, Constants, FailKind, Failure, Host, Machine, Num, Program, Row, Value};
use xtmd_ir::check::{Call, CallCall, Mounted, PageTheme, PageWidth};
use xtmd_ir::ir::{self, Expr, Node, Stmt};

pub use xtmd_ir::check::Broken;

/// The locals around a node: values, or references to places (a `mut for` row's item).
pub type Bindings = Vec<(Rc<str>, Binding)>;

/// A value fn the engine supplies, by full name (`core/moment.local`): given the page's machine
/// and the arguments, its value, or `None` when it returns nothing.
pub type ValueFn = Rc<dyn Fn(&mut Machine<'_>, &[Value]) -> R<Option<Value>>>;

/// Whether the engine draws an extern widget, by full name.
pub type Supplies = Rc<dyn Fn(&str) -> bool>;

/// What the host tells the page about where it is read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PageValues {
    pub width: Option<PageWidth>,
    pub theme: Option<PageTheme>,
}

/// The page's data: `$state`, and every widget's own variables by the place it is drawn at.
#[derive(Clone, Debug, Default)]
pub struct PageData {
    pub state: Vec<(String, Expr)>,
    pub own: BTreeMap<String, BTreeMap<String, Expr>>,
}

/// How a page is mounted: what the host and the engine give it.
#[derive(Clone)]
pub struct PageOptions {
    pub page: PageValues,
    /// `$state`'s values, in the IR's form, over its starting ones: checked as a host's write is, and what
    /// doesn't fit is in the outcome and keeps its starting value.
    pub state: Vec<(String, Expr)>,
    /// The page's data to start from: `$state` as one checked write, refused whole, and then widgets' own
    /// variables, each where it fits. The page is drawn once, with them.
    pub data: Option<PageData>,
    /// Each channel's values, in the IR's form: `("doc/scores", [("unit", …)])`.
    pub externs: Vec<(String, Vec<(String, Expr)>)>,
    /// The value fns the engine supplies, by full name.
    pub fns: HashMap<String, ValueFn>,
    /// Whether the engine draws an extern widget. One it doesn't shows its Markdown instead
    /// (page 6), and is reported in [`Page::missing`]. Left out, every one is drawn.
    pub supplies: Option<Supplies>,
    /// Reuse a widget's drawn subtree when nothing it read has changed (on by default). Off
    /// draws every widget anew on every change, for comparing the two.
    pub memo: bool,
}

impl Default for PageOptions {
    fn default() -> PageOptions {
        PageOptions { page: PageValues::default(), state: Vec::new(), data: None, externs: Vec::new(), fns: HashMap::new(), supplies: None, memo: true }
    }
}

/// Where a node's values are read: its widget instance, the locals around it, its path as
/// observed (loop rows by position) and its identity (loop rows by their item).
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub owner: Option<u64>,
    pub bindings: Bindings,
    pub path: String,
    pub key: Option<String>,
    /// The content whose nodes are being drawn, kept so that a call in it is known again.
    pub keep: Option<Arc<Vec<Node>>>,
}

impl Env {
    fn key_or_path(&self) -> &str {
        self.key.as_deref().unwrap_or(&self.path)
    }
}

/// An event a drawn widget's caller set: its handler, and where it runs.
#[derive(Clone, Debug)]
pub struct Handle {
    pub handler: String,
    pub env: Env,
}

/// A `mut` input given a variable: where the widget writes.
#[derive(Clone, Debug)]
pub struct WriteTo {
    pub place: ir::Place,
    pub env: Env,
}

/// A drawn extern widget, as the page asks an engine to draw it: a shared node that never
/// changes once drawn. A redraw that reuses a widget's subtree (nothing it read changed) hands out
/// the very same nodes, so an engine can skip one with [`Drawn::same`] as the one it drew last.
/// Its fields are read through `Deref`, as [`DrawnNode`]'s.
#[derive(Clone, Debug)]
pub struct Drawn(Rc<DrawnNode>);

impl Drawn {
    pub fn new(node: DrawnNode) -> Drawn {
        Drawn(Rc::new(node))
    }

    /// A node of nothing: where a value holds no widgets.
    pub(crate) fn nothing() -> Drawn {
        Drawn::new(DrawnNode { path: String::new(), id: String::new(), widget: String::new(), props: BTreeMap::new(), slots: BTreeMap::new(), events: BTreeMap::new(), writes: BTreeMap::new(), levels: Vec::new(), failed: None, held: Vec::new() })
    }

    /// Whether two are the very same node: the page reused it, and nothing in it changed.
    pub fn same(a: &Drawn, b: &Drawn) -> bool {
        Rc::ptr_eq(&a.0, &b.0)
    }
}

impl std::ops::Deref for Drawn {
    type Target = DrawnNode;

    fn deref(&self) -> &DrawnNode {
        &self.0
    }
}

/// What a drawn widget holds. Its props are worked out: an engine only reads them.
#[derive(Clone, Debug)]
pub struct DrawnNode {
    /// Where it is drawn, loop rows by their position: how its own variables are kept (`observe_own`).
    pub path: String,
    /// Who it is: its own on the page, and the same for as long as it is there. A loop row's widgets
    /// follow its item, not its position.
    pub id: String,
    pub widget: String,
    pub props: BTreeMap<String, Value>,
    pub slots: BTreeMap<String, Vec<Drawn>>,
    /// Events the caller set, each to a handler.
    pub events: BTreeMap<String, Handle>,
    /// `mut` inputs given a variable.
    pub writes: BTreeMap<String, WriteTo>,
    pub levels: Vec<ir::Level>,
    /// A widget that failed: why, shown in its place (page 6). `widget` is the one that did.
    pub failed: Option<String>,
    /// The calls its inputs hold, drawn: what a `Content` in `props` stands for.
    pub held: Vec<Vec<Drawn>>,
}

impl DrawnNode {
    pub fn prop(&self, name: &str) -> Option<&Value> {
        self.props.get(name)
    }

    /// Every widget inside it, in order: its slots', and those its inputs hold.
    pub fn children(&self) -> impl Iterator<Item = &Drawn> {
        self.slots.values().flatten().chain(self.held.iter().flatten())
    }

    /// The calls a value of one of its inputs holds, drawn: a rich_text's or a view's.
    pub fn nodes(&self, v: &Value) -> Option<&[Drawn]> {
        match v {
            Value::Content(c) => match &c.of {
                ContentOf::Drawn(k) => self.held.get(*k).map(Vec::as_slice),
                _ => None,
            },
            Value::Opt(Some(x)) => self.nodes(x),
            _ => None,
        }
    }
}

/// Refused writes and failures since they were last taken.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    pub refused: Vec<Broken>,
    pub failures: Vec<Broken>,
}

/// A shade drawn: the fn it was given, by name ("" for a lambda), and its arguments; with
/// samples, the colour at the area's two corners and its middle.
#[derive(Clone, Debug, PartialEq)]
pub struct Shaded {
    pub shader: String,
    pub args: Expr,
    pub samples: Option<Vec<(Expr, Expr)>>,
}

fn broken_code(code: &str, site: &str) -> Broken {
    Broken { code: Some(code.to_string()), rule: None, site: Some(site.to_string()) }
}

/// A change from outside that failed, as the host is told.
fn refused(f: &Failure, site: &str) -> Broken {
    match &f.rule {
        Some(rule) => Broken { code: None, rule: Some(rule.clone()), site: Some(site.to_string()) },
        None => broken_code("contracts.outside", site),
    }
}

fn broken_rule(rule: &str) -> Broken {
    Broken { code: None, rule: Some(rule.to_string()), site: None }
}

pub(crate) fn join(path: &str, part: &str) -> String {
    if path.is_empty() {
        part.to_string()
    } else {
        format!("{}/{}", path, part)
    }
}

/// A failure's code, as the checks report it: `failure.rule`, `failure.limit`.
pub fn failure_code(f: &Failure) -> String {
    let kind = match f.kind {
        FailKind::Number(_) => "number",
        FailKind::Rule => "rule",
        FailKind::Position => "position",
        FailKind::Removed => "removed",
        FailKind::Limit(_) => "limit",
        FailKind::NotConstant => "notconstant",
        FailKind::Ir => "ir",
    };
    format!("failure.{}", kind)
}

fn outer_kind(o: &Outer) -> (&'static str, &str) {
    match o {
        Outer::Cell(n) => ("cell", n),
        Outer::Input(n) => ("input", n),
        Outer::State(n) => ("state", n),
        Outer::Let(n) => ("let", n),
        Outer::Extern(n) => ("extern", n),
        Outer::Slot(n) => ("slot", n),
    }
}

/// What a widget is given: expressions to work out where it is called, or the values of a call
/// made as one.
#[derive(Clone, Copy)]
pub(crate) enum Args<'a> {
    Exprs(&'a BTreeMap<String, Expr>),
    Values(&'a BTreeMap<String, Value>),
}

/// Whether a piece of Markdown's input or slot holds a sentence, and no blocks.
pub(crate) fn takes_sentence(piece: &str, part: &str) -> bool {
    matches!((piece, part), ("paragraph" | "heading", "value") | ("table", "head" | "rows") | ("bold" | "italic" | "strike" | "link", "body"))
}

/// A sentence goes here, so a rich_text of blocks doesn't.
pub(crate) fn check_sentence(v: &Value) -> R<()> {
    match v {
        Value::Content(c) if c.blocks => Err(Failure::ir("a sentence goes here, and this rich_text is blocks")),
        Value::Opt(Some(x)) => check_sentence(x),
        Value::List(l) => l.items.iter().try_for_each(|i| check_sentence(&i.value)),
        _ => Ok(()),
    }
}

pub(crate) fn case_value(ty: &str, name: &str) -> Value {
    Value::Case(Rc::new(Case { ty: Rc::from(ty), name: Rc::from(name), values: Vec::new() }))
}

pub(crate) type Nodes = Arc<Vec<Node>>;
type Body = Rc<Vec<Stmt>>;

/// A rule: its body and its message.
/// A widget's or `$state`'s rules: each body, its message, and whether it only warns.
type Rules = Rc<Vec<(Body, String, bool)>>;

fn rules_of<'a>(rules: impl Iterator<Item = &'a ir::Require>) -> Rules {
    Rc::new(rules.map(|r| (Rc::new(r.body.clone()), r.message.clone(), r.warn)).collect())
}

struct HandlerFn {
    params: Vec<ir::Param>,
    body: Body,
    /// The event it only passes on: `fn() { invoke click(); }`.
    passes: Option<String>,
}

/// What a page knows from its IR and its options, which never changes.
pub(crate) struct Shared {
    pub(crate) module: ir::Module,
    pub(crate) program: Program,
    pub(crate) templates: HashMap<String, Rc<ir::Template>>,
    template_rules: HashMap<String, Rules>,
    pub(crate) extern_widgets: HashMap<String, Rc<ir::ExternWidget>>,
    /// Each extern widget as a template, for the instance its Markdown is written in.
    extern_templates: HashMap<String, Rc<ir::Template>>,
    handlers: HashMap<String, HandlerFn>,
    host_fns: HashSet<String>,
    state_decl: Option<ir::State>,
    state_rules: Rules,
    lets: HashMap<String, Expr>,
    engine_fns: HashMap<String, ValueFn>,
    /// The nodes the page owns, so an instance holds a slot's content without a copy, and a
    /// memo knows its call again.
    stable: Stable,
}

impl Shared {
    /// Nodes an instance or a held slot keeps: the template's own when they are its, a copy when
    /// a value made them.
    fn nodes_rc(&self, nodes: &Vec<Node>) -> Nodes {
        match self.stable.lists.get(&addr(nodes)) {
            Some(n) => n.clone(),
            None => Arc::new(nodes.clone()),
        }
    }

    /// The extern call a node stands for, when the page owns it: a node made by a value has none,
    /// and is never the same call as another.
    fn call_id(&self, x: &ir::NodeExtern) -> Option<usize> {
        self.stable.externs.get(&addr(x)).copied()
    }

    /// The node of a view this is, when the page owns it.
    fn node_id(&self, n: &Node) -> Option<usize> {
        self.stable.nodes.get(&addr(n)).copied()
    }

    /// Whether two slots hold the same content: the same shared copy, or nodes that are equal.
    fn same_slot(&self, held: &Nodes, now: &Vec<Node>) -> bool {
        match self.stable.lists.get(&addr(now)) {
            Some(rc) if Arc::ptr_eq(rc, held) => true,
            _ => **held == *now,
        }
    }

}

/// The values a call made as a value was given.
fn arg_values(args: Args) -> Vec<Value> {
    match args {
        Args::Values(v) => v.values().cloned().collect(),
        Args::Exprs(_) => Vec::new(),
    }
}

fn addr<T>(x: &T) -> usize {
    x as *const T as usize
}

/// The nodes a page owns for as long as it runs: its templates' views and Markdown, its extern
/// widgets' Markdown, and one shared copy of each slot's content, which instances and held slots
/// draw from. Every copy of a slot's content maps to that one copy, and every copy of an extern
/// call to the call it copies, so "the same slot" and "the same call" are decided as the web
/// decides them, by identity, where nothing is ever copied.
#[derive(Default)]
struct Stable {
    lists: HashMap<usize, Nodes>,
    /// Each extern call the page owns, by where it is, to the original it stands for.
    externs: HashMap<usize, usize>,
    /// Each node the same way: a loop, a branch, a place, a local.
    nodes: HashMap<usize, usize>,
}

impl Stable {
    /// A slot's content as the one shared copy every instance holds.
    fn share(&mut self, s: &Vec<Node>) -> Nodes {
        if let Some(rc) = self.lists.get(&addr(s)) {
            return rc.clone();
        }
        let rc = Arc::new(s.clone());
        self.lists.insert(addr(s), rc.clone());
        self.link(s, &rc);
        rc
    }

    /// Registers the extern calls and slot contents of `orig`, an original, and of `copy`, a
    /// copy of it (or `orig` itself): each copy's to its original's.
    fn link(&mut self, orig: &[Node], copy: &[Node]) {
        for (o, c) in orig.iter().zip(copy) {
            self.nodes.insert(addr(o), addr(o));
            self.nodes.insert(addr(c), addr(o));
            match (o, c) {
                (Node::Extern(xo), Node::Extern(xc)) => {
                    self.externs.insert(addr(xo), addr(xo));
                    self.externs.insert(addr(xc), addr(xo));
                    self.slots(&xo.slots, &xc.slots);
                }
                (Node::Use(uo), Node::Use(uc)) => self.slots(&uo.slots, &uc.slots),
                (Node::For(fo), Node::For(fc)) => self.link(&fo.body, &fc.body),
                (Node::If(io), Node::If(ic)) => {
                    for (bo, bc) in io.branches.iter().zip(&ic.branches) {
                        self.link(&bo.then, &bc.then);
                    }
                    self.link(&io.else_, &ic.else_);
                }
                (Node::Match(mo), Node::Match(mc)) => {
                    for (co, cc) in mo.cases.iter().zip(&mc.cases) {
                        self.link(&co.then, &cc.then);
                    }
                    self.link(&mo.else_, &mc.else_);
                }
                _ => {}
            }
        }
    }

    fn slots(&mut self, orig: &BTreeMap<String, Vec<Node>>, copy: &BTreeMap<String, Vec<Node>>) {
        for ((_, so), (_, sc)) in orig.iter().zip(copy) {
            let rc = self.share(so);
            if !std::ptr::eq(so, sc) {
                self.lists.insert(addr(sc), rc);
                self.link(so, sc);
            }
        }
    }
}

#[derive(Debug)]
struct Part {
    place: Place,
    was: Value,
    /// What the place is in, and what was read to get there, with their versions: while they are as they
    /// were, so is it.
    deps: HashMap<String, u64>,
}

/// What drawing one part of the page read and met, itself: what is inside it has its own, in `kids`. A
/// redraw reuses it while none of it changed.
#[derive(Debug, Default)]
struct Frame {
    /// Each outside value read, by key (`s:field`, `x:channel.value`, `c:id:cell`, `o:key`),
    /// with its version then.
    deps: HashMap<String, u64>,
    /// Each part of a value read at its place, an item of a list among them, and what was there: another
    /// part that changes is nothing to it.
    parts: RefCell<Vec<Part>>,
    kids: Vec<Rc<Frame>>,
    /// The instances drawn here, with their paths: seen again when it is reused.
    nested: Vec<(u64, String)>,
    own: Vec<String>,
    missed: Vec<String>,
    /// It failed, or asked the host for a value: drawn anew every time.
    dirty: bool,
    /// The last redraw it is a part of.
    seen: Cell<u64>,
    /// When everything it read was last found as it was; `STALE` once something wasn't, which it never is
    /// again.
    fresh: Cell<u64>,
}

const STALE: u64 = u64::MAX;

/// A widget written in Extended Mark as it was last drawn, with what its drawing read.
struct Memo {
    frame: Rc<Frame>,
    template: String,
    path: String,
    owner: Option<u64>,
    args: Vec<Value>,
    bindings: Bindings,
    slots: Vec<(String, Nodes)>,
    events: BTreeMap<String, String>,
    epoch: u64,
    drawn: Vec<Drawn>,
}

/// An extern call as it was last drawn, with what its drawing read.
struct ExternMemo {
    frame: Rc<Frame>,
    /// The call it was: [`Shared::call_id`], or where it is in the content it is kept with.
    node: usize,
    keep: Option<Arc<Vec<Node>>>,
    path: String,
    owner: Option<u64>,
    bindings: Bindings,
    epoch: u64,
    drawn: Vec<Drawn>,
}

/// What a loop, a branch, a place or a local of a view gave, with what it read.
struct NodeMemo {
    frame: Rc<Frame>,
    node: usize,
    keep: Option<Arc<Vec<Node>>>,
    path: String,
    owner: Option<u64>,
    bindings: Bindings,
    epoch: u64,
    drawn: Vec<Drawn>,
    value: Option<Value>,
}

/// Whether two values are the very same one, as `===` says on the web: shared, not equal.
fn same_ptr(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Text(x), Value::Text(y)) => Rc::ptr_eq(x, y),
        (Value::List(x), Value::List(y)) => Rc::ptr_eq(x, y),
        (Value::Record(x), Value::Record(y)) => Rc::ptr_eq(x, y),
        (Value::Case(x), Value::Case(y)) => Rc::ptr_eq(x, y),
        (Value::Range(x), Value::Range(y)) => Rc::ptr_eq(x, y),
        (Value::Fn(x), Value::Fn(y)) => Rc::ptr_eq(x, y),
        (Value::View(x), Value::View(y)) => Rc::ptr_eq(x, y),
        (Value::Content(x), Value::Content(y)) => Rc::ptr_eq(x, y),
        (Value::Opt(Some(x)), Value::Opt(Some(y))) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

/// Whether what is at a place is what it was: an opt is made anew each time it is read.
fn same_now(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Opt(None), Value::Opt(None)) => true,
        (Value::Opt(Some(x)), Value::Opt(Some(y))) => Rc::ptr_eq(x, y) || same_now(x, y),
        (Value::Num(x), Value::Num(y)) | (Value::Duration(x), Value::Duration(y)) => x == y && x.show() == y.show(),
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Text(x), Value::Text(y)) => x == y,
        _ => same_ptr(a, b),
    }
}

/// Whether two references are to the same place.
fn same_place(a: &Place, b: &Place) -> bool {
    let root = match (&a.root, &b.root) {
        (PlaceRoot::Local { frame: f, name: n }, PlaceRoot::Local { frame: g, name: o }) => f == g && n == o,
        (PlaceRoot::Outer(x, p), PlaceRoot::Outer(y, q)) => x == y && p == q,
        _ => false,
    };
    root && a.path.len() == b.path.len()
        && a.path.iter().zip(&b.path).all(|(s, t)| match (s, t) {
            (Step::Field(x), Step::Field(y)) => x == y,
            (Step::Index(x), Step::Index(y)) => x == y,
            (Step::Item(x), Step::Item(y)) | (Step::ItemOpt(x), Step::ItemOpt(y)) => x == y,
            (Step::Inside, Step::Inside) => true,
            _ => false,
        })
}

/// A widget instance: one call of a widget written in Extended Mark, kept by its key across redraws.
struct Instance {
    id: u64,
    path: String,
    template: Rc<ir::Template>,
    inputs: HashMap<String, Value>,
    /// `mut` inputs: the caller's place, read and written live.
    places: HashMap<String, WriteTo>,
    cells: HashMap<String, Value>,
    slots: HashMap<String, (Nodes, Env)>,
    events: HashMap<String, Handle>,
    /// The last redraw it is drawn in.
    seen: u64,
}

/// What changes while a page runs.
#[derive(Default)]
pub(crate) struct World {
    instances: HashMap<u64, Instance>,
    /// Instances by identity: rows follow their items, so a row's widget keeps its variables.
    by_path: HashMap<String, u64>,
    /// The same instances by the path they were last drawn at.
    at_path: HashMap<String, u64>,
    /// Extern widgets' own `mut` values, for inputs the caller gave no variable, by identity.
    own: HashMap<String, BTreeMap<String, Value>>,
    own_seen: HashSet<String>,
    /// Who has been drawn this redraw: content placed twice is two widgets.
    claimed: HashSet<String>,
    /// How many inputs' own calls are being drawn around here: a sentence's, or a view's.
    in_inputs: u32,
    /// Own variables to put back as the page is drawn, by place: what is left after had no place.
    put_back: BTreeMap<String, BTreeMap<String, Expr>>,
    /// `$state`, in the order its values were given.
    state: Vec<(String, Value)>,
    externs: HashMap<String, Value>,
    calls: Vec<Call>,
    outcome: Outcome,
    /// Extern widgets drawn as their Markdown because the engine doesn't supply them.
    missed: BTreeSet<String>,
    supplies: Option<Supplies>,
    errors: Vec<Failure>,
    /// Warnings for the host, since it last took them: not failures, the page carries on.
    warnings: Vec<Broken>,
    /// The `warn` rules that don't hold now, so each is told once as it becomes false.
    warns: HashSet<String>,
    /// The machine's step count and its last identity, kept between the machines a page makes.
    steps: u64,
    worked: u64,
    next_id: u64,
    /// Each outside value's version, bumped when it is written, and the epoch, bumped when every
    /// memo must go (an undo, what the engine supplies).
    versions: HashMap<String, u64>,
    epoch: u64,
    memo: HashMap<String, Rc<Memo>>,
    extern_memo: HashMap<String, Rc<ExternMemo>>,
    node_memo: HashMap<String, Rc<NodeMemo>>,
    /// The nodes drawn anew this redraw.
    node_seen: HashSet<String>,
    /// Which redraw this is.
    draws: u64,
    /// Moves with every bump: what was found fresh at one is, until the next.
    stamp: u64,
    /// What is read on the way to a part, while it is.
    part_deps: Option<HashMap<String, u64>>,
    /// Above 0 while what is read is told by name: a held fn's calls, and `$state`'s rules.
    whole: u32,
    /// `$state`'s rules, by their place among them: whether each held, with what it read.
    rule_memo: HashMap<usize, (HashMap<String, u64>, u64, bool)>,
    /// What each widget being drawn has read and made so far, outermost first.
    frames: Vec<Frame>,
    memo_on: bool,
}

impl World {
    fn state_get(&self, name: &str) -> Option<Value> {
        self.state.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    }

    /// A read of an outside value, for the part being drawn.
    fn track(&mut self, key: impl FnOnce() -> String) {
        let Some(deps) = self.part_deps.as_mut().or(self.frames.last_mut().map(|f| &mut f.deps)) else { return };
        let k = key();
        if !deps.contains_key(&k) {
            let v = self.versions.get(&k).copied().unwrap_or(0);
            deps.insert(k, v);
        }
    }

    /// A part of the page that starts to be drawn.
    fn enter(&mut self) {
        let frame = Frame { seen: Cell::new(self.draws), ..Frame::default() };
        self.frames.push(frame);
    }

    /// And ends: it is inside the part around it.
    fn exit(&mut self) -> Rc<Frame> {
        let frame = Rc::new(self.frames.pop().unwrap_or_default());
        if let Some(around) = self.frames.last_mut() {
            around.dirty |= frame.dirty;
            around.kids.push(frame.clone());
        }
        frame
    }

    /// What is being drawn is drawn anew every time.
    fn soil(&mut self) {
        if let Some(top) = self.frames.last_mut() {
            top.dirty = true;
        }
    }

    /// A part that is drawn again as it was, with everything in it.
    fn see(&mut self, frame: &Frame) {
        if frame.seen.replace(self.draws) == self.draws {
            return;
        }
        for (id, path) in &frame.nested {
            if let Some(i) = self.instances.get_mut(id) {
                i.seen = self.draws;
            }
            self.at_path.insert(path.clone(), *id);
        }
        self.own_seen.extend(frame.own.iter().cloned());
        self.missed.extend(frame.missed.iter().cloned());
        for k in &frame.kids {
            self.see(k);
        }
    }

    /// Whether a name is another widget's already, this redraw.
    fn taken(&self, id: &str) -> bool {
        self.claimed.contains(id)
            || self.extern_memo.get(id).map_or(false, |x| x.frame.seen.get() == self.draws)
            || self.by_path.get(id).and_then(|i| self.instances.get(i)).map_or(false, |i| i.seen == self.draws)
    }

    /// A `warn` rule, worked out: told once as it becomes false.
    fn warned(&mut self, site: &str, message: &str, holds: bool) {
        let at = format!("{}\n{}", site, message);
        if holds {
            self.warns.remove(&at);
        } else if self.warns.insert(at) {
            self.warnings.push(Broken { code: None, rule: Some(message.to_string()), site: Some(site.to_string()) });
        }
    }

    /// A record's `warn` rules have no place of their own on the page: each is told once.
    fn records_warned(&mut self, messages: Vec<String>) {
        for message in messages {
            if self.warns.insert(message.clone()) {
                self.warnings.push(Broken { code: None, rule: Some(message), site: None });
            }
        }
    }

    /// An outside value written: every memo that read it is out of date.
    fn bump(&mut self, key: String) {
        *self.versions.entry(key).or_insert(0) += 1;
        self.stamp += 1;
    }

    /// Whether every value a memo read is as it was.
    fn current(&self, deps: &HashMap<String, u64>) -> bool {
        deps.iter().all(|(k, v)| self.versions.get(k).copied().unwrap_or(0) == *v)
    }

    fn state_set(&mut self, name: &str, v: Value) {
        match self.state.iter_mut().find(|(k, _)| k == name) {
            Some(slot) => slot.1 = v,
            None => self.state.push((name.to_string(), v)),
        }
    }
}

/// The page's two halves, shared with the host its machines call back.
#[derive(Clone)]
pub(crate) struct Rt {
    pub(crate) s: Rc<Shared>,
    w: Rc<RefCell<World>>,
}

struct PageHost(Rt);

impl Host for PageHost {
    fn read(&self, m: &mut Machine, what: &Outer, owner: Option<u64>) -> R<Value> {
        self.0.host_read(m, what, owner)
    }

    fn read_at(&self, m: &mut Machine, place: &Place) -> R<Value> {
        {
            let w = self.0.w.borrow();
            if w.frames.is_empty() || w.part_deps.is_some() || w.whole > 0 {
                drop(w);
                return m.value_at(place);
            }
        }
        let (was, deps) = self.0.part_at(m, place)?;
        if let Some(f) = self.0.w().frames.last_mut() {
            f.parts.get_mut().push(Part { place: place.clone(), was: was.clone(), deps });
        }
        Ok(was)
    }

    fn write(&self, m: &mut Machine, what: &Outer, owner: Option<u64>, value: Value) -> R<()> {
        self.0.host_write(m, what, owner, value)
    }

    fn call_extern(&self, m: &mut Machine, name: &str, args: &[Value]) -> R<Option<Option<Value>>> {
        let rt = &self.0;
        if let Some(f) = rt.s.engine_fns.get(name).cloned() {
            let out = f(m, args)?;
            if let (Some(v), Some(t)) = (&out, rt.s.program.externs.get(name).and_then(|x| x.returns.as_ref())) {
                if !m.takes(v, t)? {
                    return Err(Failure::new(FailKind::Rule, format!("`{}` gave what isn't its type", name)));
                }
            }
            return Ok(Some(out));
        }
        if !rt.s.host_fns.contains(name) {
            return Ok(None);
        }
        // a widget that asks the host is drawn anew every time
        rt.w().soil();
        rt.w().calls.push(Call { call: CallCall { name: name.to_string(), args: args.iter().map(Value::to_ir).collect() } });
        Ok(Some(None))
    }

    fn invoke(&self, m: &mut Machine, event: &str, args: Vec<Value>, owner: Option<u64>) -> R<()> {
        let ev = owner.and_then(|o| self.0.w.borrow().instances.get(&o).and_then(|i| i.events.get(event).cloned()));
        match ev {
            Some(ev) => self.0.run_handler(m, &ev.handler, &ev.env, args).map(|_| ()),
            None => Ok(()),
        }
    }
}

impl Rt {
    fn w(&self) -> RefMut<'_, World> {
        self.w.borrow_mut()
    }

    // ---- the host: what lives outside a fn

    fn host_read(&self, m: &mut Machine, what: &Outer, owner: Option<u64>) -> R<Value> {
        match what {
            Outer::State(n) => {
                let mut w = self.w();
                w.track(|| format!("s:{}", n));
                w.state_get(n).ok_or_else(|| Failure::ir(format!("$state has no field {}", n)))
            }
            Outer::Extern(n) => {
                let mut w = self.w();
                w.track(|| format!("x:{}", n));
                w.externs.get(&**n).cloned().ok_or_else(|| Failure::new(FailKind::NotConstant, format!("{} is given by the host", n)))
            }
            Outer::Let(n) => {
                if let Some(v) = m.prog.lets.get(&**n) {
                    return Ok(v.clone());
                }
                match self.s.lets.get(&**n) {
                    Some(e) => m.eval(e),
                    None => Err(Failure::ir(format!("no let {}", n))),
                }
            }
            Outer::Input(n) => {
                let found = {
                    let w = self.w.borrow();
                    match owner.and_then(|o| w.instances.get(&o)) {
                        None => return Err(Failure::ir(format!("no input {} here", n))),
                        Some(i) => (i.places.get(&**n).cloned(), i.inputs.get(&**n).cloned()),
                    }
                };
                match found {
                    (Some(p), _) => m.eval_with(&p.env.bindings, p.env.owner, &Expr::Place(p.place)),
                    (None, Some(v)) => {
                        // a slot's content reads its caller's inputs inside the widget it is placed
                        // in, whose own inputs may not change: the read is what makes that widget
                        // be drawn again
                        if let Some(o) = owner {
                            self.w().track(|| format!("i:{}:{}", o, n));
                        }
                        Ok(v)
                    }
                    (None, None) => Err(Failure::ir(format!("no input {}", n))),
                }
            }
            Outer::Cell(n) => {
                let mut w = self.w();
                if let Some(o) = owner {
                    w.track(|| format!("c:{}:{}", o, n));
                }
                owner.and_then(|o| w.instances.get(&o)).and_then(|i| i.cells.get(&**n)).cloned().ok_or_else(|| Failure::ir(format!("no variable {}", n)))
            }
            Outer::Slot(n) => {
                let (t, held) = {
                    let mut w = self.w();
                    // read as an input is: whoever reads it is out of date when the caller gives it anew
                    if let Some(o) = owner {
                        w.track(|| format!("l:{}:{}", o, n));
                    }
                    let i = owner.and_then(|o| w.instances.get(&o)).ok_or_else(|| Failure::ir(format!("no slot {} here", n)))?;
                    (i.template.clone(), i.slots.get(&**n).cloned())
                };
                match held {
                    Some((nodes, env)) => {
                        let ty = t.slots.iter().find(|s| s.name == **n).map_or_else(|| ir::Type::named("core/view"), |s| s.ty.clone());
                        let key = env.key_or_path().to_string();
                        m.slot_value(&ty, &nodes, &env.bindings, env.owner, (Rc::from(env.path), Rc::from(key)))
                    }
                    // only an `opt` slot may be left out (page 3): the runtime makes up no value
                    None if t.slots.iter().any(|s| s.name == **n && s.ty.name == "core/opt") => Ok(Value::none()),
                    None => Err(Failure::ir(format!("the slot {} isn't filled", n))),
                }
            }
        }
    }

    fn host_write(&self, m: &mut Machine, what: &Outer, owner: Option<u64>, value: Value) -> R<()> {
        match what {
            Outer::State(n) => {
                let ty = self.s.state_decl.as_ref().and_then(|s| s.fields.iter().find(|f| *f.name == **n)).map(|f| f.ty.clone());
                if let Some(ty) = ty {
                    if !m.fits(&value, &ty)? {
                        return Err(Failure::new(FailKind::Rule, format!("$state.{} is outside its contract", n)));
                    }
                }
                let mut w = self.w();
                w.state_set(n, value);
                w.bump(format!("s:{}", n));
                Ok(())
            }
            Outer::Cell(n) => {
                let ty = {
                    let w = self.w.borrow();
                    let Some(inst) = owner.and_then(|o| w.instances.get(&o)) else {
                        return Err(Failure::ir(format!("no variable {} here", n)));
                    };
                    inst.template.cells.iter().find(|c| *c.name == **n).map(|c| c.ty.clone())
                };
                if let Some(ty) = ty {
                    if !m.fits(&value, &ty)? {
                        return Err(Failure::new(FailKind::Rule, format!("{} is outside its contract", n)));
                    }
                }
                if let Some(o) = owner {
                    let mut w = self.w();
                    if let Some(inst) = w.instances.get_mut(&o) {
                        inst.cells.insert(n.to_string(), value);
                        w.bump(format!("c:{}:{}", o, n));
                    }
                }
                Ok(())
            }
            Outer::Input(n) => {
                let p = owner.and_then(|o| self.w.borrow().instances.get(&o).and_then(|i| i.places.get(&**n).cloned()));
                match p {
                    Some(p) => m.set_place(&p.env.bindings, p.env.owner, &p.place, value),
                    None => Err(Failure::ir(format!("{} is not a mut input given a variable", n))),
                }
            }
            other => {
                let (kind, name) = outer_kind(other);
                Err(Failure::ir(format!("{} {} can't be written", kind, name)))
            }
        }
    }

    pub(crate) fn eval_in(&self, m: &mut Machine, env: &Env, e: &Expr) -> R<Value> {
        m.eval_with(&env.bindings, env.owner, e)
    }

    fn write_place(&self, m: &mut Machine, w: &WriteTo, value: Value) -> R<()> {
        m.set_place(&w.env.bindings, w.env.owner, &w.place, value)
    }

    pub(crate) fn loop_rows(&self, m: &mut Machine, f: &ir::NodeFor, env: &Env) -> R<Vec<Row>> {
        m.loop_rows(&env.bindings, env.owner, &f.over, &f.name, f.position.as_deref(), f.mutable)
    }

    /// The same rows with their keys: a list row's is its item's identity.
    pub(crate) fn keyed_rows(&self, m: &mut Machine, f: &ir::NodeFor, env: &Env) -> R<Vec<Row>> {
        self.loop_rows(m, f, env)
    }

    /// A `mut` input's write to the variable it was given: checked against the input's type, as an engine
    /// makes the value, and by the write against the variable's.
    fn write_into(&self, m: &mut Machine, widget: &str, input: &str, to: &WriteTo, value: Value) -> R<()> {
        let x = self.s.extern_widgets.get(widget).cloned();
        if let Some(f) = x.as_ref().and_then(|x| x.inputs.iter().find(|f| f.name == input)) {
            if !m.takes(&value, &f.ty)? {
                return Err(Failure::new(FailKind::Rule, format!("{} is given what isn't its type", input)));
            }
        }
        self.write_place(m, to, value)?;
        // an input of a type parameter's type takes any value: the variable's own type says which
        if x.map_or(false, |x| !x.generics.is_empty()) {
            if let Some((ty, v)) = self.root_of(m, to)? {
                if !m.takes(&v, &ty)? {
                    return Err(Failure::new(FailKind::Rule, format!("{} is given what isn't its variable's type", input)));
                }
            }
        }
        Ok(())
    }

    /// The variable a place is in, `$state`'s or a widget's own: its type, and its value now.
    fn root_of(&self, m: &mut Machine, to: &WriteTo) -> R<Option<(ir::Type, Value)>> {
        let PlaceRoot::Outer(outer, owner) = m.root_of(&to.env.bindings, to.env.owner, &to.place)? else { return Ok(None) };
        let w = self.w.borrow();
        let inst = owner.and_then(|o| w.instances.get(&o));
        let found = match &outer {
            Outer::State(n) => {
                let ty = self.s.state_decl.as_ref().and_then(|s| s.fields.iter().find(|f| *f.name == **n)).map(|f| f.ty.clone());
                ty.zip(w.state_get(n))
            }
            Outer::Cell(n) => {
                let ty = inst.and_then(|i| i.template.cells.iter().find(|c| *c.name == **n)).map(|c| c.ty.clone());
                ty.zip(inst.and_then(|i| i.cells.get(&**n).cloned()))
            }
            Outer::Input(n) => {
                let to = inst.and_then(|i| i.places.get(&**n).cloned());
                drop(w);
                return match to {
                    Some(to) => self.root_of(m, &to),
                    None => Ok(None),
                };
            }
            _ => None,
        };
        Ok(found)
    }

    /// The message of the first of `$state`'s rules that no longer holds.
    fn state_rule_broken(&self, m: &mut Machine) -> R<Option<String>> {
        let rules = self.s.state_rules.clone();
        for (k, (_, message, warn)) in rules.iter().enumerate() {
            if !*warn && !self.state_rule(m, k)? {
                return Ok(Some(message.clone()));
            }
        }
        Ok(None)
    }

    /// The same as a host is told: a rule that can't be worked out doesn't hold.
    fn state_refusal(&self, m: &mut Machine) -> Option<Broken> {
        match self.state_rule_broken(m) {
            Ok(rule) => rule.as_deref().map(broken_rule),
            Err(f) => {
                let broken = broken_code(&failure_code(&f), "$state");
                self.w().errors.push(f);
                Some(broken)
            }
        }
    }

    /// Whether one of `$state`'s rules holds: worked out again only when something it read changed.
    fn state_rule(&self, m: &mut Machine, k: usize) -> R<bool> {
        {
            let w = self.w.borrow();
            if let Some((deps, epoch, holds)) = w.rule_memo.get(&k) {
                if *epoch == w.epoch && w.current(deps) {
                    return Ok(*holds);
                }
            }
        }
        let body = self.s.state_rules[k].0.clone();
        let (ok, deps) = self.reading(|| self.run_body(m, &[], body, &Env::default(), Vec::new()));
        let holds = !matches!(ok?, Some(Value::Bool(false)));
        let mut w = self.w();
        let epoch = w.epoch;
        w.rule_memo.insert(k, (deps, epoch, holds));
        Ok(holds)
    }

    /// What some code read, for whoever holds what it gave.
    fn reading<T>(&self, f: impl FnOnce() -> T) -> (T, HashMap<String, u64>) {
        {
            let mut w = self.w();
            w.enter();
            w.whole += 1;
        }
        let out = f();
        let mut w = self.w();
        w.whole -= 1;
        let frame = w.exit();
        fn all(f: &Frame, deps: &mut HashMap<String, u64>) {
            for (k, v) in &f.deps {
                deps.entry(k.clone()).or_insert(*v);
            }
            f.kids.iter().for_each(|k| all(k, deps));
        }
        let mut deps = HashMap::new();
        all(&frame, &mut deps);
        (out, deps)
    }

    fn run_handler(&self, m: &mut Machine, id: &str, env: &Env, args: Vec<Value>) -> R<Option<Value>> {
        let h = self.s.handlers.get(id).ok_or_else(|| Failure::ir(format!("no handler {}", id)))?;
        self.run_body(m, &h.params, h.body.clone(), env, args)
    }

    fn run_body(&self, m: &mut Machine, params: &[ir::Param], body: Body, env: &Env, args: Vec<Value>) -> R<Option<Value>> {
        let f = Value::Fn(Rc::new(Closure { params: params.to_vec(), body, captured: env.bindings.clone(), owner: env.owner }));
        m.apply(&f, args.into_iter().map(Binding::Value).collect())
    }

    // ---- drawing

    pub(crate) fn pick_branch<'n>(&self, m: &mut Machine, n: &'n ir::NodeIf, env: &Env) -> R<(&'n Vec<Node>, Env)> {
        for b in &n.branches {
            match &b.when {
                ir::Cond::Cond(c) => {
                    if let Value::Bool(true) = self.eval_in(m, env, c)? {
                        return Ok((&b.then, env.clone()));
                    }
                }
                ir::Cond::Open(o) => {
                    if let Some(opened) = m.open_cond(&env.bindings, env.owner, o)? {
                        let mut inner = env.clone();
                        inner.bindings.push((Rc::from(o.name.as_str()), opened));
                        return Ok((&b.then, inner));
                    }
                }
            }
        }
        Ok((&n.else_, env.clone()))
    }

    pub(crate) fn draw_nodes(&self, m: &mut Machine, nodes: &[Node], env0: &Env) -> R<Vec<Drawn>> {
        let mut env = env0.clone();
        let mut out = Vec::new();
        for n in nodes {
            match n {
                Node::Declare(d) => {
                    let site = format!("let {}", d.let_.name);
                    let made = self.hold(m, n, &site, &env, |m| Ok((Vec::new(), Some(self.eval_in(m, &env, &d.let_.value)?))));
                    let (_, v) = match made {
                        Ok(made) => made,
                        Err(f) if self.contains(&env) => {
                            // what comes after may read it
                            out.push(self.failed("doc", &join(&env.path, &site), &join(env.key_or_path(), &site), &f));
                            break;
                        }
                        Err(f) => return Err(f),
                    };
                    env.bindings.push((Rc::from(d.let_.name.as_str()), Binding::Value(v.unwrap_or_else(Value::none))));
                }
                Node::Extern(_) | Node::Use(_) => out.extend(self.draw_node(m, n, &env)?),
                Node::For(ir::NodeFor { site, .. }) | Node::If(ir::NodeIf { site, .. }) | Node::Match(ir::NodeMatch { site, .. }) | Node::Place(ir::NodePlace { site, .. }) => {
                    match self.hold(m, n, site, &env, |m| Ok((self.draw_node(m, n, &env)?, None))) {
                        Ok(made) => out.extend(made.0),
                        Err(f) if self.contains(&env) => out.push(self.failed("doc", &join(&env.path, site), &join(env.key_or_path(), site), &f)),
                        Err(f) => return Err(f),
                    }
                }
            }
        }
        Ok(out)
    }

    /// Whether a failure here stays here: in the document's own code, which no widget is around to show it
    /// for. In a sentence it is the failure of what holds the sentence.
    fn contains(&self, env: &Env) -> bool {
        env.owner.is_none() && self.w.borrow().in_inputs == 0
    }

    /// What a loop, a branch, a place or a local gives: as it was, when it is the same one with the
    /// same locals around it and nothing it read has changed.
    fn hold(&self, m: &mut Machine, node: &Node, site: &str, env: &Env, work: impl FnOnce(&mut Machine) -> R<(Vec<Drawn>, Option<Value>)>) -> R<(Vec<Drawn>, Option<Value>)> {
        let Some(id) = self.s.node_id(node).or_else(|| env.keep.as_ref().map(|_| addr(node))) else { return work(m) };
        let key = format!("{}|{}", env.key_or_path(), site);
        let (hit, twice) = {
            let w = self.w.borrow();
            let memo = w.node_memo.get(&key);
            // the same content placed twice in one redraw is two
            let twice = w.node_seen.contains(&key) || memo.map_or(false, |x| x.frame.seen.get() == w.draws);
            let hit = if w.memo_on && !twice { memo.filter(|x| x.node == id && x.epoch == w.epoch && x.path == env.path && x.owner == env.owner && same_keep(&x.keep, &env.keep)).cloned() } else { None };
            (hit, twice)
        };
        if let Some(x) = hit {
            if self.unchanged(m, &x.frame) && self.same_bindings(m, &x.bindings, &env.bindings) {
                self.replay(&x.frame);
                return Ok((x.drawn.clone(), x.value.clone()));
            }
        }
        {
            let mut w = self.w();
            w.node_seen.insert(key.clone());
            w.enter();
        }
        let made = work(m);
        let frame = self.w().exit();
        let (drawn, value) = made?;
        let mut w = self.w();
        if w.memo_on && !frame.dirty && !twice {
            let epoch = w.epoch;
            w.node_memo.insert(key, Rc::new(NodeMemo { frame, node: id, keep: env.keep.clone(), path: env.path.clone(), owner: env.owner, bindings: env.bindings.clone(), epoch, drawn: drawn.clone(), value: value.clone() }));
        } else if !twice {
            w.node_memo.remove(&key);
        }
        Ok((drawn, value))
    }

    fn draw_node(&self, m: &mut Machine, n: &Node, env: &Env) -> R<Vec<Drawn>> {
        let mut out = Vec::new();
        {
            match n {
                Node::Declare(_) => {}
                Node::For(f) => {
                    let rows = self.loop_rows(m, f, env)?;
                    for (k, row) in rows.into_iter().enumerate() {
                        let key = row.key;
                        let mut bindings = env.bindings.clone();
                        bindings.extend(row.bindings);
                        let inner = Env { owner: env.owner, bindings, path: join(&env.path, &format!("{}[{}]", f.site, k)), key: Some(join(env.key_or_path(), &format!("{}[{}]", f.site, key))), keep: env.keep.clone() };
                        out.extend(self.draw_nodes(m, &f.body, &inner)?);
                    }
                }
                Node::If(i) => {
                    let (nodes, inner) = self.pick_branch(m, i, env)?;
                    out.extend(self.draw_nodes(m, nodes, &inner)?);
                }
                Node::Match(mt) => {
                    let v = self.eval_in(m, env, &mt.value)?;
                    let mut done = false;
                    for c in &mt.cases {
                        if let Some(binds) = self.matches(m, &v, &c.when)? {
                            let mut inner = env.clone();
                            inner.bindings.extend(binds);
                            out.extend(self.draw_nodes(m, &c.then, &inner)?);
                            done = true;
                            break;
                        }
                    }
                    if !done {
                        out.extend(self.draw_nodes(m, &mt.else_, env)?);
                    }
                }
                Node::Place(p) => {
                    let v = self.eval_in(m, env, &p.view)?;
                    if p.sentence {
                        check_sentence(&v)?;
                    }
                    out.extend(self.draw_view(m, &v, &join(&env.path, &p.site), &join(env.key_or_path(), &p.site))?);
                }
                Node::Extern(x) => out.extend(self.draw_extern(m, self.s.call_id(x).or_else(|| env.keep.as_ref().map(|_| addr(x))), &x.name, &x.site, Args::Exprs(&x.props), &x.slots, &x.events, &x.writes, &x.levels, env)?),
                Node::Use(u) => {
                    let path = join(&env.path, &u.site);
                    let key = join(env.key_or_path(), &u.site);
                    out.extend(self.draw_use(m, &u.name, &path, Args::Exprs(&u.args), &u.slots, &u.events, env, &u.levels, &key)?);
                }
            }
        }
        Ok(out)
    }

    /// A match's case against a value: the locals it binds, or `None` when it doesn't match.
    pub(crate) fn matches(&self, m: &mut Machine, v: &Value, patterns: &[ir::Pattern]) -> R<Option<Bindings>> {
        for p in patterns {
            match p {
                ir::Pattern::Case(pc) => match v {
                    Value::Case(c) if *c.name == *pc.name => {
                        let binds = pc.binds.iter().enumerate().map(|(k, b)| (Rc::from(b.as_str()), Binding::Value(c.values.get(k).map_or_else(Value::none, |x| x.1.clone())))).collect();
                        return Ok(Some(binds));
                    }
                    Value::Opt(Some(x)) if pc.name == "SOME" => return Ok(Some(pc.binds.iter().take(1).map(|b| (Rc::from(b.as_str()), Binding::Value((**x).clone()))).collect())),
                    Value::Opt(None) if pc.name == "NONE" => return Ok(Some(Vec::new())),
                    _ => {}
                },
                ir::Pattern::Num(n) => {
                    if let Value::Num(_) = v {
                        let x = Num::parse(n).ok_or_else(|| Failure::ir(format!("`{}` is not a number", n)))?;
                        if m.equal(v, &Value::Num(x))? {
                            return Ok(Some(Vec::new()));
                        }
                    }
                }
                ir::Pattern::Text(t) => {
                    if matches!(v, Value::Text(x) if **x == **t) {
                        return Ok(Some(Vec::new()));
                    }
                }
                ir::Pattern::Bool(b) => {
                    if matches!(v, Value::Bool(x) if x == b) {
                        return Ok(Some(Vec::new()));
                    }
                }
            }
        }
        Ok(None)
    }

    fn supplies(&self, widget: &str) -> bool {
        let s = self.w.borrow().supplies.clone();
        s.map_or(true, |f| f(widget))
    }

    /// An extern widget's call: reused from the last redraw when it is the same call at the same
    /// place, with the same locals around it, and nothing it read has changed.
    #[allow(clippy::too_many_arguments)]
    fn draw_extern(
        &self,
        m: &mut Machine,
        node: Option<usize>,
        name: &str,
        site: &str,
        given: Args,
        slots: &BTreeMap<String, Vec<Node>>,
        events: &BTreeMap<String, String>,
        writes: &BTreeMap<String, ir::Place>,
        levels: &[ir::Level],
        env: &Env,
    ) -> R<Vec<Drawn>> {
        self.draw_extern_as(m, node, name, join(env.key_or_path(), site), join(&env.path, site), given, slots, events, writes, levels, env)
    }

    /// The same, as who it is and at the path it is drawn.
    #[allow(clippy::too_many_arguments)]
    fn draw_extern_as(
        &self,
        m: &mut Machine,
        node: Option<usize>,
        name: &str,
        key: String,
        path: String,
        given: Args,
        slots: &BTreeMap<String, Vec<Node>>,
        events: &BTreeMap<String, String>,
        writes: &BTreeMap<String, ir::Place>,
        levels: &[ir::Level],
        env: &Env,
    ) -> R<Vec<Drawn>> {
        let key = self.claim(&key);
        let memo_on = self.w.borrow().memo_on;
        if let (true, Some(id)) = (memo_on, node) {
            let hit = {
                let w = self.w.borrow();
                w.extern_memo.get(&key).filter(|x| x.node == id && x.epoch == w.epoch && x.path == path && x.owner == env.owner && same_keep(&x.keep, &env.keep)).cloned()
            };
            if let Some(x) = hit {
                if self.unchanged(m, &x.frame) && self.same_bindings(m, &x.bindings, &env.bindings) {
                    self.replay(&x.frame);
                    return Ok(x.drawn.clone());
                }
            }
        }
        self.w().enter();
        let drawn = self.draw_extern_now(m, name, &key, &path, given, slots, events, writes, levels, env);
        let drawn = match drawn {
            Err(f) if self.contains(env) => Ok(vec![self.failed(name, &path, &key, &f)]),
            drawn => drawn,
        };
        let frame = self.w().exit();
        let drawn = drawn?;
        {
            let mut w = self.w();
            match node {
                Some(id) if w.memo_on && !frame.dirty => {
                    let epoch = w.epoch;
                    w.extern_memo.insert(key.clone(), Rc::new(ExternMemo { frame, node: id, keep: env.keep.clone(), path, owner: env.owner, bindings: env.bindings.clone(), epoch, drawn: drawn.clone() }));
                }
                _ => {
                    w.extern_memo.remove(&key);
                }
            }
        }
        Ok(drawn)
    }

    /// A reused part is inside the one being drawn, and its instances are seen again.
    fn replay(&self, frame: &Rc<Frame>) {
        let mut w = self.w();
        w.see(frame);
        if let Some(around) = w.frames.last_mut() {
            around.kids.push(frame.clone());
        }
    }

    /// Whether nothing a part read, or what is inside it, has changed since.
    fn unchanged(&self, m: &mut Machine, frame: &Frame) -> bool {
        let stamp = self.w.borrow().stamp;
        match frame.fresh.get() {
            STALE => return false,
            at if at == stamp && stamp > 0 => return true,
            _ => {}
        }
        let ok = self.w.borrow().current(&frame.deps)
            && frame.parts.borrow_mut().iter_mut().all(|p| self.part_stands(m, p))
            && frame.kids.iter().all(|k| self.unchanged(m, k));
        frame.fresh.set(if ok { stamp } else { STALE });
        ok
    }

    /// What is at a place now, read for no one.
    fn part_at(&self, m: &mut Machine, place: &Place) -> R<(Value, HashMap<String, u64>)> {
        let around = self.w().part_deps.replace(HashMap::new());
        let v = m.value_at(place);
        let deps = std::mem::replace(&mut self.w().part_deps, around).unwrap_or_default();
        v.map(|v| (v, deps))
    }

    fn part_stands(&self, m: &mut Machine, p: &mut Part) -> bool {
        if self.w.borrow().current(&p.deps) {
            return true;
        }
        match self.part_at(m, &p.place) {
            Ok((now, deps)) if same_now(&p.was, &now) => {
                p.deps = deps;
                true
            }
            // what was there is another, or its place is gone
            _ => false,
        }
    }

    /// Whether the locals around a call are the ones it was drawn with.
    fn same_bindings(&self, m: &mut Machine, a: &Bindings, b: &Bindings) -> bool {
        a.len() == b.len()
            && a.iter().zip(b).all(|((n, x), (o, y))| {
                n == o
                    && match (x, y) {
                        (Binding::Value(p), Binding::Value(q)) => same_ptr(p, q) || m.equal(p, q).unwrap_or(false),
                        (Binding::Ref(p), Binding::Ref(q)) => same_place(p, q),
                        _ => false,
                    }
            })
    }

    /// The widget's subtree from the last redraw, when it is called the same way at the same
    /// place and nothing it read has changed since: its instances, own values and missing widgets
    /// are seen again, and every widget around it is told what it read. A failure working out
    /// whether it is the same is a no: the widget is drawn, and fails there, as without reuse.
    #[allow(clippy::too_many_arguments)]
    fn reuse(&self, m: &mut Machine, name: &str, path: &str, key: &str, args: Args, slots: &BTreeMap<String, Vec<Node>>, events: &BTreeMap<String, String>, env: &Env) -> Option<Vec<Drawn>> {
        let memo = {
            let w = self.w.borrow();
            if !w.memo_on {
                return None;
            }
            let x = w.memo.get(key)?;
            if x.epoch != w.epoch || x.template != name || x.path != path || x.owner != env.owner {
                return None;
            }
            x.clone()
        };
        if memo.slots.len() != slots.len() || !memo.slots.iter().zip(slots).all(|((k, held), (n, now))| k == n && self.s.same_slot(held, now)) || memo.events != *events {
            return None;
        }
        if !self.unchanged(m, &memo.frame) || !self.same_bindings(m, &memo.bindings, &env.bindings) {
            return None;
        }
        // an input worked out here is the same while what it read and the locals around it are
        let now = arg_values(args);
        if now.len() != memo.args.len() {
            return None;
        }
        for (v, was) in now.iter().zip(&memo.args) {
            if !m.equal(v, was).unwrap_or(false) {
                return None;
            }
        }
        self.replay(&memo.frame);
        Some(memo.drawn.clone())
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_extern_now(
        &self,
        m: &mut Machine,
        name: &str,
        key: &str,
        path: &str,
        given: Args,
        slots: &BTreeMap<String, Vec<Node>>,
        events: &BTreeMap<String, String>,
        writes: &BTreeMap<String, ir::Place>,
        levels: &[ir::Level],
        env: &Env,
    ) -> R<Vec<Drawn>> {
        let (key, path) = (key.to_string(), path.to_string());
        let mut props = match given {
            Args::Values(v) => v.clone(),
            Args::Exprs(given) => {
                let mut props = BTreeMap::new();
                for (k, e) in given {
                    props.insert(k.clone(), self.eval_in(m, env, e)?);
                }
                props
            }
        };
        let mut written = BTreeMap::new();
        for (k, p) in writes {
            props.insert(k.clone(), m.eval_with(&env.bindings, env.owner, &Expr::Place(p.clone()))?);
            written.insert(k.clone(), WriteTo { place: p.clone(), env: env.clone() });
        }
        let w = self.s.extern_widgets.get(name).cloned();
        for f in w.iter().flat_map(|w| w.inputs.iter()) {
            if !props.contains_key(&f.name) {
                if let Some(d) = &f.default {
                    let v = self.eval_in(m, env, d)?;
                    props.insert(f.name.clone(), v);
                }
            }
        }
        // a widget the engine doesn't draw shows its Markdown instead (page 6), drawn with the
        // blocks the engine does: its markdown block, in an instance holding its inputs
        if let Some(w) = &w {
            if !self.supplies(name) {
                {
                    let mut w = self.w();
                    w.missed.insert(name.to_string());
                    if let Some(f) = w.frames.last_mut() {
                        f.missed.push(name.to_string());
                    }
                }
                let inst = self.enter_extern(m, w, &path, &props, slots, env)?;
                let out = self.draw_nodes(m, &w.markdown, &Env { path: path.clone(), key: Some(key.clone()), ..inst.clone() });
                self.leave(&inst);
                return out;
            }
        }
        // what it wrote to its own `mut` inputs stands until the caller gives them a variable
        let keeps = w.as_ref().map_or(false, |w| w.inputs.iter().any(|f| f.mutable));
        for (n, v) in self.back(m, &path, |n| if written.contains_key(n) { None } else { w.as_ref()?.inputs.iter().find(|f| f.name == n && f.mutable).map(|f| f.ty.clone()) })? {
            self.w().own.entry(key.clone()).or_default().insert(n, v);
        }
        if keeps {
            let mut world = self.w();
            world.track(|| format!("o:{}", key));
            if let Some(own) = world.own.get(&key).cloned() {
                world.own_seen.insert(key.clone());
                if let Some(f) = world.frames.last_mut() {
                    f.own.push(key.clone());
                }
                for (k, v) in own {
                    if !written.contains_key(&k) {
                        props.insert(k, v);
                    }
                }
            }
        }
        let mut drawn_slots = BTreeMap::new();
        for (s, nodes) in slots {
            drawn_slots.insert(s.clone(), self.draw_nodes(m, nodes, env)?);
        }
        if let Some(w) = w.as_ref().filter(|w| !w.requires.is_empty()) {
            let inst = self.enter_extern(m, w, &path, &props, slots, env)?;
            let held = self.check_rules(m, &w.name, &inst, &path);
            self.leave(&inst);
            held?;
        }
        let mut inside = Vec::new();
        self.w().in_inputs += 1;
        let drawn = self.draw_inputs(m, w.as_deref(), &mut props, &mut inside, &path, &key);
        self.w().in_inputs -= 1;
        drawn?;
        let events = events.iter().map(|(e, h)| (e.clone(), Handle { handler: h.clone(), env: env.clone() })).collect();
        Ok(vec![Drawn::new(DrawnNode { path, id: key, widget: name.to_string(), props, slots: drawn_slots, events, writes: written, levels: levels.to_vec(), failed: None, held: inside })])
    }

    /// Why a value from outside is refused, or None when it is one of the type.
    fn refusal(&self, m: &mut Machine, v: &Value, t: &ir::Type, site: &str) -> Option<Broken> {
        match m.takes(v, t) {
            Ok(true) => None,
            Ok(false) => Some(broken_code("contracts.outside", site)),
            Err(f) => Some(refused(&f, site)),
        }
    }

    /// Whether a call from outside gives its params values of their types: told when it doesn't.
    fn takes_args(&self, m: &mut Machine, params: &[ir::Param], args: &[Value], site: &str) -> bool {
        for (p, v) in params.iter().zip(args) {
            if let Some(broken) = self.refusal(m, v, &p.ty, site) {
                self.w().outcome.refused.push(broken);
                return false;
            }
        }
        true
    }

    /// A channel's value as the host gives it, or its default for one that isn't of its type. A channel's
    /// value this page doesn't read is no part of it.
    fn give_extern(&self, m: &mut Machine, channel: &str, name: &str, v: Option<Value>) -> R<()> {
        let s = self.s.clone();
        let channels = s.module.declarations.iter().filter_map(|d| match &d.declare {
            ir::Decl::Extern(ir::Extern::Channel(c)) if c.name == channel => Some(c),
            _ => None,
        });
        let Some(f) = channels.flat_map(|c| &c.values).find(|f| f.name == name) else { return Ok(()) };
        let site = format!("{}.{}", channel, name);
        let broken = v.as_ref().and_then(|v| self.refusal(m, v, &f.ty, &site));
        let v = match (v, &broken, &f.default) {
            (Some(v), None, _) => Some(v),
            (_, _, Some(x)) => Some(Machine::new(&s.program, Box::new(Constants)).eval(x)?),
            _ => None,
        };
        let mut w = self.w();
        w.outcome.refused.extend(broken);
        if let Some(v) = v {
            w.externs.insert(site, v);
        }
        Ok(())
    }

    /// What is put back at a place, as it is drawn: each variable that is there and whose value fits it.
    fn back(&self, m: &mut Machine, path: &str, ty: impl Fn(&str) -> Option<ir::Type>) -> R<Vec<(String, Value)>> {
        let Some(vars) = self.w.borrow().put_back.get(path).cloned() else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for (name, x) in vars {
            let Some(t) = ty(&name) else { continue };
            let Ok(v) = Machine::new(&self.s.program, Box::new(Constants)).eval(&x) else { continue };
            if self.refusal(m, &v, &t, path).is_some() {
                continue;
            }
            if let Some(vars) = self.w().put_back.get_mut(path) {
                vars.remove(&name);
            }
            out.push((name, v));
        }
        Ok(out)
    }

    /// The calls an input holds, drawn: a rich_text's and a view's, wherever in its value they are.
    fn draw_inputs(&self, m: &mut Machine, w: Option<&ir::ExternWidget>, props: &mut BTreeMap<String, Value>, held: &mut Vec<Vec<Drawn>>, path: &str, key: &str) -> R<()> {
        let piece = w.and_then(|w| w.piece.as_deref()).unwrap_or("");
        for (k, v) in props.iter_mut() {
            if takes_sentence(piece, k) {
                check_sentence(v)?;
            }
            if let Some(x) = self.draw_input(m, v, held, &join(path, k), &join(key, k))? {
                *v = x;
            }
        }
        Ok(())
    }

    /// `v` with the calls it holds drawn, or None when it holds none.
    fn draw_input(&self, m: &mut Machine, v: &Value, held: &mut Vec<Vec<Drawn>>, path: &str, key: &str) -> R<Option<Value>> {
        Ok(match v {
            Value::Content(_) | Value::View(_) => {
                let drawn = self.draw_view(m, v, path, key)?;
                held.push(drawn);
                Some(Value::Content(Rc::new(Content { of: ContentOf::Drawn(held.len() - 1), blocks: false })))
            }
            Value::List(l) => {
                let mut items = None;
                for (k, i) in l.items.iter().enumerate() {
                    if let Some(x) = self.draw_input(m, &i.value, held, &format!("{}[{}]", path, k), &format!("{}[{}]", key, k))? {
                        items.get_or_insert_with(|| l.items.clone())[k].value = x;
                    }
                }
                items.map(|items| Value::List(Rc::new(List { items })))
            }
            Value::Record(r) => {
                let mut fields = None;
                for (k, (f, x)) in r.fields.iter().enumerate() {
                    if let Some(x) = self.draw_input(m, x, held, &format!("{}.{}", path, f), &format!("{}.{}", key, f))? {
                        fields.get_or_insert_with(|| r.fields.clone())[k].1 = x;
                    }
                }
                fields.map(|fields| Value::Record(Rc::new(Record { ty: r.ty.clone(), fields })))
            }
            Value::Case(c) => {
                let mut values = None;
                for (k, (f, x)) in c.values.iter().enumerate() {
                    if let Some(x) = self.draw_input(m, x, held, &format!("{}.{}", path, f), &format!("{}.{}", key, f))? {
                        values.get_or_insert_with(|| c.values.clone())[k].1 = x;
                    }
                }
                values.map(|values| Value::Case(Rc::new(Case { ty: c.ty.clone(), name: c.name.clone(), values })))
            }
            Value::Opt(Some(x)) => self.draw_input(m, x, held, path, key)?.map(Value::some),
            _ => None,
        })
    }

    /// Who a widget is, this redraw: the place it was written, or that and which time it is placed.
    fn claim(&self, key: &str) -> String {
        let mut w = self.w();
        let mut id = key.to_string();
        let mut n = 2;
        while w.taken(&id) {
            id = format!("{}~{}", key, n);
            n += 1;
        }
        w.claimed.insert(id.clone());
        id
    }

    /// A widget written in Extended Mark: its instance, kept by its key, and its view.
    #[allow(clippy::too_many_arguments)]
    fn draw_use(
        &self,
        m: &mut Machine,
        name: &str,
        path: &str,
        args: Args,
        slots: &BTreeMap<String, Vec<Node>>,
        events: &BTreeMap<String, String>,
        env: &Env,
        levels: &[ir::Level],
        key: &str,
    ) -> R<Vec<Drawn>> {
        let Some(t) = self.s.templates.get(name).cloned() else {
            // an extern widget held as a view value
            return self.draw_extern_as(m, None, name, key.to_string(), path.to_string(), args, slots, events, &BTreeMap::new(), levels, env);
        };
        let claimed = self.claim(key);
        let key = claimed.as_str();
        if let Some(reused) = self.reuse(m, name, path, key, args, slots, events, env) {
            return Ok(reused);
        }
        self.w().enter();
        let drawn = match self.enter_use(m, &t, path, args, slots, events, env, key).and_then(|own| self.draw_nodes(m, &t.view, &own)) {
            Ok(out) => out,
            Err(f) => vec![self.failed(name, path, key, &f)],
        };
        let frame = self.w().exit();
        let (memo_on, kept) = {
            let w = self.w.borrow();
            (w.memo_on, w.by_path.contains_key(key))
        };
        match memo_on && !frame.dirty && kept {
            true => {
                let args = arg_values(args);
                let slots = slots.iter().map(|(k, v)| (k.clone(), self.s.nodes_rc(v))).collect();
                let mut w = self.w();
                let epoch = w.epoch;
                w.memo.insert(key.to_string(), Rc::new(Memo { frame, template: name.to_string(), path: path.to_string(), owner: env.owner, args, bindings: env.bindings.clone(), slots, events: events.clone(), epoch, drawn: drawn.clone() }));
            }
            false => {
                self.w().memo.remove(key);
            }
        }
        Ok(drawn)
    }

    /// A widget that failed: reported once, and in its place (page 6).
    fn failed(&self, name: &str, path: &str, key: &str, f: &Failure) -> Drawn {
        let broken = match (&f.kind, &f.rule) {
            (FailKind::Rule, Some(rule)) => Broken { code: None, rule: Some(rule.clone()), site: Some(key.to_string()) },
            _ => broken_code(&failure_code(f), key),
        };
        {
            let mut w = self.w();
            w.soil();
            if !w.outcome.failures.iter().any(|b| b.site.as_deref() == Some(key) && b.code == broken.code && b.rule == broken.rule) {
                w.outcome.failures.push(broken);
            }
        }
        Drawn::new(DrawnNode { path: path.to_string(), id: key.to_string(), widget: name.to_string(), props: BTreeMap::new(), slots: BTreeMap::new(), events: BTreeMap::new(), writes: BTreeMap::new(), levels: Vec::new(), failed: Some(f.message.clone()), held: Vec::new() })
    }

    /// A widget's instance at a key, made or kept, with its inputs, slots and events from this
    /// call: the env its own code runs in. Its rules have to hold.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn enter_use(
        &self,
        m: &mut Machine,
        t: &Rc<ir::Template>,
        path: &str,
        args: Args,
        slots: &BTreeMap<String, Vec<Node>>,
        events: &BTreeMap<String, String>,
        env: &Env,
        key: &str,
    ) -> R<Env> {
        let kept = {
            let w = self.w.borrow();
            w.by_path.get(key).and_then(|id| w.instances.get(id)).filter(|i| i.template.name == t.name).map(|i| i.id)
        };
        let fresh = kept.is_none();
        let id = match kept {
            Some(id) => id,
            None => {
                let id = m.new_id();
                let inst = Instance {
                    id,
                    path: key.to_string(),
                    template: t.clone(),
                    inputs: HashMap::new(),
                    places: HashMap::new(),
                    cells: HashMap::new(),
                    slots: HashMap::new(),
                    events: HashMap::new(),
                    seen: 0,
                };
                let mut w = self.w();
                w.instances.insert(id, inst);
                w.by_path.insert(key.to_string(), id);
                id
            }
        };
        let was = {
            let mut w = self.w();
            w.at_path.insert(path.to_string(), id);
            if let Some(f) = w.frames.last_mut() {
                f.nested.push((id, path.to_string()));
            }
            let draws = w.draws;
            let it = w.instances.get_mut(&id).unwrap();
            it.seen = draws;
            it.places.clear();
            std::mem::take(&mut it.inputs)
        };
        if let Args::Values(values) = args {
            let values: Vec<(String, Value)> = values.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            self.with_instance(id, |i| i.inputs.extend(values));
        }
        for (k, e) in if let Args::Exprs(args) = args { args.iter().collect() } else { Vec::new() } {
            match e {
                Expr::Place(p) => {
                    self.with_instance(id, |i| i.places.insert(k.clone(), WriteTo { place: p.clone(), env: env.clone() }));
                }
                _ => {
                    let v = self.eval_in(m, env, e)?;
                    self.with_instance(id, |i| i.inputs.insert(k.clone(), v));
                }
            }
        }
        let own = Env { owner: Some(id), bindings: Vec::new(), path: path.to_string(), key: Some(key.to_string()), keep: None };
        for f in &t.inputs {
            let given = self.with_instance(id, |i| i.inputs.contains_key(&f.name) || i.places.contains_key(&f.name)).unwrap_or(true);
            if !given {
                let v = match &f.default {
                    Some(d) => self.eval_in(m, &own, d)?,
                    None => Value::none(),
                };
                self.with_instance(id, |i| i.inputs.insert(f.name.clone(), v));
            }
        }
        // an input given a new value: whatever read it, in the widget's slots too, is out of date
        if !fresh {
            let now = self.with_instance(id, |i| i.inputs.clone()).unwrap_or_default();
            for (k, v) in &now {
                let same = was.get(k).map_or(false, |b| m.equal(b, v).unwrap_or(false));
                if !same {
                    self.w().bump(format!("i:{}:{}", id, k));
                }
            }
        }
        let held: HashMap<String, (Nodes, Env)> = slots.iter().map(|(s, nodes)| (s.clone(), (self.s.nodes_rc(nodes), env.clone()))).collect();
        let handles: HashMap<String, Handle> = events.iter().map(|(e, h)| (e.clone(), Handle { handler: h.clone(), env: env.clone() })).collect();
        // a slot given new content, or the same content with other values around it, is an input given
        // a new value: whatever read it is out of date
        if !fresh {
            let before = self.with_instance(id, |i| i.slots.clone()).unwrap_or_default();
            let names: std::collections::BTreeSet<&String> = before.keys().chain(held.keys()).collect();
            for s in names {
                let same = match (before.get(s), held.get(s)) {
                    (Some((a, x)), Some((b, y))) => Arc::ptr_eq(a, b) && x.owner == y.owner && x.path == y.path && self.same_bindings(m, &x.bindings, &y.bindings),
                    _ => false,
                };
                if !same {
                    self.w().bump(format!("l:{}:{}", id, s));
                }
            }
        }
        self.with_instance(id, |i| {
            i.slots = held;
            i.events = handles;
        });
        if fresh {
            for c in &t.cells {
                let v = match &c.default {
                    Some(d) => self.eval_in(m, &own, d)?,
                    None => Value::none(),
                };
                self.with_instance(id, |i| i.cells.insert(c.name.clone(), v));
            }
        }
        for (n, v) in self.back(m, path, |n| t.cells.iter().find(|c| c.name == n).map(|c| c.ty.clone()))? {
            self.with_instance(id, |i| i.cells.insert(n, v));
        }
        self.check_rules(m, &t.name, &own, path)?;
        Ok(own)
    }

    /// A widget's rules, with everything they read: a `require` has to hold, and a `warn` is told.
    fn check_rules(&self, m: &mut Machine, name: &str, own: &Env, site: &str) -> R<()> {
        let Some(rules) = self.s.template_rules.get(name).cloned() else { return Ok(()) };
        for (body, message, warn) in rules.iter() {
            let holds = !matches!(self.run_body(m, &[], body.clone(), own, Vec::new())?, Some(Value::Bool(false)));
            if *warn {
                self.w().warned(site, message, holds);
            } else if !holds {
                return Err(Failure { kind: FailKind::Rule, message: message.clone(), rule: Some(message.clone()) });
            }
        }
        Ok(())
    }

    fn with_instance<T>(&self, id: u64, f: impl FnOnce(&mut Instance) -> T) -> Option<T> {
        self.w().instances.get_mut(&id).map(f)
    }

    /// An extern widget's instance, for its markdown block: its inputs' values and its slots.
    pub(crate) fn enter_extern(&self, m: &mut Machine, w: &ir::ExternWidget, path: &str, props: &BTreeMap<String, Value>, slots: &BTreeMap<String, Vec<Node>>, env: &Env) -> R<Env> {
        let t = self.s.extern_templates.get(&w.name).cloned().ok_or_else(|| Failure::ir(format!("no extern widget {}", w.name)))?;
        let id = m.new_id();
        let mut inputs: HashMap<String, Value> = props.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for f in &w.inputs {
            if !inputs.contains_key(&f.name) {
                let v = match &f.default {
                    Some(d) => self.eval_in(m, env, d)?,
                    None => Value::none(),
                };
                inputs.insert(f.name.clone(), v);
            }
        }
        let held = slots.iter().map(|(s, nodes)| (s.clone(), (self.s.nodes_rc(nodes), env.clone()))).collect();
        let inst = Instance { id, path: format!("{}#md", path), template: t, inputs, places: HashMap::new(), cells: HashMap::new(), slots: held, events: HashMap::new(), seen: self.w.borrow().draws };
        self.w().instances.insert(id, inst);
        Ok(Env { owner: Some(id), bindings: Vec::new(), path: path.to_string(), key: None, keep: None })
    }

    /// Drops an instance made only to write Markdown.
    pub(crate) fn leave(&self, env: &Env) {
        let mut w = self.w();
        if let Some(id) = env.owner {
            if w.instances.get(&id).map_or(false, |i| i.path.ends_with("#md")) {
                w.instances.remove(&id);
            }
        }
    }

    /// A view held in a value: a call made as one, or calls.
    pub(crate) fn draw_view(&self, m: &mut Machine, v: &Value, path: &str, key: &str) -> R<Vec<Drawn>> {
        match v {
            Value::Opt(Some(x)) => self.draw_view(m, x, path, key),
            Value::List(l) => {
                let mut out = Vec::new();
                for (k, i) in l.items.iter().enumerate() {
                    out.extend(self.draw_view(m, &i.value, &format!("{}[{}]", path, k), &format!("{}[{}]", key, i.id))?);
                }
                Ok(out)
            }
            Value::Content(c) => match &c.of {
                ContentOf::Nodes { nodes, captured, owner, at } => {
                    let (path, key) = match at {
                        Some((p, k)) => (p.to_string(), k.to_string()),
                        None => (path.to_string(), key.to_string()),
                    };
                    self.draw_nodes(m, nodes, &Env { owner: *owner, bindings: captured.clone(), path, key: Some(key), keep: Some(nodes.clone()) })
                }
                ContentOf::Parts(parts) => {
                    let mut out = Vec::new();
                    for (k, p) in parts.iter().enumerate() {
                        let site = format!("j{}", k);
                        out.extend(self.draw_view(m, p, &join(path, &site), &join(key, &site))?);
                    }
                    Ok(out)
                }
                ContentOf::Drawn(_) => Ok(Vec::new()),
            },
            Value::View(view) => {
                let who = view.at.as_deref().unwrap_or(key);
                let made = Env { owner: view.owner, bindings: view.captured.clone(), path: path.to_string(), key: Some(who.to_string()), keep: None };
                self.draw_use(m, &view.widget, path, Args::Values(&view.args), &view.slots, &view.events, &made, &[], who)
            }
            _ => Ok(Vec::new()),
        }
    }
}

/// Whether two calls are kept by the same content.
fn same_keep(a: &Option<Arc<Vec<Node>>>, b: &Option<Arc<Vec<Node>>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}

/// What a transaction undoes.
struct Snapshot {
    state: Vec<(String, Value)>,
    cells: Vec<(u64, HashMap<String, Value>)>,
    calls: usize,
}

/// One write a layout makes: a `mut` input of a drawn widget.
#[derive(Clone, Debug)]
pub struct LayoutWrite {
    pub id: String,
    pub input: String,
    pub value: Value,
}

/// The page: mount it, read its tree, its `$state` and its Markdown, and change it.
pub struct Page {
    rt: Rt,
    drawn: Vec<Drawn>,
    /// How many changes came from outside the layout: the reader's, the host's, the page's
    /// values. A layout that didn't settle is held until the next one.
    changes: u64,
    /// How many times the page was drawn: an engine that draws every frame redraws its own
    /// widgets when this moves.
    revision: u64,
    listeners: Vec<(usize, Box<dyn FnMut()>)>,
    next_listener: usize,
    /// The planes already laid out once, by identity: `fit` only sets where they start.
    pub(crate) started: HashSet<String>,
    /// The outside change a layout that didn't settle is held at.
    pub(crate) held_layout: Option<u64>,
    /// The fns a host calls.
    held_fns: crate::host::HeldFns,
    /// What of the data it started from didn't go back.
    left: Option<Vec<String>>,
}

/// The IR this runtime runs: a whole number of its own, apart from its release.
pub const IR_VERSION: i64 = 4;

/// An IR of another version is refused, and told what to do.
fn check_version(version: i64) -> R<()> {
    if version == IR_VERSION {
        return Ok(());
    }
    Err(Failure::ir(format!("this page is IR {}, this engine reads IR {}: {}", version, IR_VERSION, if version > IR_VERSION { "use a later engine" } else { "run xtmd migrate" })))
}

impl Page {
    /// A page from the IR file `xtmd build` writes, as JSON.
    pub fn from_json(json: &str, opts: PageOptions) -> R<Page> {
        // its version first: another's may not read
        if let Some(v) = serde_json::from_str::<serde_json::Value>(json).ok().and_then(|v| v["ir"]["version"].as_i64()) {
            check_version(v)?;
        }
        let file: ir::File = serde_json::from_str(json).map_err(|e| Failure::ir(format!("the IR doesn't read: {}", e)))?;
        Page::new(file, opts)
    }

    /// A page from an IR file: mounted, and drawn once.
    pub fn new(file: ir::File, opts: PageOptions) -> R<Page> {
        Page::from_module(file.ir, opts)
    }

    pub fn from_module(module: ir::Module, opts: PageOptions) -> R<Page> {
        check_version(module.version)?;
        let mut program = Program::from_module(&module);
        let mut templates = HashMap::new();
        let mut template_rules = HashMap::new();
        let mut nodes = Stable::default();
        for t in &module.templates {
            for f in &t.fns {
                program.fns.insert(f.name.clone(), Rc::new(f.clone()));
            }
            let t = Rc::new(t.clone());
            nodes.link(&t.view, &t.view);
            if let Some(md) = &t.markdown {
                nodes.link(md, md);
            }
            template_rules.insert(t.name.clone(), rules_of(t.requires.iter()));
            templates.insert(t.name.clone(), t);
        }
        let handlers = module
            .handlers
            .iter()
            .map(|h| {
                let passes = match h.body.as_slice() {
                    [Stmt::Invoke(i)] => Some(i.event.clone()),
                    _ => None,
                };
                (h.id.clone(), HandlerFn { params: h.params.clone(), body: Rc::new(h.body.clone()), passes })
            })
            .collect();
        let host_fns = module.needs.host.iter().cloned().collect();
        let mut state_decl = None;
        let mut extern_widgets = HashMap::new();
        let mut extern_templates = HashMap::new();
        let mut lets = HashMap::new();
        for d in &module.declarations {
            match &d.declare {
                ir::Decl::State(s) => state_decl = Some(s.clone()),
                ir::Decl::Extern(ir::Extern::Widget(w)) => {
                    let w = Rc::new(w.clone());
                    nodes.link(&w.markdown, &w.markdown);
                    if !w.requires.is_empty() {
                        template_rules.insert(w.name.clone(), rules_of(w.requires.iter()));
                    }
                    extern_templates.insert(
                        w.name.clone(),
                        Rc::new(ir::Template {
                            name: w.name.clone(),
                            generics: Vec::new(),
                            inputs: w.inputs.clone(),
                            slots: w.slots.clone(),
                            events: w.events.clone(),
                            cells: Vec::new(),
                            requires: Vec::new(),
                            fns: Vec::new(),
                            view: Vec::new(),
                            markdown: Some(Vec::new()),
                            levels: w.levels.clone(),
                        }),
                    );
                    extern_widgets.insert(w.name.clone(), w);
                }
                ir::Decl::Let(l) => {
                    lets.entry(l.name.clone()).or_insert_with(|| l.value.clone());
                }
                _ => {}
            }
        }
        // core's fns are the engine's: one it doesn't have is another core's
        let supplied = |n: &str| opts.fns.contains_key(n) || xtmd_eval::natives::exists(n) || xtmd_eval::natives::ENGINE_ONLY.contains(&n) || program.externs.get(n).map_or(false, |x| !x.default.is_empty());
        if let Some(n) = module.needs.core.iter().find(|n| !extern_widgets.contains_key(*n) && !supplied(n)) {
            return Err(Failure::ir(format!("this page needs `{}`, which this engine doesn't have", n)));
        }
        let state_rules = rules_of(state_decl.iter().flat_map(|s: &ir::State| s.requires.iter()));
        let shared = Shared {
            program,
            templates,
            template_rules,
            extern_widgets,
            extern_templates,
            handlers,
            host_fns,
            state_decl,
            state_rules,
            lets,
            engine_fns: opts.fns.clone(),
            stable: nodes,
            module,
        };
        let world = World { supplies: opts.supplies.clone(), memo_on: opts.memo, ..World::default() };
        let mut page = Page {
            rt: Rt { s: Rc::new(shared), w: Rc::new(RefCell::new(world)) },
            drawn: Vec::new(),
            changes: 0,
            revision: 0,
            listeners: Vec::new(),
            next_listener: 0,
            started: HashSet::new(),
            held_layout: None,
            held_fns: Default::default(),
            left: None,
        };
        // the page's values, then $state's starting values, then what the host gives
        let s = page.rt.s.clone();
        for d in &s.module.declarations {
            if let ir::Decl::Extern(ir::Extern::Channel(c)) = &d.declare {
                for f in &c.values {
                    if let Some(x) = &f.default {
                        let v = page.constant(x)?;
                        page.rt.w().externs.insert(format!("{}.{}", c.name, f.name), v);
                    }
                }
            }
        }
        for f in s.state_decl.iter().flat_map(|s| s.fields.iter()) {
            if let Some(x) = &f.default {
                let v = page.constant(x)?;
                page.rt.w().state_set(&f.name, v);
            }
        }
        for (c, values) in &opts.externs {
            for (k, x) in values {
                let v = page.given(x, &format!("{}.{}", c, k));
                page.with_machine(|rt, m| rt.give_extern(m, c, k, v))?;
            }
        }
        page.apply_page(opts.page);
        page.start_from(&opts.state)?;
        match &opts.data {
            Some(d) if page.take_state(&d.state)? => page.left = Some(page.draw_with(&d.own)?),
            _ => page.redraw()?,
        }
        Ok(page)
    }

    /// `$state` as the host gives it, before the page is drawn: checked as a host's write is, all
    /// together, or else each field that fits and keeps the rules, one at a time. What doesn't keeps its
    /// starting value.
    fn start_from(&mut self, given: &[(String, Expr)]) -> R<()> {
        let mut values = Vec::new();
        for (k, x) in given {
            if let Some(v) = self.given(x, &format!("$state.{}", k)) {
                values.push((k.clone(), v));
            }
        }
        let told = self.rt.w.borrow().outcome.refused.len();
        if values.is_empty() || self.write_state(values.clone())? {
            return Ok(());
        }
        self.rt.w().outcome.refused.truncate(told);
        for (k, v) in values {
            self.write_state(vec![(k, v)])?;
        }
        Ok(())
    }

    /// Runs `f` with a machine on the page's program, its host the page: one per change, its
    /// step count and identities carried from the last.
    pub(crate) fn with_machine<T>(&self, f: impl FnOnce(&Rt, &mut Machine) -> T) -> T {
        let rt = self.rt.clone();
        let s = rt.s.clone();
        let mut m = Machine::new(&s.program, Box::new(PageHost(rt.clone())));
        {
            let w = rt.w.borrow();
            m.steps = w.steps;
            m.next_id = w.next_id;
        }
        let out = f(&rt, &mut m);
        let mut w = rt.w.borrow_mut();
        w.steps = m.steps;
        w.worked += m.worked;
        w.next_id = m.next_id;
        w.records_warned(std::mem::take(&mut m.warned));
        out
    }

    fn reset_steps(&self) {
        self.rt.w().steps = 0;
    }


    // ---- reading the page

    /// The drawn extern widgets, in order, with their props worked out.
    pub fn tree(&self) -> &[Drawn] {
        &self.drawn
    }

    /// How many times the page has been drawn. An engine that draws every frame keeps the
    /// revision it last drew and rebuilds what it holds when this moves.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// How many changes came from outside the layout.
    pub fn changes(&self) -> u64 {
        self.changes
    }

    /// Every drawn extern widget by its path, as a check observes it.
    pub fn observe_tree(&self) -> BTreeMap<String, Mounted> {
        fn walk(nodes: &[Drawn], out: &mut BTreeMap<String, Mounted>) {
            for n in nodes {
                let props = n.props.iter().filter(|(_, v)| !matches!(v, Value::Fn(_))).map(|(k, v)| (k.clone(), v.to_ir())).collect();
                let slots = n.slots.iter().map(|(s, kids)| (s.clone(), kids.iter().map(|k| k.path.clone()).collect())).collect();
                out.insert(n.path.clone(), Mounted { widget: n.widget.clone(), props, slots });
                for kids in n.slots.values().chain(n.held.iter()) {
                    walk(kids, out);
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(&self.drawn, &mut out);
        out
    }

    /// Every `shade` drawn, by path: the fn it was given, by name, and its arguments (plan tab
    /// 16, `shaded`). With `samples`, also the colour at the area's two corners and its middle,
    /// worked out by the shader itself, as the test engine does.
    pub fn observe_shaded(&self, samples: bool) -> R<BTreeMap<String, Shaded>> {
        fn shades<'a>(nodes: &'a [Drawn], out: &mut Vec<&'a Drawn>) {
            for n in nodes {
                if n.widget == "core/shade" {
                    out.push(n);
                }
                for kids in n.slots.values().chain(n.held.iter()) {
                    shades(kids, out);
                }
            }
        }
        let mut found = Vec::new();
        shades(&self.drawn, &mut found);
        let mut out = BTreeMap::new();
        for n in found {
            let f = n.props.get("shader");
            let args = n.props.get("args").cloned().unwrap_or_else(Value::none);
            let mut entry = Shaded { shader: f.map_or_else(String::new, shader_name), args: args.to_ir(), samples: None };
            let corners = match n.props.get("area") {
                Some(Value::Record(area)) => {
                    let coord = |p: &str, k: &str| match area.get(p) {
                        Some(Value::Record(pt)) => pt.get(k).and_then(|v| match v {
                            Value::Num(n) => Some(*n),
                            _ => None,
                        }),
                        _ => None,
                    };
                    match (coord("from", "x"), coord("from", "y"), coord("to", "x"), coord("to", "y")) {
                        (Some(fx), Some(fy), Some(tx), Some(ty)) => Some((fx, fy, tx, ty)),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let (true, Some(f), Some((fx, fy, tx, ty))) = (samples, f, corners) {
                let half = Num::parse("0.5").unwrap();
                let point = |x: Num, y: Num| crate::layout::pt_value(&crate::layout::Pt { x, y });
                let px = Value::Num(tx.sub(fx)?.abs().div(Num::int(64))?);
                let ats = [point(fx, fy), point(fx.add(tx)?.mul(half)?, fy.add(ty)?.mul(half)?), point(tx, ty)];
                let mut taken = Vec::new();
                for at in ats {
                    self.reset_steps();
                    let pixel = self.with_machine(|_, m| m.apply(f, vec![Binding::Value(at.clone()), Binding::Value(px.clone()), Binding::Value(args.clone())]))?;
                    taken.push((at.to_ir(), pixel.map_or(Expr::None(ir::Empty {}), |p| p.to_ir())));
                }
                entry.samples = Some(taken);
            }
            out.insert(n.path.clone(), entry);
        }
        Ok(out)
    }

    /// `$state` as it is now, in the IR's form.
    pub fn observe_state(&self) -> BTreeMap<String, Expr> {
        self.rt.w.borrow().state.iter().map(|(k, v)| (k.clone(), v.to_ir())).collect()
    }

    /// Every widget's own variables, by the place it is drawn at: an Extended Mark widget's `mut` variables,
    /// and an extern widget's own values for `mut` inputs its caller gave no variable. With
    /// `$state`, what a snapshot holds.
    pub fn observe_own(&self) -> BTreeMap<String, BTreeMap<String, Expr>> {
        self.own_values().into_iter().map(|(p, vs)| (p, vs.into_iter().map(|(k, v)| (k, v.to_ir())).collect())).collect()
    }

    pub(crate) fn own_values(&self) -> BTreeMap<String, BTreeMap<String, Value>> {
        let w = self.rt.w.borrow();
        let mut out: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
        for (path, id) in &w.at_path {
            if let Some(i) = w.instances.get(id).filter(|i| !i.cells.is_empty()) {
                out.insert(path.clone(), i.cells.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
            }
        }
        fn walk(nodes: &[Drawn], w: &World, out: &mut BTreeMap<String, BTreeMap<String, Value>>) {
            for n in nodes {
                if let Some(own) = w.own.get(&n.id) {
                    for (k, v) in own.iter().filter(|(k, _)| !n.writes.contains_key(*k)) {
                        out.entry(n.path.clone()).or_default().insert(k.clone(), v.clone());
                    }
                }
                for kids in n.slots.values().chain(n.held.iter()) {
                    walk(kids, w, out);
                }
            }
        }
        walk(&self.drawn, &w, &mut out);
        out
    }

    /// Widgets' own variables put back by place, as one change: each value checked against its
    /// variable's type. One that doesn't fit, or whose widget isn't drawn at that place, is left
    /// out, and returned as `place.name`.
    pub fn restore_own(&mut self, values: &BTreeMap<String, BTreeMap<String, Expr>>) -> R<Vec<String>> {
        self.changes += 1;
        self.reset_steps();
        self.draw_with(values)
    }

    /// The page's data put back, as one change: `$state` as one checked write, then widgets' own
    /// variables as [`Page::restore_own`] puts them back. `None`, and nothing is, when `$state` is refused.
    pub fn restore(&mut self, data: &PageData) -> R<Option<Vec<String>>> {
        self.changes += 1;
        self.reset_steps();
        if !self.take_state(&data.state)? {
            return Ok(None);
        }
        Ok(Some(self.draw_with(&data.own)?))
    }

    /// What of the data it started from didn't go back, as `place.name`; `None` when it started from none,
    /// or its `$state` was refused.
    pub fn left(&self) -> Option<&[String]> {
        self.left.as_deref()
    }

    /// `$state` written from the IR's form, whole or not at all.
    fn take_state(&mut self, given: &[(String, Expr)]) -> R<bool> {
        let mut values = Vec::new();
        for (k, x) in given {
            let Some(v) = self.given(x, &format!("$state.{}", k)) else { return Ok(false) };
            values.push((k.clone(), v));
        }
        self.write_state(values)
    }

    /// The page drawn with widgets' own variables at their places: what had none, as `place.name`.
    fn draw_with(&mut self, own: &BTreeMap<String, BTreeMap<String, Expr>>) -> R<Vec<String>> {
        {
            let mut w = self.rt.w();
            w.put_back = own.clone();
            // every memo read the old values
            w.epoch += 1;
        }
        let drawn = self.redraw();
        let back = std::mem::take(&mut self.rt.w().put_back);
        drawn?;
        Ok(back.iter().flat_map(|(path, vars)| vars.keys().map(move |n| format!("{}.{}", path, n))).collect())
    }

    /// One of `$state`'s values.
    pub fn state_value(&self, name: &str) -> Option<Value> {
        self.rt.w.borrow().state_get(name)
    }

    /// The extern fns of the host called since they were last taken.
    pub fn take_calls(&mut self) -> Vec<Call> {
        std::mem::take(&mut self.rt.w().calls)
    }

    /// Refused writes and failures since they were last taken.
    pub fn take_outcome(&mut self) -> Outcome {
        std::mem::take(&mut self.rt.w().outcome)
    }

    /// Every failure caught since they were last taken, with its message, for a host's log.
    pub fn take_errors(&mut self) -> Vec<Failure> {
        std::mem::take(&mut self.rt.w().errors)
    }

    /// The extern widgets this page uses that the engine doesn't supply, each shown as its
    /// Markdown: reported when the page is mounted, and kept up to date as it changes.
    pub fn missing(&self) -> Vec<String> {
        self.rt.w.borrow().missed.iter().cloned().collect()
    }

    /// What the page takes (its IR's `needs`) and nobody gives: the engine's fns and a pack's, and the
    /// host's values. The page runs without them until it reads one.
    pub fn lacking(&self) -> Vec<String> {
        let s = &self.rt.s;
        let needs = &s.module.needs;
        let fns = needs.core.iter().chain(needs.packs.iter().flat_map(|p| &p.externs)).filter(|n| match s.program.externs.get(*n) {
            Some(x) => !s.engine_fns.contains_key(*n) && !xtmd_eval::natives::exists(n) && x.default.is_empty() && !s.host_fns.contains(*n),
            None => false,
        });
        let w = self.rt.w.borrow();
        let values = needs.host.iter().filter(|n| !s.program.externs.contains_key(*n) && !w.externs.contains_key(*n));
        let mut out: Vec<String> = fns.chain(values).cloned().collect();
        out.sort();
        out
    }

    /// Called after every redraw, with nothing: the listener reads the page again. Returns an
    /// id for [`Page::off_change`].
    pub fn on_change(&mut self, listener: impl FnMut() + 'static) -> usize {
        self.next_listener += 1;
        self.listeners.push((self.next_listener, Box::new(listener)));
        self.next_listener
    }

    pub fn off_change(&mut self, id: usize) {
        self.listeners.retain(|(k, _)| *k != id);
    }

    // ---- changing it: every change is one transaction, checked, and undone when it fails

    /// A reader's event on a drawn widget, by its id.
    pub fn event(&mut self, id: &str, name: &str, args: Vec<Value>) -> R<()> {
        let Some(node) = self.find(id) else { return Ok(()) };
        let Some(ev) = node.events.get(name).cloned() else { return Ok(()) };
        let params = self.rt.s.extern_widgets.get(&node.widget).and_then(|w| w.events.iter().find(|e| e.name == name)).map(|e| e.params.clone()).unwrap_or_default();
        if !self.with_machine(|rt, m| rt.takes_args(m, &params, &args, id)) {
            return Ok(());
        }
        self.transaction(id, move |rt, m| rt.run_handler(m, &ev.handler, &ev.env, args).map(|_| ()))
    }

    /// A drawn widget writing one of its `mut` inputs, as a slider does.
    pub fn write(&mut self, id: &str, input: &str, value: Value) -> R<()> {
        self.changes += 1;
        self.reset_steps();
        let Some(node) = self.find(id) else { return Ok(()) };
        let (to, widget, key) = (node.writes.get(input).cloned(), node.widget.clone(), node.id.clone());
        let Some(to) = to else {
            // a widget's own value: kept by the page for it, checked against the input's type
            let Some(f) = self.rt.s.extern_widgets.get(&widget).and_then(|w| w.inputs.iter().find(|f| f.name == input && f.mutable)).cloned() else { return Ok(()) };
            if let Some(broken) = self.with_machine(|rt, m| rt.refusal(m, &value, &f.ty, id)) {
                self.rt.w().outcome.refused.push(broken);
                return Ok(());
            }
            {
                let mut w = self.rt.w();
                w.own.entry(key.clone()).or_default().insert(input.to_string(), value);
                w.bump(format!("o:{}", key));
            }
            return self.redraw();
        };
        let snap = self.snapshot();
        let r = self.with_machine(|rt, m| -> R<Option<Broken>> {
            rt.write_into(m, &widget, input, &to, value)?;
            Ok(rt.state_refusal(m))
        });
        match r {
            Ok(None) => {}
            Ok(Some(broken)) => {
                self.undo(snap);
                self.rt.w().outcome.refused.push(broken);
            }
            Err(f) => {
                self.undo(snap);
                self.rt.w().outcome.refused.push(refused(&f, id));
            }
        }
        self.redraw()
    }

    /// A layout pass's writes, as one change: a value equal to the one there is no change, and a
    /// refused write undoes the pass. Whether the layout has to run again: only when a variable
    /// changed, since a widget's own values are read by no Extended Mark code.
    pub fn layout_write(&mut self, writes: Vec<LayoutWrite>) -> R<bool> {
        self.reset_steps();
        let snap = self.snapshot();
        let own_before = self.rt.w.borrow().own.clone();
        let mut changed = false;
        let mut shared = false;
        for LayoutWrite { id, input, value } in writes {
            let Some(node) = self.find(&id) else { continue };
            if let Some(now) = node.props.get(&input) {
                if self.with_machine(|_, m| m.equal(now, &value))? {
                    continue;
                }
            }
            let (to, key, widget) = (node.writes.get(&input).cloned(), node.id.clone(), node.widget.clone());
            let r = match to {
                Some(to) => {
                    let r = self.with_machine(|rt, m| rt.write_into(m, &widget, &input, &to, value));
                    if r.is_ok() {
                        shared = true;
                    }
                    r
                }
                None => {
                    let Some(f) = self.rt.s.extern_widgets.get(&widget).and_then(|w| w.inputs.iter().find(|f| f.name == input && f.mutable)).cloned() else { continue };
                    match self.with_machine(|_, m| m.takes(&value, &f.ty)) {
                        Ok(true) => {
                            let mut w = self.rt.w();
                            w.own.entry(key.clone()).or_default().insert(input.clone(), value);
                            w.bump(format!("o:{}", key));
                            Ok(())
                        }
                        Ok(false) => Err(Failure::new(FailKind::Rule, format!("{} is given what isn't its type", input))),
                        Err(f) => Err(f),
                    }
                }
            };
            if let Err(f) = r {
                self.undo(snap);
                let mut w = self.rt.w();
                w.own = own_before;
                w.outcome.refused.push(refused(&f, &id));
                return Ok(false);
            }
            changed = true;
        }
        if !changed {
            return Ok(false);
        }
        if let Some(broken) = self.with_machine(|rt, m| rt.state_refusal(m)) {
            self.undo(snap);
            let mut w = self.rt.w();
            w.own = own_before;
            w.outcome.refused.push(broken);
            return Ok(false);
        }
        self.redraw()?;
        Ok(shared)
    }

    /// The warnings since the host last took them: `warn` rules found false (page 2).
    pub fn take_warnings(&mut self) -> Vec<Broken> {
        std::mem::take(&mut self.rt.w().warnings)
    }

    /// A plane whose layout didn't settle (page 6, failures).
    pub fn layout_failed(&mut self, id: &str) {
        let mut w = self.rt.w();
        if !w.outcome.failures.iter().any(|b| b.site.as_deref() == Some(id) && b.code.as_deref() == Some("failure.layout")) {
            w.outcome.failures.push(broken_code("failure.layout", id));
        }
    }

    /// A host's write to `$state`: one change, checked once: each value's type, then its
    /// contract, then `$state`'s rules.
    pub fn set(&mut self, changes: Vec<(String, Value)>) -> R<()> {
        self.changes += 1;
        self.reset_steps();
        if self.write_state(changes)? {
            self.redraw()?;
        }
        Ok(())
    }

    /// The same, with its values in the IR's form.
    pub fn set_given(&mut self, changes: &[(String, Expr)]) -> R<()> {
        self.changes += 1;
        self.reset_steps();
        if self.take_state(changes)? {
            self.redraw()?;
        }
        Ok(())
    }

    /// `$state` fields written, checked: refused whole, or done. Nothing is drawn.
    fn write_state(&mut self, changes: Vec<(String, Value)>) -> R<bool> {
        let snap = self.snapshot();
        for (k, v) in changes {
            let f = self.rt.s.state_decl.as_ref().and_then(|s| s.fields.iter().find(|f| f.name == k)).cloned();
            let site = format!("$state.{}", k);
            let broken = match &f {
                Some(f) => self.with_machine(|rt, m| rt.refusal(m, &v, &f.ty, &site)),
                None => Some(broken_code("contracts.outside", &site)),
            };
            if let Some(broken) = broken {
                self.undo(snap);
                self.rt.w().outcome.refused.push(broken);
                return Ok(false);
            }
            let mut w = self.rt.w();
            w.state_set(&k, v);
            w.bump(format!("s:{}", k));
        }
        if let Some(broken) = self.with_machine(|rt, m| rt.state_refusal(m)) {
            self.undo(snap);
            self.rt.w().outcome.refused.push(broken);
            return Ok(false);
        }
        Ok(true)
    }

    /// A host calling a fn the document exposes.
    pub fn expose(&mut self, channel: &str, name: &str, args: Vec<Value>) -> R<()> {
        let s = self.rt.s.clone();
        for d in &s.module.declarations {
            let ir::Decl::Expose(e) = &d.declare else { continue };
            if e.channel != channel {
                continue;
            }
            let Some(f) = e.fns.iter().find(|f| f.name == name) else { continue };
            if !self.with_machine(|rt, m| rt.takes_args(m, &f.params, &args, &format!("{}.{}", channel, name))) {
                return Ok(());
            }
            let body = Rc::new(f.body.clone());
            return self.transaction(&format!("{}.{}", channel, name), move |rt, m| rt.run_body(m, &f.params, body, &Env::default(), args).map(|_| ()));
        }
        Ok(())
    }

    /// The host giving a channel's values: one that doesn't fit its type is dropped for its default, and
    /// the host is told.
    pub fn set_extern(&mut self, channel: &str, values: Vec<(String, Value)>) -> R<()> {
        self.put_externs(channel, values.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    /// The same, with its values in the IR's form.
    pub fn set_extern_given(&mut self, channel: &str, values: &[(String, Expr)]) -> R<()> {
        let values = values.iter().map(|(k, x)| (k.clone(), self.given(x, &format!("{}.{}", channel, k)))).collect();
        self.put_externs(channel, values)
    }

    /// A channel's value now, by its full name: `doc/host.n`.
    pub fn given_extern(&self, name: &str) -> Option<Value> {
        self.rt.w.borrow().externs.get(name).cloned()
    }

    fn put_externs(&mut self, channel: &str, values: Vec<(String, Option<Value>)>) -> R<()> {
        self.changes += 1;
        self.reset_steps();
        for (k, v) in values {
            self.with_machine(|rt, m| rt.give_extern(m, channel, &k, v))?;
            self.rt.w().bump(format!("x:{}.{}", channel, k));
        }
        self.redraw()
    }

    /// The page's own values: its width and theme.
    pub fn set_page(&mut self, page: PageValues) -> R<()> {
        self.changes += 1;
        self.apply_page(page);
        self.redraw()
    }

    /// Which extern widgets the engine draws: set by an engine before it first draws the page,
    /// from its registry. The page is drawn again.
    pub fn set_supplies(&mut self, supplies: Option<Supplies>) -> R<()> {
        {
            let mut w = self.rt.w();
            w.supplies = supplies;
            w.epoch += 1;
            w.missed.clear();
        }
        self.redraw()
    }

    /// Turns reuse on or off: off draws every widget anew on every change, for comparing the
    /// two. What was kept stays until it is drawn again.
    pub fn set_memo(&mut self, on: bool) {
        self.rt.w().memo_on = on;
    }

    // ---- the page's Markdown

    /// The page as Markdown, with the values it has now: the same text on every engine.
    pub fn markdown(&self) -> R<String> {
        self.reset_steps();
        let blocks = self.with_machine(|rt, m| crate::markdown::page_blocks(rt, m))?;
        Ok(xtmd_markdown::write(&blocks))
    }

    /// The page's Markdown as blocks, before they are written.
    pub fn markdown_blocks(&self) -> R<Vec<xtmd_markdown::Block>> {
        self.reset_steps();
        self.with_machine(|rt, m| crate::markdown::page_blocks(rt, m))
    }

    // ---- what an engine asks while it draws

    /// A value in the IR's form, as the host and the checks give them.
    pub fn value(&self, x: &Expr) -> R<Value> {
        self.constant(x)
    }

    /// Works out an expression on the page, outside any widget: a binding, a `let`, `$state`.
    pub fn eval(&self, x: &Expr) -> R<Value> {
        self.reset_steps();
        self.with_machine(|rt, m| rt.eval_in(m, &Env::default(), x))
    }

    /// The value a token stands for, from the build's tokens: `core/space.M` is 12.
    pub fn token_value(&self, v: &Value) -> R<Value> {
        match v {
            Value::Case(c) => self.with_machine(|_, m| m.default_token_value(c)),
            other => Ok(other.clone()),
        }
    }

    /// Whether two values are the same, as `==` says.
    pub fn equal(&self, a: &Value, b: &Value) -> R<bool> {
        self.with_machine(|_, m| m.equal(a, b))
    }

    /// Runs a fn the tree holds, outside any change, with the step limit to itself: how an engine
    /// paints a `shade`, one pixel at a time from its shader.
    pub fn apply(&self, f: &Value, args: Vec<Value>) -> R<Option<Value>> {
        self.reset_steps();
        self.with_machine(|_, m| m.apply(f, args.into_iter().map(Binding::Value).collect()))
    }

    // ---- for the host's side (host.rs)

    pub(crate) fn held_fns(&mut self) -> &mut crate::host::HeldFns {
        &mut self.held_fns
    }

    pub(crate) fn state_values(&self) -> Vec<(String, Value)> {
        self.rt.w.borrow().state.clone()
    }

    pub(crate) fn is_token(&self, ty: &str) -> bool {
        self.rt.s.program.tokens.contains_key(ty)
    }

    pub(crate) fn alias(&self, name: &str) -> Option<ir::Type> {
        self.rt.s.program.aliases.get(name).cloned()
    }

    /// An extern widget's `mut` inputs, with their types.
    pub(crate) fn mut_inputs(&self, widget: &str) -> Vec<(String, ir::Type)> {
        self.rt.s.extern_widgets.get(widget).map(|w| w.inputs.iter().filter(|f| f.mutable).map(|f| (f.name.clone(), f.ty.clone())).collect()).unwrap_or_default()
    }

    pub(crate) fn is_current(&self, deps: &HashMap<String, u64>) -> bool {
        self.rt.w.borrow().current(deps)
    }

    /// Whether two fns are the same code with the same values around it.
    pub(crate) fn same_fn(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Fn(x), Value::Fn(y)) => Rc::ptr_eq(x, y) || (Rc::ptr_eq(&x.body, &y.body) && x.owner == y.owner && self.with_machine(|rt, m| rt.same_bindings(m, &x.captured, &y.captured))),
            _ => false,
        }
    }

    /// A fn called, with what it read.
    pub(crate) fn apply_reading(&self, f: &Value, args: Vec<Value>) -> (R<Option<Value>>, HashMap<String, u64>) {
        self.with_machine(|rt, m| {
            m.reset_steps();
            rt.reading(|| m.apply(f, args.into_iter().map(Binding::Value).collect()))
        })
    }

    /// How many of its views' expressions the page has worked out, for a test that counts them.
    pub fn worked(&self) -> u64 {
        self.rt.w.borrow().worked
    }

    /// Whether a drawn widget's event runs anything. A handler that only passes the event on, as
    /// `column` gives its `flex` `click = fn() { invoke click(); }`, counts only when the caller
    /// of the widget it passes it to set that event in turn: so an engine makes only what does
    /// something clickable.
    pub fn listens(&self, d: &Drawn, event: &str) -> bool {
        let w = self.rt.w.borrow();
        let mut ev = d.events.get(event).cloned();
        for _ in 0..64 {
            let Some(e) = ev else { return false };
            let Some(h) = self.rt.s.handlers.get(&e.handler) else { return false };
            let Some(only) = &h.passes else { return true };
            ev = e.env.owner.and_then(|o| w.instances.get(&o)).and_then(|i| i.events.get(only).cloned());
        }
        false
    }

    /// The type of the variable a drawn widget's `mut` input writes, its contract included:
    /// where a slider's ends and step come from (page 5). The input's own type when the caller
    /// gave no variable; `None` for a local, whose type the IR doesn't keep.
    pub fn write_type(&self, d: &Drawn, input: &str) -> Option<ir::Type> {
        let s = &self.rt.s;
        let Some(to) = d.writes.get(input) else {
            return s.extern_widgets.get(&d.widget).and_then(|w| w.inputs.iter().find(|f| f.name == input)).map(|f| f.ty.clone());
        };
        let w = self.rt.w.borrow();
        let inst = to.env.owner.and_then(|o| w.instances.get(&o));
        let mut t = match &to.place.root {
            ir::Root::State(n) => s.state_decl.as_ref().and_then(|st| st.fields.iter().find(|f| f.name == *n)).map(|f| f.ty.clone()),
            ir::Root::Cell(n) => inst.and_then(|i| i.template.cells.iter().find(|c| c.name == *n)).map(|c| c.ty.clone()),
            ir::Root::Input(n) => inst.and_then(|i| i.template.inputs.iter().find(|f| f.name == *n)).map(|f| f.ty.clone()),
            ir::Root::Local(_) => None,
        };
        for step in &to.place.path {
            let mut base = t.clone();
            for _ in 0..16 {
                match base.as_ref().and_then(|b| s.program.aliases.get(&b.name)) {
                    Some(next) => base = Some(next.clone()),
                    None => break,
                }
            }
            let base = base?;
            t = match step {
                // a record's field, or a list's item
                ir::Step::Field(f) => {
                    let fields = if base.fields.is_empty() { s.program.records.get(&base.name).map(|r| r.fields.clone()).unwrap_or_default() } else { base.fields.clone() };
                    fields.iter().find(|x| x.name == f.name).map(|x| x.ty.clone())
                }
                ir::Step::Item(_) => base.args.first().cloned(),
            };
        }
        t
    }

    // ---- inside

    fn apply_page(&mut self, page: PageValues) {
        let mut w = self.rt.w();
        if let Some(width) = page.width {
            let name = match width {
                PageWidth::Narrow => "NARROW",
                PageWidth::Medium => "MEDIUM",
                PageWidth::Wide => "WIDE",
            };
            w.externs.insert("core/page.width".into(), case_value("core/width", name));
            w.bump("x:core/page.width".into());
        }
        if let Some(theme) = page.theme {
            let name = match theme {
                PageTheme::Light => "LIGHT",
                PageTheme::Dark => "DARK",
            };
            w.externs.insert("core/page.theme".into(), case_value("core/theme", name));
            w.bump("x:core/page.theme".into());
        }
    }

    /// A value a host gives, in the IR's form: one that can't be made, such as a record whose own rule is
    /// false, is refused, and `None`.
    fn given(&self, x: &Expr, site: &str) -> Option<Value> {
        match self.constant(x) {
            Ok(v) => Some(v),
            Err(f) => {
                let mut w = self.rt.w();
                w.outcome.refused.push(refused(&f, site));
                w.errors.push(f);
                None
            }
        }
    }

    fn constant(&self, x: &Expr) -> R<Value> {
        Machine::new(&self.rt.s.program, Box::new(Constants)).eval(x)
    }

    fn snapshot(&self) -> Snapshot {
        let w = self.rt.w.borrow();
        Snapshot { state: w.state.clone(), cells: w.instances.iter().map(|(id, i)| (*id, i.cells.clone())).collect(), calls: w.calls.len() }
    }

    fn undo(&self, s: Snapshot) {
        let mut w = self.rt.w();
        // what was drawn from the undone values goes with them
        w.epoch += 1;
        w.state = s.state;
        for (id, cells) in s.cells {
            if let Some(i) = w.instances.get_mut(&id) {
                i.cells = cells;
            }
        }
        w.calls.truncate(s.calls);
    }

    /// A change from outside: done whole, with `$state`'s rules after it, or undone.
    fn transaction(&mut self, site: &str, run: impl FnOnce(&Rt, &mut Machine) -> R<()>) -> R<()> {
        self.changes += 1;
        let snap = self.snapshot();
        let r = self.with_machine(|rt, m| -> R<()> {
            m.reset_steps();
            run(rt, m)?;
            if let Some(rule) = rt.state_rule_broken(m)? {
                return Err(Failure { kind: FailKind::Rule, message: rule.clone(), rule: Some(rule) });
            }
            Ok(())
        });
        if let Err(f) = r {
            self.undo(snap);
            let broken = match (&f.kind, &f.rule) {
                (FailKind::Rule, Some(rule)) => Broken { code: None, rule: Some(rule.clone()), site: Some(site.to_string()) },
                _ => broken_code(&failure_code(&f), site),
            };
            let mut w = self.rt.w();
            w.outcome.failures.push(broken);
            w.errors.push(f);
        }
        self.redraw()
    }

    /// The drawn widget of an id.
    pub fn find(&self, id: &str) -> Option<&Drawn> {
        find_node(&self.drawn, &|n| n.id == id)
    }

    /// The widget drawn at a place, loop rows by their position.
    pub fn at(&self, path: &str) -> Option<&Drawn> {
        find_node(&self.drawn, &|n| n.path == path)
    }

    /// Draws the whole page again: once per change.
    pub fn redraw(&mut self) -> R<()> {
        let drawn = self.with_machine(|rt, m| -> R<Vec<Drawn>> {
            // each redraw has the step limit to itself, as each event does: steps never pile up
            m.reset_steps();
            {
                let mut w = rt.w();
                w.draws += 1;
                w.claimed.clear();
                w.missed.clear();
                w.at_path.clear();
                w.own_seen.clear();
                w.node_seen.clear();
                w.frames.clear();
            }
            let rules = rt.s.state_rules.clone();
            for (k, (_, message, _)) in rules.iter().enumerate().filter(|(_, r)| r.2) {
                match rt.state_rule(m, k) {
                    Ok(holds) => rt.w().warned("$state", message, holds),
                    // one that can't be worked out is a failure, and stops nothing
                    Err(f) => {
                        let mut w = rt.w();
                        w.outcome.failures.push(broken_code(&failure_code(&f), "$state"));
                        w.errors.push(f);
                    }
                }
            }
            let drawn = match rt.s.templates.get("doc") {
                Some(doc) => rt.draw_nodes(m, &doc.view, &Env::default())?,
                None => Vec::new(),
            };
            // a widget that left the page leaves with its variables
            let mut w = rt.w();
            let gone: Vec<(u64, String)> = w.instances.values().filter(|i| i.seen != w.draws).map(|i| (i.id, i.path.clone())).collect();
            for (id, path) in gone {
                w.instances.remove(&id);
                // only its own: a widget of another kind may have taken its key this time
                if w.by_path.get(&path) == Some(&id) {
                    w.by_path.remove(&path);
                    w.memo.remove(&path);
                }
            }
            let seen = std::mem::take(&mut w.own_seen);
            w.own.retain(|k, _| seen.contains(k));
            w.own_seen = seen;
            let draws = w.draws;
            w.extern_memo.retain(|_, x| x.frame.seen.get() == draws);
            w.node_memo.retain(|_, x| x.frame.seen.get() == draws);
            Ok(drawn)
        })?;
        self.drawn = drawn;
        self.revision += 1;
        for (_, l) in self.listeners.iter_mut() {
            l();
        }
        Ok(())
    }
}

fn find_node<'a>(nodes: &'a [Drawn], is: &dyn Fn(&Drawn) -> bool) -> Option<&'a Drawn> {
    for n in nodes {
        if is(n) {
            return Some(n);
        }
        for kids in n.slots.values().chain(n.held.iter()) {
            if let Some(f) = find_node(kids, is) {
                return Some(f);
            }
        }
    }
    None
}

/// A shader's name: a named fn is lowered to a lambda that calls it with its own parameters.
fn shader_name(f: &Value) -> String {
    let Value::Fn(c) = f else { return String::new() };
    let [Stmt::Return(r)] = c.body.as_slice() else { return String::new() };
    let Some(Expr::Call(call)) = &r.value else { return String::new() };
    let passes = call.args.len() == c.params.len() && call.args.iter().zip(c.params.iter()).all(|(a, p)| matches!(a, Expr::Get(ir::Get::Local(n)) if *n == p.name));
    if passes {
        call.name.clone()
    } else {
        String::new()
    }
}
