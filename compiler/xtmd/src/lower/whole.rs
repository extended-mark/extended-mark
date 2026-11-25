//! Which numbers are proved whole, and small enough for any engine's own whole numbers: what an operator
//! works on and what it gives, from the contracts of what it reads. The contract of a variable, an
//! input, a record's field, a local and a declared fn's parameter holds whenever it is read, so its
//! range is a fact; an item's, a lambda's parameter's and a handler's isn't taken for one.

use std::collections::HashMap;

use crate::formats::ir::{self, Cond, Decl, Expr, Extern, Get, Node, OpName, Root, Stmt, Type};

/// The whole numbers a double holds exactly end here.
const MOST: i128 = 9_007_199_254_740_991;
/// The most items a list, or a range that is walked, has (page 6).
const ITEMS: i128 = 10_000;

type Range = Option<(i128, i128)>;

fn small(r: (i128, i128)) -> Range {
    (r.0 >= -MOST && r.1 <= MOST).then_some(r)
}

/// What is known where the code is: the types of what it can read, and what its locals hold.
#[derive(Clone, Default)]
struct Scope {
    inputs: HashMap<String, Type>,
    cells: HashMap<String, Type>,
    locals: Vec<(String, Option<Type>, Range)>,
}

impl Scope {
    fn with(&self, name: &str, ty: &Type) -> Scope {
        let mut s = self.clone();
        s.locals.push((name.to_string(), Some(ty.clone()), None));
        s
    }

    fn params(&self, ps: &[ir::Param]) -> Scope {
        let mut s = self.clone();
        s.locals.extend(ps.iter().map(|p| (p.name.clone(), Some(p.ty.clone()), None)));
        s
    }

    /// Parameters whose contracts are not known to hold.
    fn loose(&self, ps: &[ir::Param]) -> Scope {
        let mut s = self.clone();
        s.locals.extend(ps.iter().map(|p| (p.name.clone(), Some(plainly(&p.ty)), None)));
        s
    }
}

struct Types {
    aliases: HashMap<String, Type>,
    records: HashMap<String, ir::Record>,
    state: HashMap<String, Type>,
    externs: HashMap<String, Type>,
    lets: HashMap<String, Type>,
}

pub fn tell(m: &mut ir::Module) {
    let mut t = Types { aliases: HashMap::new(), records: HashMap::new(), state: HashMap::new(), externs: HashMap::new(), lets: HashMap::new() };
    for d in &m.declarations {
        match &d.declare {
            Decl::Type(a) => {
                t.aliases.insert(a.name.clone(), a.is.clone());
            }
            Decl::Record(r) => {
                t.records.insert(r.name.clone(), r.clone());
            }
            Decl::State(s) => t.state.extend(s.fields.iter().map(|f| (f.name.clone(), f.ty.clone()))),
            Decl::Extern(Extern::Channel(c)) => t.externs.extend(c.values.iter().map(|v| (format!("{}.{}", c.name, v.name), v.ty.clone()))),
            Decl::Let(l) => {
                t.lets.insert(l.name.clone(), l.ty.clone());
            }
            _ => {}
        }
    }
    let none = Scope::default();
    for d in &mut m.declarations {
        match &mut d.declare {
            Decl::Fn(f) => t.body(&mut f.body, &none.params(&f.params)),
            Decl::Extern(Extern::Fn(f)) => t.body(&mut f.default, &none.params(&f.params)),
            Decl::Record(r) => {
                let mut s = none.clone();
                s.locals.extend(r.fields.iter().map(|f| (f.name.clone(), Some(f.ty.clone()), None)));
                t.rules(&mut r.requires, &s);
                r.fields.iter_mut().filter_map(|f| f.default.as_mut()).for_each(|e| { t.expr(e, &none); });
            }
            Decl::State(s) => t.rules(&mut s.requires, &none),
            Decl::Expose(e) => e.fns.iter_mut().for_each(|f| t.body(&mut f.body, &none.params(&f.params))),
            Decl::Extern(Extern::Widget(w)) => {
                let s = Scope { inputs: typed(&w.inputs), ..Scope::default() };
                t.rules(&mut w.requires, &s);
            }
            _ => {}
        }
    }
    // a handler is written where its call is: its widget's inputs and variables are told by it
    let mut of: HashMap<String, Scope> = HashMap::new();
    for x in &mut m.templates {
        let s = Scope { inputs: typed(&x.inputs), cells: typed(&x.cells), locals: Vec::new() };
        t.rules(&mut x.requires, &s);
        x.fns.iter_mut().for_each(|f| t.body(&mut f.body, &s.params(&f.params)));
        x.cells.iter_mut().chain(x.inputs.iter_mut()).filter_map(|f| f.default.as_mut()).for_each(|e| { t.expr(e, &s); });
        t.nodes(&mut x.view, &s, &mut of);
        if let Some(md) = &mut x.markdown {
            t.nodes(md, &s, &mut of);
        }
    }
    for h in &mut m.handlers {
        let s = of.get(&h.id).cloned().unwrap_or_default();
        t.body(&mut h.body, &s.loose(&h.params));
    }
}

/// A type, its own contract apart.
fn plainly(t: &Type) -> Type {
    Type { contract: None, ..t.clone() }
}

fn typed(fs: &[ir::Field]) -> HashMap<String, Type> {
    fs.iter().map(|f| (f.name.clone(), f.ty.clone())).collect()
}

/// A bound that is a plain number: the whole numbers inside it.
fn bound(e: &Option<Expr>, low: bool) -> Option<i128> {
    let Some(Expr::Num(n)) = e else { return None };
    let (sign, digits) = match n.strip_prefix('-') {
        Some(d) => (-1, d),
        None => (1, n.as_str()),
    };
    let (whole, part) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.len() > 17 || !whole.bytes().chain(part.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let w = sign * whole.parse::<i128>().ok()?;
    let exact = part.bytes().all(|b| b == b'0');
    Some(match (exact, low, sign) {
        (true, ..) => w,
        (false, true, 1) => w + 1,
        (false, false, -1) => w - 1,
        _ => w,
    })
}

impl Types {
    /// The whole numbers a type holds.
    fn of(&self, t: &Type) -> Range {
        let (mut low, mut high, mut whole) = (None::<i128>, None::<i128>, false);
        let mut t = t.clone();
        for _ in 0..16 {
            if let Some(c) = &t.contract {
                if let Some(r) = &c.range {
                    low = low.max(bound(&r.low, true));
                    high = match (high, bound(&r.high, false)) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (a, b) => a.or(b),
                    };
                }
                // whole steps from a whole start, which is the range's low end, or zero
                if let Some(Expr::Num(s)) = &c.step {
                    let start = c.range.as_ref().and_then(|r| r.low.as_ref());
                    whole |= !s.contains('.') && !s.contains('e') && start.map_or(true, |x| matches!(x, Expr::Num(n) if !n.contains('.') && !n.contains('e')));
                }
            }
            if t.name == "core/int" {
                whole = true;
            }
            match self.aliases.get(&t.name) {
                Some(a) => t = a.clone(),
                None => break,
            }
        }
        match (t.name.as_str(), low, high) {
            ("core/number" | "core/int", Some(l), Some(h)) if whole && l <= h => small((l, h)),
            _ => None,
        }
    }

    fn plain(&self, t: &Type) -> Type {
        let mut t = t.clone();
        for _ in 0..16 {
            match self.aliases.get(&t.name) {
                Some(a) => t = a.clone(),
                None => break,
            }
        }
        t
    }

    fn field(&self, t: &Type, name: &str) -> Option<Type> {
        let t = self.plain(t);
        if !t.fields.is_empty() {
            return t.fields.iter().find(|f| f.name == name).map(|f| f.ty.clone());
        }
        let r = self.records.get(&t.name)?;
        let f = r.fields.iter().find(|f| f.name == name)?;
        match r.params.iter().position(|p| p.name == f.ty.name) {
            Some(k) => t.args.get(k).cloned(),
            None if r.params.is_empty() => Some(f.ty.clone()),
            None => None,
        }
    }

    fn type_of(&self, e: &Expr, s: &Scope) -> Option<Type> {
        match e {
            Expr::Get(Get::Local(n)) => s.locals.iter().rev().find(|(k, ..)| k == n)?.1.clone(),
            Expr::Get(Get::Input(n)) => s.inputs.get(n).cloned(),
            Expr::Get(Get::Cell(n)) => s.cells.get(n).cloned(),
            Expr::Get(Get::State(n)) => self.state.get(n).cloned(),
            Expr::Get(Get::Extern(n)) => self.externs.get(n).cloned(),
            Expr::Get(Get::Let(n)) => self.lets.get(n).cloned(),
            Expr::Field(f) => self.field(&self.type_of(&f.value, s)?, &f.name),
            Expr::Place(p) => {
                let root = match &p.root {
                    Root::Local(n) => Get::Local(n.clone()),
                    Root::Input(n) => Get::Input(n.clone()),
                    Root::Cell(n) => Get::Cell(n.clone()),
                    Root::State(n) => Get::State(n.clone()),
                };
                let mut t = self.type_of(&Expr::Get(root), s)?;
                for step in &p.path {
                    t = match step {
                        ir::Step::Field(f) => self.field(&t, &f.name)?,
                        ir::Step::Item(_) => plainly(&self.plain(self.plain(&t).args.first()?)),
                    };
                }
                Some(t)
            }
            Expr::Call(c) => c.ty.clone(),
            Expr::Op(o) => o.ty.clone(),
            Expr::Apply(a) => a.ty.clone(),
            _ => None,
        }
    }

    /// The whole numbers an expression may give, every operator in it told as it is met.
    fn expr(&self, e: &mut Expr, s: &Scope) -> Range {
        match e {
            Expr::Num(n) if !n.contains('e') && !n.contains('.') => n.parse::<i128>().ok().and_then(|n| small((n, n))),
            Expr::Num(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Duration(_) | Expr::None(_) | Expr::Token(_) => None,
            Expr::Get(g) => {
                let held = match g {
                    Get::Local(n) => s.locals.iter().rev().find(|(k, ..)| k == n).and_then(|(.., r)| *r),
                    _ => None,
                };
                let typed = self.type_of(&Expr::Get(g.clone()), s).and_then(|t| self.of(&t));
                meet(held, typed)
            }
            Expr::Field(f) => {
                self.expr(&mut f.value, s);
                let f = f.clone();
                self.type_of(&Expr::Field(f), s).and_then(|t| self.of(&t))
            }
            Expr::Place(p) => {
                for x in &mut p.path {
                    if let ir::Step::Item(i) = x {
                        self.expr(&mut i.at, s);
                    }
                }
                self.type_of(&Expr::Place(p.clone()), s).and_then(|t| self.of(&t))
            }
            Expr::Call(c) => {
                let mut args: Vec<Range> = Vec::new();
                for x in &mut c.args {
                    args.push(self.expr(x, s));
                }
                match (c.name.as_str(), args.as_slice()) {
                    ("core/list.len", _) => Some((0, ITEMS)),
                    ("core/ordered.clamp", [Some(_), Some(l), Some(h)]) if l.1 <= h.0 => Some((l.0, h.1)),
                    _ => None,
                }
            }
            Expr::Op(o) => {
                let args: Vec<Range> = o.args.iter_mut().map(|x| self.expr(x, s)).collect();
                if o.name == OpName::Pick {
                    o.whole = false;
                    return match args.as_slice() {
                        [_, Some(a), Some(b)] => Some((a.0.min(b.0), a.1.max(b.1))),
                        _ => None,
                    };
                }
                let out = match (o.name, args.as_slice()) {
                    (OpName::Minus, [Some(a)]) => small((-a.1, -a.0)),
                    (OpName::Plus, [Some(a), Some(b)]) => small((a.0 + b.0, a.1 + b.1)),
                    (OpName::Minus, [Some(a), Some(b)]) => small((a.0 - b.1, a.1 - b.0)),
                    (OpName::Times, [Some(a), Some(b)]) => {
                        let ends = [a.0 * b.0, a.0 * b.1, a.1 * b.0, a.1 * b.1];
                        small((*ends.iter().min().unwrap(), *ends.iter().max().unwrap()))
                    }
                    // never by zero, and no further from zero than either
                    (OpName::Rem, [Some(_), Some(b)]) if b.0 > 0 || b.1 < 0 => {
                        let most = b.0.abs().max(b.1.abs());
                        Some((-most, most))
                    }
                    _ => None,
                };
                let compares = matches!(o.name, OpName::Eq | OpName::Ne | OpName::Lt | OpName::Le | OpName::Gt | OpName::Ge);
                o.whole = out.is_some() || (compares && args.len() == 2 && args.iter().all(Option::is_some));
                out
            }
            Expr::List(xs) => {
                xs.iter_mut().for_each(|x| { self.expr(x, s); });
                None
            }
            Expr::Record(r) => {
                r.fields.values_mut().for_each(|x| { self.expr(x, s); });
                None
            }
            Expr::Case(c) => {
                c.values.values_mut().for_each(|x| { self.expr(x, s); });
                None
            }
            Expr::Some(x) | Expr::Sentence(x) => {
                self.expr(x, s);
                None
            }
            Expr::Range(r) => {
                for x in [&mut r.low, &mut r.high, &mut r.step].into_iter().flatten() {
                    self.expr(x, s);
                }
                None
            }
            Expr::Tokval(t) => {
                self.expr(&mut t.value, s);
                None
            }
            Expr::Template(t) => {
                t.holes.iter_mut().for_each(|x| { self.expr(x, s); });
                None
            }
            Expr::Rich(r) => {
                let nodes = std::sync::Arc::make_mut(&mut r.nodes);
                self.nodes(nodes, s, &mut HashMap::new());
                None
            }
            Expr::View(v) => {
                v.args.values_mut().for_each(|x| { self.expr(x, s); });
                v.slots.values_mut().for_each(|ns| self.nodes(ns, s, &mut HashMap::new()));
                None
            }
            Expr::Apply(a) => {
                self.expr(&mut a.fn_, s);
                a.args.iter_mut().for_each(|x| { self.expr(x, s); });
                None
            }
            Expr::Show(x) => {
                self.expr(&mut x.value, s);
                None
            }
            Expr::Fn(l) => {
                let inner = s.loose(&l.params);
                self.body(&mut l.body, &inner);
                None
            }
            Expr::Try(t) => {
                self.expr(&mut t.value, s);
                None
            }
            Expr::With(w) => {
                self.expr(&mut w.value, s);
                w.fields.values_mut().for_each(|x| { self.expr(x, s); });
                None
            }
        }
    }

    /// The rows of a loop: what its variable holds, from its type or from the range it walks.
    fn rows(&self, over: &mut Expr, s: &Scope) -> Range {
        self.expr(over, s);
        let Expr::Range(r) = over else { return None };
        let whole_step = r.step.as_ref().map_or(true, |x| matches!(x, Expr::Num(n) if !n.contains('.') && !n.contains('e')));
        let low = r.low.as_mut().and_then(|x| self.expr(x, s))?;
        let high = r.high.as_mut().and_then(|x| self.expr(x, s))?;
        (whole_step).then_some((low.0.min(high.0), low.1.max(high.1)))
    }

    fn looped(&self, s: &Scope, name: &str, ty: &Type, position: &Option<String>, mutable: bool, rows: Range) -> Scope {
        let mut inner = s.clone();
        // a `mut for`'s variable is an opt of the item
        let ty = if mutable { Type::of("core/opt", vec![ty.clone()]) } else { plainly(&self.plain(ty)) };
        inner.locals.push((name.to_string(), Some(ty), rows));
        if let Some(p) = position {
            inner.locals.push((p.clone(), None, Some((0, ITEMS))));
        }
        inner
    }

    fn cond(&self, c: &mut Cond, s: &Scope) -> Scope {
        match c {
            Cond::Cond(e) => {
                self.expr(e, s);
                s.clone()
            }
            Cond::Open(o) => {
                self.expr(&mut o.opt, s);
                s.with(&o.name, &o.ty)
            }
        }
    }

    fn rules(&self, rs: &mut [ir::Require], s: &Scope) {
        rs.iter_mut().for_each(|r| self.body(&mut r.body, s));
    }

    fn body(&self, body: &mut [Stmt], s: &Scope) {
        let mut s = s.clone();
        for x in body {
            match x {
                Stmt::Declare(ir::StmtDeclare::Let(l)) => {
                    let held = self.expr(&mut l.value, &s);
                    s.locals.push((l.name.clone(), Some(l.ty.clone()), held));
                }
                Stmt::Declare(ir::StmtDeclare::Mut(l)) => {
                    self.expr(&mut l.value, &s);
                    s = s.with(&l.name, &l.ty);
                }
                Stmt::Set(x) => {
                    self.expr(&mut x.value, &s);
                    for step in &mut x.place.path {
                        if let ir::Step::Item(i) = step {
                            self.expr(&mut i.at, &s);
                        }
                    }
                }
                Stmt::Return(r) => r.value.iter_mut().for_each(|e| { self.expr(e, &s); }),
                Stmt::If(i) => {
                    for b in &mut i.branches {
                        let inner = self.cond(&mut b.when, &s);
                        self.body(&mut b.then, &inner);
                    }
                    self.body(&mut i.else_, &s);
                }
                Stmt::For(f) => {
                    let rows = self.rows(&mut f.over, &s);
                    let inner = self.looped(&s, &f.name, &f.ty, &f.position, f.mutable, rows);
                    self.body(&mut f.body, &inner);
                }
                Stmt::Match(m) => {
                    self.expr(&mut m.value, &s);
                    for c in &mut m.cases {
                        self.body(&mut c.then, &bound_by(&c.when, &s));
                    }
                    self.body(&mut m.else_, &s);
                }
                Stmt::Invoke(i) => i.args.iter_mut().for_each(|e| { self.expr(e, &s); }),
                Stmt::Call(c) => c.args.iter_mut().for_each(|e| { self.expr(e, &s); }),
                Stmt::Apply(a) => {
                    self.expr(&mut a.fn_, &s);
                    a.args.iter_mut().for_each(|e| { self.expr(e, &s); });
                }
            }
        }
    }

    fn nodes(&self, ns: &mut [Node], s: &Scope, of: &mut HashMap<String, Scope>) {
        let mut s = s.clone();
        for n in ns {
            match n {
                Node::Extern(x) => {
                    x.props.values_mut().for_each(|e| { self.expr(e, &s); });
                    x.slots.values_mut().for_each(|k| self.nodes(k, &s, of));
                    x.events.values().for_each(|h| drop(of.insert(h.clone(), s.clone())));
                }
                Node::Use(u) => {
                    u.args.values_mut().for_each(|e| { self.expr(e, &s); });
                    u.slots.values_mut().for_each(|k| self.nodes(k, &s, of));
                    u.events.values().for_each(|h| drop(of.insert(h.clone(), s.clone())));
                }
                Node::For(f) => {
                    let rows = self.rows(&mut f.over, &s);
                    let inner = self.looped(&s, &f.name, &f.ty, &f.position, f.mutable, rows);
                    self.nodes(&mut f.body, &inner, of);
                }
                Node::If(i) => {
                    for b in &mut i.branches {
                        let inner = self.cond(&mut b.when, &s);
                        self.nodes(&mut b.then, &inner, of);
                    }
                    self.nodes(&mut i.else_, &s, of);
                }
                Node::Match(m) => {
                    self.expr(&mut m.value, &s);
                    for c in &mut m.cases {
                        self.nodes(&mut c.then, &bound_by(&c.when, &s), of);
                    }
                    self.nodes(&mut m.else_, &s, of);
                }
                Node::Place(p) => {
                    self.expr(&mut p.view, &s);
                }
                Node::Declare(d) => {
                    let held = self.expr(&mut d.let_.value, &s);
                    s.locals.push((d.let_.name.clone(), Some(d.let_.ty.clone()), held));
                }
            }
        }
    }
}

/// What a match's patterns bind: nothing is known of them, and they hide what had their names.
fn bound_by(ps: &[ir::Pattern], s: &Scope) -> Scope {
    let mut inner = s.clone();
    for p in ps {
        if let ir::Pattern::Case(c) = p {
            inner.locals.extend(c.binds.iter().map(|b| (b.clone(), None, None)));
        }
    }
    inner
}

/// Both hold.
fn meet(a: Range, b: Range) -> Range {
    match (a, b) {
        (Some(a), Some(b)) => Some((a.0.max(b.0), a.1.min(b.1))),
        (a, b) => a.or(b),
    }
}
