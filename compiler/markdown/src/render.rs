//! A page's Markdown nodes worked out into blocks: each widget's `markdown` with its inputs, its
//! own variables at their starting values, `$state` and the page's values as they are.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::Arc;

use xtmd_eval::interp::Outer;
use xtmd_eval::value::ContentOf;
use xtmd_eval::{Binding, FailKind, Host, Machine, Program, Value};
use xtmd_ir::ir::{self, Expr, Node};

use crate::{join_runs, Align, Block, Inline, Item};

/// What went wrong writing the page.
#[derive(Debug)]
pub struct Failure(pub String);

type R<T> = Result<T, Failure>;

fn fail(e: xtmd_eval::Failure) -> Failure {
    Failure(e.message)
}

/// Where a node's values are read: its widget instance and the locals around it.
#[derive(Clone, Default)]
struct Env {
    owner: Option<u64>,
    locals: Vec<(Rc<str>, Binding)>,
}

/// One widget being written: its inputs, its own variables and its slots, with where they were
/// written.
#[derive(Default)]
struct Instance {
    inputs: HashMap<String, Value>,
    cells: HashMap<String, Value>,
    slots: HashMap<String, (Arc<Vec<Node>>, Env)>,
    widget: String,
}

#[derive(Default)]
struct Shared {
    instances: HashMap<u64, Instance>,
    state: HashMap<String, Value>,
    externs: HashMap<String, Value>,
    lets: HashMap<String, Value>,
    /// Each widget's slots' types, by the widget's name.
    slot_types: HashMap<(String, String), ir::Type>,
}

struct Page(Rc<RefCell<Shared>>);

impl Host for Page {
    fn read(&self, m: &mut Machine, what: &Outer, owner: Option<u64>) -> Result<Value, xtmd_eval::Failure> {
        let sh = self.0.borrow();
        let missing = |what: &Outer| xtmd_eval::Failure::new(FailKind::Ir, format!("{:?} isn't known here", what));
        match what {
            Outer::State(n) => sh.state.get(&**n).cloned().ok_or_else(|| missing(what)),
            Outer::Extern(n) => sh.externs.get(&**n).cloned().ok_or_else(|| xtmd_eval::Failure::new(FailKind::NotConstant, format!("`{}` is given when the page runs", n))),
            Outer::Let(n) => match m.prog.lets.get(&**n) {
                Some(v) => Ok(v.clone()),
                None => sh.lets.get(&**n).cloned().ok_or_else(|| missing(what)),
            },
            Outer::Input(n) => owner.and_then(|o| sh.instances.get(&o)).and_then(|i| i.inputs.get(&**n)).cloned().ok_or_else(|| missing(what)),
            Outer::Cell(n) => owner.and_then(|o| sh.instances.get(&o)).and_then(|i| i.cells.get(&**n)).cloned().ok_or_else(|| missing(what)),
            Outer::Slot(n) => {
                let held = owner.and_then(|o| sh.instances.get(&o)).and_then(|i| i.slots.get(&**n).cloned().map(|s| (s, sh.slot_types.get(&(i.widget.clone(), n.to_string())).cloned())));
                drop(sh);
                match held {
                    Some(((nodes, at), ty)) => m.slot_value(&ty.unwrap_or_else(|| ir::Type::named("core/view")), &nodes, &at.locals, at.owner, (Rc::from(""), Rc::from(""))),
                    // a slot left out
                    None => Ok(Value::none()),
                }
            }
        }
    }

    fn write(&self, _m: &mut Machine, what: &Outer, _owner: Option<u64>, _value: Value) -> Result<(), xtmd_eval::Failure> {
        Err(xtmd_eval::Failure::new(FailKind::NotConstant, format!("writing the page's Markdown changes nothing, so {:?} isn't written", what)))
    }
}

/// What one rendered node gives: blocks, or an item of the list around it.
enum Piece {
    Block(Block),
    /// A part of a sentence.
    Run(Inline),
    Item(Item),
}

struct Writer<'p> {
    m: Machine<'p>,
    shared: Rc<RefCell<Shared>>,
    templates: HashMap<String, ir::Template>,
    externs: HashMap<String, ir::ExternWidget>,
    depth: usize,
}

fn get<'a>(v: &'a Value, field: &str) -> Option<&'a Value> {
    match v {
        Value::Record(r) => r.get(field),
        Value::Case(c) => c.values.iter().find(|(k, _)| &**k == field).map(|(_, x)| x),
        _ => None,
    }
}

fn text_of(v: Option<&Value>) -> String {
    match v {
        Some(Value::Text(t)) => t.to_string(),
        Some(Value::Num(n)) => n.show(),
        Some(Value::Opt(Some(x))) => text_of(Some(x)),
        _ => String::new(),
    }
}

fn items(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::List(l)) => l.items.iter().map(|i| i.value.clone()).collect(),
        _ => Vec::new(),
    }
}

/// A template's text: its pieces with each hole's shown text between them (TeX, a template).
fn template_text(v: Option<&Value>) -> String {
    let Some(v) = v else { return String::new() };
    let pieces = items(get(v, "pieces"));
    let holes = items(get(v, "holes"));
    let mut out = String::new();
    for (k, p) in pieces.iter().enumerate() {
        out.push_str(&text_of(Some(p)));
        if let Some(h) = holes.get(k) {
            out.push_str(&text_of(get(h, "value")));
        }
    }
    out
}

impl<'p> Writer<'p> {
    fn eval(&mut self, env: &Env, e: &Expr) -> R<Value> {
        self.m.eval_with(&env.locals, env.owner, e).map_err(fail)
    }

    fn new_instance(&mut self, inst: Instance) -> u64 {
        let id = self.m.new_id();
        self.shared.borrow_mut().instances.insert(id, inst);
        id
    }

    fn nodes(&mut self, nodes: &[Node], env: &Env) -> R<Vec<Piece>> {
        let mut env = env.clone();
        let mut out = Vec::new();
        for n in nodes {
            match n {
                Node::Declare(d) => {
                    let v = self.eval(&env, &d.let_.value)?;
                    env.locals.push((Rc::from(d.let_.name.as_str()), Binding::Value(v)));
                }
                Node::For(f) => {
                    for row in self.m.loop_rows(&env.locals, env.owner, &f.over, &f.name, f.position.as_deref(), f.mutable).map_err(fail)? {
                        let mut inner = env.clone();
                        inner.locals.extend(row.bindings);
                        out.extend(self.nodes(&f.body, &inner)?);
                    }
                }
                Node::If(i) => {
                    let mut done = false;
                    for b in &i.branches {
                        let mut inner = env.clone();
                        let pass = match &b.when {
                            ir::Cond::Cond(e) => matches!(self.eval(&env, e)?, Value::Bool(true)),
                            ir::Cond::Open(o) => match self.m.open_cond(&env.locals, env.owner, o).map_err(fail)? {
                                Some(b) => {
                                    inner.locals.push((Rc::from(o.name.as_str()), b));
                                    true
                                }
                                None => false,
                            },
                        };
                        if pass {
                            out.extend(self.nodes(&b.then, &inner)?);
                            done = true;
                            break;
                        }
                    }
                    if !done {
                        out.extend(self.nodes(&i.else_, &env)?);
                    }
                }
                Node::Match(mt) => {
                    let v = self.eval(&env, &mt.value)?;
                    match self.m.pick_case(mt, &v).map_err(fail)? {
                        Some((k, binds)) => {
                            let mut inner = env.clone();
                            inner.locals.extend(binds);
                            out.extend(self.nodes(&mt.cases[k].then, &inner)?);
                        }
                        None => out.extend(self.nodes(&mt.else_, &env)?),
                    }
                }
                Node::Place(p) => {
                    let v = self.eval(&env, &p.view)?;
                    if p.sentence {
                        check_sentence(&v)?;
                    }
                    out.extend(self.view(&v)?);
                }
                Node::Extern(x) => out.extend(self.extern_call(x, &env)?),
                Node::Use(u) => out.extend(self.use_call(u, &env)?),
            }
        }
        Ok(out)
    }

    /// A view held in a value: a call made as one, or calls.
    fn view(&mut self, v: &Value) -> R<Vec<Piece>> {
        match v {
            Value::View(view) => {
                // its slots are read where the view was made
                let at = Env { owner: view.owner, locals: view.captured.clone() };
                let slots = view.slots.iter().map(|(k, n)| (k.clone(), (Arc::new(n.clone()), at.clone()))).collect();
                self.widget(&view.widget, view.args.clone(), slots)
            }
            Value::Content(c) => match &c.of {
                ContentOf::Nodes { nodes, captured, owner, .. } => self.nodes(nodes, &Env { owner: *owner, locals: captured.clone() }),
                ContentOf::Parts(parts) => {
                    let mut out = Vec::new();
                    for p in parts {
                        out.extend(self.view(p)?);
                    }
                    Ok(out)
                }
                ContentOf::Drawn(_) => Ok(Vec::new()),
            },
            Value::List(l) => {
                let mut out = Vec::new();
                for i in &l.items {
                    out.extend(self.view(&i.value)?);
                }
                Ok(out)
            }
            Value::Opt(Some(x)) => self.view(x),
            _ => Ok(Vec::new()),
        }
    }

    fn extern_call(&mut self, x: &ir::NodeExtern, env: &Env) -> R<Vec<Piece>> {
        let mut props = BTreeMap::new();
        for (k, e) in &x.props {
            props.insert(k.clone(), self.eval(env, e)?);
        }
        for (k, p) in &x.writes {
            props.insert(k.clone(), self.eval(env, &Expr::Place(p.clone()))?);
        }
        let slots: HashMap<String, (Arc<Vec<Node>>, Env)> = x.slots.iter().map(|(k, n)| (k.clone(), (Arc::new(n.clone()), env.clone()))).collect();
        self.widget(&x.name, props, slots)
    }

    fn use_call(&mut self, u: &ir::NodeUse, env: &Env) -> R<Vec<Piece>> {
        let mut props = BTreeMap::new();
        for (k, e) in &u.args {
            props.insert(k.clone(), self.eval(env, e)?);
        }
        let slots: HashMap<String, (Arc<Vec<Node>>, Env)> = u.slots.iter().map(|(k, n)| (k.clone(), (Arc::new(n.clone()), env.clone()))).collect();
        self.widget(&u.name, props, slots)
    }

    /// A widget, given its inputs' values and its slots: one that is a piece of Markdown is
    /// written as it; any other writes its `markdown`.
    fn widget(&mut self, name: &str, props: BTreeMap<String, Value>, slots: HashMap<String, (Arc<Vec<Node>>, Env)>) -> R<Vec<Piece>> {
        self.depth += 1;
        if self.depth > 64 {
            self.depth -= 1;
            return Err(Failure(format!("`{}` places itself more than 64 deep", name)));
        }
        let out = self.widget_inner(name, props, slots);
        self.depth -= 1;
        // a widget that fails is a notice in its place, a quote in Markdown; the page carries on (page 6)
        match out {
            Err(f) if self.externs.get(name).map_or(true, |w| w.piece.is_none()) => {
                let short = name.rsplit('/').next().unwrap_or(name);
                let notice = Inline::Text { value: format!("{} failed: {}", short, f.0), bold: false, italic: false, strike: false, link: None };
                Ok(vec![Piece::Block(Block::Quote(vec![Block::Paragraph(vec![notice])]))])
            }
            other => other,
        }
    }

    fn slot_blocks(&mut self, slots: &HashMap<String, (Arc<Vec<Node>>, Env)>, name: &str) -> R<Vec<Piece>> {
        match slots.get(name) {
            Some((nodes, env)) => {
                let (nodes, env) = (nodes.clone(), env.clone());
                self.nodes(&nodes, &env)
            }
            None => Ok(Vec::new()),
        }
    }

    fn widget_inner(&mut self, name: &str, props: BTreeMap<String, Value>, slots: HashMap<String, (Arc<Vec<Node>>, Env)>) -> R<Vec<Piece>> {
        let p = |k: &str| props.get(k);
        let block = |b: Block| Ok(vec![Piece::Block(b)]);
        let piece = self.externs.get(name).and_then(|w| w.piece.clone());
        match piece.as_deref().unwrap_or("") {
            "paragraph" => {
                let runs = self.sentence(p("value"))?;
                return if runs.is_empty() { Ok(Vec::new()) } else { block(Block::Paragraph(runs)) };
            }
            "heading" => {
                let level = match p("level") {
                    Some(Value::Num(n)) => n.to_i64().unwrap_or(1).clamp(1, 6) as u8,
                    _ => 1,
                };
                let inline = self.sentence(p("value"))?;
                return block(Block::Heading { level, inline });
            }
            "bullets" => {
                let ordered = matches!(p("ordered"), Some(Value::Bool(true)));
                let start = match p("start") {
                    Some(Value::Num(n)) => n.to_i64().unwrap_or(1),
                    _ => 1,
                };
                let pieces = self.slot_blocks(&slots, "items")?;
                let items: Vec<Item> = pieces
                    .into_iter()
                    .map(|x| match x {
                        Piece::Item(i) => i,
                        other => Item { done: None, blocks: blocks_of(vec![other]) },
                    })
                    .collect();
                // a list with no items writes nothing
                return if items.is_empty() { Ok(Vec::new()) } else { block(Block::Bullets { ordered, start, items }) };
            }
            "item" => {
                let done = match p("done") {
                    Some(Value::Opt(Some(d))) => d.as_bool(),
                    Some(Value::Bool(b)) => Some(*b),
                    _ => None,
                };
                let blocks = blocks_of(self.slot_blocks(&slots, "body")?);
                return Ok(vec![Piece::Item(Item { done, blocks })]);
            }
            "quote" => {
                let blocks = blocks_of(self.slot_blocks(&slots, "body")?);
                return block(Block::Quote(blocks));
            }
            "code" => return block(Block::Code { lang: text_of(p("lang")), src: text_of(p("src")) }),
            "math" => return block(Block::Math(template_text(p("tex").and_then(|t| get(t, "tex"))))),
            "divider" => return block(Block::Divider),
            "text" => return Ok(vec![Piece::Run(Inline::words(text_of(p("value"))))]),
            "bold" | "italic" | "strike" | "link" => {
                let body = runs_of(self.slot_blocks(&slots, "body")?);
                let to = text_of(p("to"));
                let marked = body.into_iter().map(|r| match r {
                    Inline::Text { value, mut bold, mut italic, mut strike, mut link } => {
                        match piece.as_deref() {
                            Some("bold") => bold = true,
                            Some("italic") => italic = true,
                            Some("strike") => strike = true,
                            _ => link = Some(to.clone()),
                        }
                        Inline::Text { value, bold, italic, strike, link }
                    }
                    other => other,
                });
                return Ok(marked.map(Piece::Run).collect());
            }
            "code_span" => return Ok(vec![Piece::Run(Inline::Code(text_of(p("src"))))]),
            "inline_math" => return Ok(vec![Piece::Run(Inline::Math(template_text(p("tex").and_then(|t| get(t, "tex")))))]),
            "inline_image" => return Ok(vec![Piece::Run(Inline::Image { src: text_of(p("src")), alt: text_of(p("alt")) })]),
            "note_mark" => return Ok(vec![Piece::Run(Inline::Note(text_of(p("mark"))))]),
            "line_break" => return Ok(vec![Piece::Run(Inline::Break)]),
            "footnote" => {
                let blocks = blocks_of(self.slot_blocks(&slots, "body")?);
                return block(Block::Footnote { mark: text_of(p("mark")), blocks });
            }
            "table" => {
                let mut head = Vec::new();
                for c in items(p("head")) {
                    head.push(self.sentence(Some(&c))?);
                }
                let mut rows = Vec::new();
                for r in items(p("rows")) {
                    let mut row = Vec::new();
                    for c in items(Some(&r)) {
                        row.push(self.sentence(Some(&c))?);
                    }
                    rows.push(row);
                }
                let align = items(p("align"))
                    .iter()
                    .map(|a| match a {
                        Value::Case(c) if &*c.name == "CENTER" => Align::Center,
                        Value::Case(c) if &*c.name == "END" => Align::End,
                        _ => Align::Start,
                    })
                    .collect();
                return block(Block::Table { head, rows, align });
            }
            _ => {}
        }
        // a widget written in Extended Mark: its markdown, with its inputs and its own variables
        if let Some(t) = self.templates.get(name).cloned() {
            let id = self.new_instance(Instance { widget: name.to_string(), ..Instance::default() });
            let env = Env { owner: Some(id), locals: Vec::new() };
            let mut inputs = HashMap::new();
            for f in &t.inputs {
                let v = match props.get(&f.name) {
                    Some(v) => v.clone(),
                    None => match &f.default {
                        Some(d) => self.eval(&env, d)?,
                        None => Value::none(),
                    },
                };
                inputs.insert(f.name.clone(), v);
            }
            {
                let mut sh = self.shared.borrow_mut();
                let inst = sh.instances.get_mut(&id).unwrap();
                inst.inputs = inputs;
                inst.slots = slots;
            }
            for c in &t.cells {
                let v = match &c.default {
                    Some(d) => self.eval(&env, d)?,
                    None => Value::none(),
                };
                self.shared.borrow_mut().instances.get_mut(&id).unwrap().cells.insert(c.name.clone(), v);
            }
            return self.nodes(t.markdown(), &env);
        }
        // an extern widget: its markdown block, with its inputs
        if let Some(w) = self.externs.get(name).cloned() {
            let id = self.new_instance(Instance { slots, widget: name.to_string(), ..Instance::default() });
            let env = Env { owner: Some(id), locals: Vec::new() };
            for f in &w.inputs {
                let v = match props.get(&f.name) {
                    Some(v) => v.clone(),
                    None => match &f.default {
                        Some(d) => self.eval(&env, d)?,
                        None => Value::none(),
                    },
                };
                self.shared.borrow_mut().instances.get_mut(&id).unwrap().inputs.insert(f.name.clone(), v);
            }
            return self.nodes(&w.markdown, &env);
        }
        Err(Failure(format!("there is no widget `{}` to write", name)))
    }

    /// A sentence's calls as runs.
    fn sentence(&mut self, v: Option<&Value>) -> R<Vec<Inline>> {
        let Some(v) = v else { return Ok(Vec::new()) };
        check_sentence(v)?;
        Ok(join_runs(runs_of(self.view(v)?)))
    }
}

/// A sentence goes here, so a rich_text of blocks doesn't.
fn check_sentence(v: &Value) -> R<()> {
    match v {
        Value::Content(c) if c.blocks => Err(Failure("a sentence goes here, and this rich_text is blocks".into())),
        Value::Opt(Some(x)) => check_sentence(x),
        _ => Ok(()),
    }
}

/// What some calls write in a sentence: their runs, and the words of any blocks among them.
fn runs_of(pieces: Vec<Piece>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    let mut blocks: Vec<Piece> = Vec::new();
    let flush = |blocks: &mut Vec<Piece>, out: &mut Vec<Inline>| {
        let mut bs = blocks_of(std::mem::take(blocks));
        match bs.as_mut_slice() {
            [] => {}
            // a widget that is one paragraph writes its sentence
            [Block::Paragraph(runs)] => out.append(runs),
            bs => {
                let words: Vec<String> = bs.iter().map(plain).filter(|s| !s.is_empty()).collect();
                out.push(Inline::words(words.join(" ")));
            }
        }
    };
    for p in pieces {
        match p {
            Piece::Run(r) => {
                flush(&mut blocks, &mut out);
                out.push(r);
            }
            other => blocks.push(other),
        }
    }
    flush(&mut blocks, &mut out);
    out
}

/// A block's words, for a view placed in a sentence.
fn plain(b: &Block) -> String {
    let runs = |r: &[Inline]| {
        r.iter()
            .map(|i| match i {
                Inline::Text { value, .. } | Inline::Code(value) | Inline::Math(value) => value.clone(),
                Inline::Image { alt, .. } => alt.clone(),
                _ => String::new(),
            })
            .collect::<String>()
    };
    match b {
        Block::Paragraph(r) | Block::Heading { inline: r, .. } => runs(r),
        Block::Quote(bs) => bs.iter().map(plain).collect::<Vec<_>>().join(" "),
        _ => String::new(),
    }
}

/// Pieces as blocks: list items that follow one another make one list, across a loop's turns
/// too, and any other block ends it (page 3).
fn blocks_of(pieces: Vec<Piece>) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut in_sentence = false;
    for p in pieces {
        let b = match p {
            Piece::Block(b) => b,
            // a sentence among blocks is a paragraph
            Piece::Run(r) => {
                if let (Some(Block::Paragraph(runs)), true) = (out.last_mut(), in_sentence) {
                    runs.push(r);
                } else {
                    out.push(Block::Paragraph(vec![r]));
                }
                in_sentence = true;
                continue;
            }
            Piece::Item(i) => Block::Bullets { ordered: false, start: 1, items: vec![i] },
        };
        in_sentence = false;
        if let (Some(Block::Bullets { ordered: o1, items: i1, .. }), Block::Bullets { ordered: o2, items: i2, .. }) = (out.last_mut(), &b) {
            if *o1 == *o2 {
                i1.extend(i2.iter().cloned());
                continue;
            }
        }
        out.push(b);
    }
    out
}

/// The page's Markdown, with the values it starts with: a document's own, or for a pack, nothing.
pub fn page(m: &ir::Module) -> R<Vec<Block>> {
    let mut prog = Program::from_module(m);
    for t in &m.templates {
        for f in &t.fns {
            prog.fns.insert(f.name.clone(), Rc::new(f.clone()));
        }
    }
    let shared = Rc::new(RefCell::new(Shared::default()));
    let mut w = Writer {
        m: Machine::new(&prog, Box::new(Page(shared.clone()))),
        shared: shared.clone(),
        templates: m.templates.iter().map(|t| (t.name.clone(), t.clone())).collect(),
        externs: HashMap::new(),
        depth: 0,
    };
    for (name, slots) in m.templates.iter().map(|t| (&t.name, &t.slots)).chain(m.declarations.iter().filter_map(|d| match &d.declare {
        ir::Decl::Extern(ir::Extern::Widget(x)) => Some((&x.name, &x.slots)),
        _ => None,
    })) {
        for sl in slots {
            shared.borrow_mut().slot_types.insert((name.clone(), sl.name.clone()), sl.ty.clone());
        }
    }
    let none = Env::default();
    // the page's values, $state's starting values, then lets that read them
    for d in &m.declarations {
        match &d.declare {
            ir::Decl::Extern(ir::Extern::Widget(x)) => {
                w.externs.insert(x.name.clone(), x.clone());
            }
            ir::Decl::Extern(ir::Extern::Channel(c)) => {
                for f in &c.values {
                    if let Some(v) = f.default.as_ref().and_then(|d| w.eval(&none, d).ok()) {
                        shared.borrow_mut().externs.insert(format!("{}.{}", c.name, f.name), v);
                    }
                }
            }
            _ => {}
        }
    }
    for d in &m.declarations {
        if let ir::Decl::State(s) = &d.declare {
            for f in &s.fields {
                if let Some(v) = f.default.as_ref().and_then(|d| w.eval(&none, d).ok()) {
                    shared.borrow_mut().state.insert(f.name.clone(), v);
                }
            }
        }
    }
    for d in &m.declarations {
        if let ir::Decl::Let(l) = &d.declare {
            if let Ok(v) = w.eval(&none, &l.value) {
                shared.borrow_mut().lets.insert(l.name.clone(), v);
            }
        }
    }
    let Some(doc) = m.templates.iter().find(|t| t.name == "doc").cloned() else { return Ok(Vec::new()) };
    let pieces = w.nodes(doc.markdown(), &none)?;
    Ok(blocks_of(pieces))
}

