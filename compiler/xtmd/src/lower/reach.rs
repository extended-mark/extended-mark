//! What a page can run: every declaration, case, token and fn its code can reach, followed from the
//! page's own view, `$state` and the fns the host may call.
//!
//! A reference is read where the IR makes one (a call's name, a type, a case), never by searching
//! its strings. The types written in the IR say which `show`, `same` and `before` a page calls; where
//! a value's type can't be known here (a type parameter's), the fn of every type the page holds counts.
//! Sets only grow, so the walk is run again until it finds nothing new.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::formats::ir::{self, Decl, Expr, Extern, Node, Stmt, Type};

/// core's extern fns an implementer is told to build on other fns of core (page 5).
const BUILDS_ON: &[(&str, &[&str])] = &[("core/moment.local", &["core/moment.utc", "core/moment_at"])];
/// core's extern fns that compare what they are given.
const COMPARES: &[(&str, &str)] = &[("core/list.remove", "same"), ("core/list.sort", "before"), ("core/ordered.clamp", "before"), ("core/range.has", "before")];

const SCALARS: [&str; 6] = ["core/number", "core/int", "core/text", "core/bool", "core/duration", "fn"];

#[derive(Clone, Default)]
struct Scope<'m> {
    locals: Vec<(String, Option<Type>)>,
    /// The widget whose inputs, variables and slots are read here.
    inputs: &'m [ir::Field],
    cells: &'m [ir::Field],
}

impl Scope<'_> {
    fn with(&self, name: &str, ty: Option<Type>) -> Self {
        let mut s = self.clone();
        s.locals.push((name.to_string(), ty));
        s
    }
}

struct Reach<'m> {
    m: &'m ir::Module,
    decls: HashMap<&'m str, usize>,
    templates: HashMap<&'m str, usize>,
    /// A widget's own fns, by full name.
    fns: HashMap<&'m str, (&'m ir::Template, &'m ir::FnDecl)>,
    handlers: HashMap<&'m str, &'m ir::Handler>,
    /// A channel's values and fns, by full name: the channel.
    of_channel: HashMap<String, usize>,

    /// What the page can run, by full name: declarations, widgets and their fns, handlers.
    kept: BTreeSet<String>,
    cases: HashSet<(String, String)>,
    /// Types a host or an engine can send in: each of their cases and rules counts.
    whole: HashSet<String>,
    /// Records the page makes as it runs.
    built: HashSet<String>,
    any_built: bool,
    /// Records made with a field left out, which has its default.
    defaulted: HashSet<String>,
    /// Fns called on a value whose type isn't known here.
    any: BTreeSet<String>,
    any_same: bool,
    /// The channels' values and fns the page takes.
    taken: BTreeSet<String>,

    /// What this round has walked.
    seen: HashSet<String>,
    todo: Vec<String>,
}

/// Removes from a page what it can't run.
pub fn prune(m: &mut ir::Module) {
    let found = {
        let mut r = Reach::new(m);
        loop {
            let before = r.size();
            r.round();
            if r.size() == before {
                break;
            }
        }
        Found { kept: r.kept, cases: r.cases, whole: r.whole, built: r.built, defaulted: r.defaulted, any_built: r.any_built, taken: r.taken }
    };
    found.cut(m);
}

impl<'m> Reach<'m> {
    fn new(m: &'m ir::Module) -> Reach<'m> {
        let mut r = Reach {
            m,
            decls: HashMap::new(),
            templates: m.templates.iter().enumerate().map(|(k, t)| (t.name.as_str(), k)).collect(),
            fns: HashMap::new(),
            handlers: m.handlers.iter().map(|h| (h.id.as_str(), h)).collect(),
            of_channel: HashMap::new(),
            kept: BTreeSet::new(),
            cases: HashSet::new(),
            whole: HashSet::new(),
            built: HashSet::new(),
            any_built: false,
            defaulted: HashSet::new(),
            any: BTreeSet::new(),
            any_same: false,
            taken: BTreeSet::new(),
            seen: HashSet::new(),
            todo: Vec::new(),
        };
        for (k, d) in m.declarations.iter().enumerate() {
            if let Some(n) = name_of(&d.declare) {
                r.decls.insert(n, k);
            }
            if let Decl::Extern(Extern::Channel(c)) = &d.declare {
                for n in c.values.iter().map(|v| &v.name).chain(c.fns.iter().map(|f| &f.name)) {
                    r.of_channel.insert(format!("{}.{}", c.name, n), k);
                }
            }
        }
        for t in &m.templates {
            for f in &t.fns {
                r.fns.insert(f.name.as_str(), (t, f));
            }
        }
        r
    }

    fn size(&self) -> usize {
        self.kept.len() + self.cases.len() + self.whole.len() + self.built.len() + self.defaulted.len() + self.any.len() + self.taken.len() + self.any_built as usize + self.any_same as usize
    }

    fn round(&mut self) {
        self.seen.clear();
        self.visit(&self.m.module.clone());
        let m = self.m;
        for d in &m.declarations {
            match &d.declare {
                Decl::State(s) => {
                    for f in &s.fields {
                        self.sent(&f.ty);
                        self.opt(&f.default, &Scope::default());
                    }
                    self.rules(&s.requires, &Scope::default());
                }
                Decl::Expose(e) => {
                    self.visit(&e.channel);
                    for f in &e.fns {
                        for p in &f.params {
                            self.sent(&p.ty);
                        }
                        self.body(&f.body, &params(&Scope::default(), &f.params));
                    }
                }
                // the page's own look: what core's page says it is drawn with
                Decl::Extern(Extern::Channel(c)) => c.tokens.iter().for_each(|t| self.token(t)),
                _ => {}
            }
        }
        while let Some(n) = self.todo.pop() {
            self.walk(&n);
        }
    }

    /// The page can run what this names.
    fn visit(&mut self, name: &str) {
        if self.decls.contains_key(name) || self.templates.contains_key(name) || self.fns.contains_key(name) {
            if self.seen.insert(name.to_string()) {
                self.kept.insert(name.to_string());
                self.todo.push(name.to_string());
            }
        }
    }

    fn walk(&mut self, name: &str) {
        let m = self.m;
        if let Some(&k) = self.templates.get(name) {
            self.template(&m.templates[k]);
        }
        if let Some((t, f)) = self.fns.get(name).copied() {
            let scope = Scope { locals: Vec::new(), inputs: &t.inputs, cells: &t.cells };
            self.fn_decl(f, &scope);
        }
        if let Some(&k) = self.decls.get(name) {
            self.decl(&m.declarations[k].declare);
        }
    }

    fn template(&mut self, t: &'m ir::Template) {
        let scope = Scope { locals: Vec::new(), inputs: &t.inputs, cells: &t.cells };
        for f in &t.inputs {
            self.mention(&f.ty);
            self.opt(&f.default, &scope);
        }
        for f in &t.cells {
            // a page's data puts a widget's own variables back
            self.sent(&f.ty);
            self.opt(&f.default, &scope);
        }
        t.slots.iter().for_each(|s| self.mention(&s.ty));
        t.events.iter().flat_map(|e| &e.params).for_each(|p| self.mention(&p.ty));
        self.rules(&t.requires, &scope);
        self.nodes(&t.view, &scope);
        self.nodes(t.markdown(), &scope);
    }

    fn fn_decl(&mut self, f: &'m ir::FnDecl, scope: &Scope<'m>) {
        for p in &f.params {
            self.mention(&p.ty);
        }
        if let Some(t) = &f.returns {
            self.mention(t);
        }
        self.body(&f.body, &params(scope, &f.params));
    }

    fn decl(&mut self, d: &'m Decl) {
        match d {
            Decl::Record(r) => {
                r.fields.iter().for_each(|f| self.mention(&f.ty));
                if self.makes(&r.name) || self.defaulted.contains(&r.name) {
                    for f in &r.fields {
                        self.opt(&f.default, &Scope::default());
                    }
                }
                if self.makes(&r.name) {
                    let mut scope = Scope::default();
                    for f in &r.fields {
                        scope = scope.with(&f.name, Some(f.ty.clone()));
                    }
                    self.rules(&r.requires, &scope);
                }
                if self.any_same {
                    if let Some(s) = &r.same {
                        self.visit(s);
                    }
                }
                self.methods(&r.name);
            }
            Decl::Enum(e) => {
                for c in &e.cases {
                    if self.made(&e.name, &c.name) {
                        c.fields.iter().for_each(|f| self.mention(&f.ty));
                    }
                }
                self.methods(&e.name);
            }
            Decl::Token(t) => {
                self.mention(&t.value);
                for c in &t.cases {
                    if self.made(&t.name, &c.name) {
                        c.fields.iter().for_each(|f| self.mention(&f.ty));
                        if let Some(v) = t.values.get(&c.name) {
                            self.expr(v, &Scope::default(), true);
                        }
                    }
                }
                self.methods(&t.name);
            }
            Decl::Interface(i) => {
                i.fields.iter().for_each(|f| self.mention(&f.ty));
                i.events.iter().flat_map(|e| &e.params).for_each(|p| self.mention(&p.ty));
            }
            Decl::Fn(f) => self.fn_decl(f, &Scope::default()),
            Decl::Extern(Extern::Fn(f)) => {
                f.params.iter().for_each(|p| self.mention(&p.ty));
                if let Some(t) = &f.returns {
                    self.sent(t);
                }
                self.body(&f.default, &params(&Scope::default(), &f.params));
                for n in BUILDS_ON.iter().filter(|(n, _)| *n == f.name).flat_map(|(_, on)| on.iter()) {
                    self.visit(n);
                }
            }
            Decl::Extern(Extern::Widget(w)) => {
                let scope = Scope { locals: Vec::new(), inputs: &w.inputs, cells: &[] };
                for f in &w.inputs {
                    if f.mutable {
                        self.sent(&f.ty);
                    } else {
                        self.mention(&f.ty);
                    }
                    self.opt(&f.default, &scope);
                }
                w.slots.iter().for_each(|s| self.mention(&s.ty));
                w.events.iter().flat_map(|e| &e.params).for_each(|p| self.sent(&p.ty));
                w.tokens.iter().for_each(|t| self.token(t));
                self.rules(&w.requires, &scope);
                self.nodes(&w.markdown, &scope);
            }
            Decl::Extern(Extern::Channel(c)) => {
                for v in &c.values {
                    if self.taken.contains(&format!("{}.{}", c.name, v.name)) {
                        self.sent(&v.ty);
                        self.opt(&v.default, &Scope::default());
                    }
                }
                for f in &c.fns {
                    if self.taken.contains(&format!("{}.{}", c.name, f.name)) {
                        f.params.iter().for_each(|p| self.mention(&p.ty));
                        if let Some(t) = &f.returns {
                            self.sent(t);
                        }
                    }
                }
            }
            Decl::Extern(Extern::Type(t)) => self.methods(&t.name),
            Decl::Let(l) => {
                self.mention(&l.ty);
                self.expr(&l.value, &Scope::default(), true);
            }
            Decl::Type(t) => self.mention(&t.is),
            Decl::State(_) | Decl::Expose(_) => {}
        }
    }

    /// The fns called on values of a type not known where they are called, of a type the page holds.
    fn methods(&mut self, ty: &str) {
        for f in self.any.clone() {
            self.visit(&format!("{}.{}", ty, f));
        }
    }

    fn made(&self, ty: &str, case: &str) -> bool {
        self.whole.contains(ty) || self.cases.contains(&(ty.to_string(), case.to_string()))
    }

    fn makes(&self, record: &str) -> bool {
        self.any_built || self.built.contains(record) || self.whole.contains(record)
    }

    fn case(&mut self, ty: &str, case: &str) {
        self.visit(ty);
        self.cases.insert((ty.to_string(), case.to_string()));
    }

    /// A token by its full name: `core/space.S`.
    fn token(&mut self, name: &str) {
        if let Some((ty, case)) = name.rsplit_once('.') {
            self.case(ty, case);
        }
    }

    fn mention(&mut self, t: &'m Type) {
        self.visit(&t.name);
        t.args.iter().for_each(|a| self.mention(a));
        t.params.iter().for_each(|p| self.mention(&p.ty));
        t.fields.iter().for_each(|f| self.mention(&f.ty));
        if let Some(r) = &t.returns {
            self.mention(r);
        }
        if let Some(i) = &t.interface {
            self.visit(i);
        }
        if let Some(c) = &t.contract {
            for b in [&c.range, &c.length].into_iter().flatten() {
                for e in [&b.low, &b.high].into_iter().flatten() {
                    self.expr(e, &Scope::default(), true);
                }
            }
            if let Some(e) = &c.step {
                self.expr(e, &Scope::default(), true);
            }
            let plain = Type { contract: None, ..t.clone() };
            if c.range.is_some() {
                self.compare(Some(&plain), "before", 0);
            }
            if !c.only.is_empty() {
                c.only.iter().for_each(|e| self.expr(e, &Scope::default(), true));
                self.compare(Some(&plain), "same", 0);
            }
        }
    }

    /// A value of this type can come from outside the page.
    fn sent(&mut self, t: &'m Type) {
        self.mention(t);
        t.args.iter().for_each(|a| self.sent(a));
        t.fields.iter().for_each(|f| self.sent(&f.ty));
        let Some(&k) = self.decls.get(t.name.as_str()) else { return };
        if !self.whole.insert(t.name.clone()) && self.seen.contains(&format!("sent {}", t.name)) {
            return;
        }
        self.seen.insert(format!("sent {}", t.name));
        let m = self.m;
        match &m.declarations[k].declare {
            Decl::Record(r) => r.fields.iter().for_each(|f| self.sent(&f.ty)),
            Decl::Enum(e) => e.cases.iter().flat_map(|c| &c.fields).for_each(|f| self.sent(&f.ty)),
            Decl::Token(x) => x.cases.iter().flat_map(|c| &c.fields).for_each(|f| self.sent(&f.ty)),
            Decl::Type(a) => self.sent(&a.is),
            _ => {}
        }
    }

    fn rules(&mut self, rules: &'m [ir::Require], scope: &Scope<'m>) {
        rules.iter().for_each(|r| self.body(&r.body, scope));
    }

    fn opt(&mut self, e: &'m Option<Expr>, scope: &Scope<'m>) {
        if let Some(e) = e {
            self.expr(e, scope, true);
        }
    }

    // ---- types

    /// A type's declaration behind its aliases.
    fn plain(&self, t: &Type) -> Type {
        let mut t = t.clone();
        for _ in 0..16 {
            match self.decls.get(t.name.as_str()).map(|&k| &self.m.declarations[k].declare) {
                Some(Decl::Type(a)) => t = a.is.clone(),
                _ => break,
            }
        }
        t
    }

    fn record(&self, name: &str) -> Option<&'m ir::Record> {
        match self.decls.get(name).map(|&k| &self.m.declarations[k].declare) {
            Some(Decl::Record(r)) => Some(r),
            _ => None,
        }
    }

    /// A field's type, a generic record's arguments put in.
    fn field_type(&self, t: &Type, name: &str) -> Option<Type> {
        let t = self.plain(t);
        if !t.fields.is_empty() {
            return t.fields.iter().find(|f| f.name == name).map(|f| f.ty.clone());
        }
        if let Some(Decl::Token(x)) = self.decls.get(t.name.as_str()).map(|&k| &self.m.declarations[k].declare) {
            return self.field_type(&x.value, name);
        }
        let r = self.record(&t.name)?;
        let f = r.fields.iter().find(|f| f.name == name)?;
        match r.params.iter().position(|p| p.name == f.ty.name) {
            Some(k) => t.args.get(k).cloned(),
            None if r.params.is_empty() => Some(f.ty.clone()),
            None => None,
        }
    }

    fn type_of(&self, e: &Expr, scope: &Scope<'m>) -> Option<Type> {
        let named = |n: &str| Some(Type::named(n));
        match e {
            Expr::Num(_) => named("core/number"),
            Expr::Text(_) | Expr::Show(_) => named("core/text"),
            Expr::Bool(_) | Expr::Sentence(_) => named("core/bool"),
            Expr::Duration(_) => named("core/duration"),
            Expr::List(xs) => Some(Type::of("core/list", vec![self.type_of(xs.first()?, scope)?])),
            Expr::Record(r) => Some(Type::named(&r.ty)).filter(|t| self.record(&t.name).map_or(false, |r| r.params.is_empty())),
            Expr::Case(c) => named(&c.enum_),
            Expr::Some(x) => Some(Type::of("core/opt", vec![self.type_of(x, scope)?])),
            Expr::Token(t) | Expr::Get(ir::Get::Token(t)) => named(t.rsplit_once('.')?.0),
            Expr::Tokval(t) => named(&t.ty),
            Expr::Rich(_) => named("core/rich_text"),
            Expr::Template(t) => Some(t.ty.clone()),
            Expr::Get(g) => match g {
                ir::Get::Local(n) => scope.locals.iter().rev().find(|(k, _)| k == n).and_then(|(_, t)| t.clone()),
                ir::Get::Input(n) => scope.inputs.iter().find(|f| f.name == *n).map(|f| f.ty.clone()),
                ir::Get::Cell(n) => scope.cells.iter().find(|f| f.name == *n).map(|f| f.ty.clone()),
                ir::Get::State(n) => self.m.declarations.iter().find_map(|d| if let Decl::State(s) = &d.declare { s.fields.iter().find(|f| f.name == *n).map(|f| f.ty.clone()) } else { None }),
                ir::Get::Let(n) => match &self.m.declarations[*self.decls.get(n.as_str())?].declare {
                    Decl::Let(l) => Some(l.ty.clone()),
                    _ => None,
                },
                ir::Get::Extern(n) => match &self.m.declarations[*self.of_channel.get(n)?].declare {
                    Decl::Extern(Extern::Channel(c)) => c.values.iter().find(|v| n.ends_with(&format!(".{}", v.name))).map(|v| v.ty.clone()),
                    _ => None,
                },
                _ => None,
            },
            Expr::Field(f) => self.field_type(&self.type_of(&f.value, scope)?, &f.name),
            Expr::Call(c) => c.ty.clone(),
            Expr::Apply(a) => a.ty.clone(),
            Expr::Op(o) => o.ty.clone(),
            Expr::With(w) => self.type_of(&w.value, scope),
            Expr::Place(p) => self.place_type(p, scope),
            _ => None,
        }
    }

    fn place_type(&self, p: &ir::Place, scope: &Scope<'m>) -> Option<Type> {
        let root = match &p.root {
            ir::Root::Local(n) => ir::Get::Local(n.clone()),
            ir::Root::Input(n) => ir::Get::Input(n.clone()),
            ir::Root::Cell(n) => ir::Get::Cell(n.clone()),
            ir::Root::State(n) => ir::Get::State(n.clone()),
        };
        let mut t = self.type_of(&Expr::Get(root), scope)?;
        for s in &p.path {
            t = match s {
                ir::Step::Field(f) => self.field_type(&t, &f.name)?,
                ir::Step::Item(_) => self.plain(&t).args.first()?.clone(),
            };
        }
        Some(t)
    }

    /// A fn called on a value: the one of its type, or of every type when that isn't known.
    fn method(&mut self, t: Option<&Type>, f: &str) {
        match t.map(|t| self.plain(t)) {
            Some(t) if self.decls.contains_key(t.name.as_str()) || t.name.starts_with("core/") => {
                self.visit(&format!("{}.{}", t.name, f));
            }
            _ => {
                self.any.insert(f.to_string());
            }
        }
    }

    /// A value shown as text.
    fn show(&mut self, t: Option<&Type>) {
        match t.map(|t| self.plain(t)) {
            Some(t) if SCALARS.contains(&t.name.as_str()) => {}
            Some(t) if t.name == "core/opt" => self.show(t.args.first()),
            t => self.method(t.as_ref(), "show"),
        }
    }

    /// Two values compared: with `same` (`==`) or `before` (`<`).
    fn compare(&mut self, t: Option<&Type>, how: &str, depth: usize) {
        let Some(t) = t.map(|t| self.plain(t)).filter(|_| depth < 12) else {
            self.any_same |= how == "same";
            self.any.insert(how.to_string());
            return;
        };
        if SCALARS.contains(&t.name.as_str()) {
            return;
        }
        if how == "before" {
            return self.method(Some(&t), how);
        }
        t.args.iter().for_each(|a| self.compare(Some(a), how, depth + 1));
        t.fields.iter().for_each(|f| self.compare(Some(&f.ty), how, depth + 1));
        match self.decls.get(t.name.as_str()).map(|&k| &self.m.declarations[k].declare) {
            Some(Decl::Record(r)) => {
                if let Some(s) = &r.same {
                    self.visit(s);
                } else if r.params.is_empty() {
                    r.fields.iter().for_each(|f| self.compare(Some(&f.ty), how, depth + 1));
                }
            }
            Some(Decl::Enum(e)) if e.params.is_empty() => e.cases.iter().flat_map(|c| &c.fields).for_each(|f| self.compare(Some(&f.ty), how, depth + 1)),
            Some(_) => {}
            // a type parameter, or any
            None if !t.name.starts_with("core/") => self.compare(None, how, 0),
            None => {}
        }
    }

    // ---- code

    fn expr(&mut self, e: &'m Expr, scope: &Scope<'m>, constant: bool) {
        // a constant record was made when the page was compiled, and its rules were run then
        let constant = constant && is_constant(e);
        match e {
            Expr::Num(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Duration(_) | Expr::None(_) => {}
            Expr::List(xs) => xs.iter().for_each(|x| self.expr(x, scope, constant)),
            Expr::Record(r) => {
                self.visit(&r.ty);
                if !constant {
                    self.built.insert(r.ty.clone());
                }
                if self.record(&r.ty).map_or(false, |d| d.fields.iter().any(|f| f.default.is_some() && !r.fields.contains_key(&f.name))) {
                    self.defaulted.insert(r.ty.clone());
                }
                r.fields.values().for_each(|x| self.expr(x, scope, constant));
            }
            Expr::Case(c) => {
                self.case(&c.enum_, &c.name);
                c.values.values().for_each(|x| self.expr(x, scope, constant));
            }
            Expr::Some(x) | Expr::Sentence(x) => self.expr(x, scope, constant),
            Expr::Range(r) => [&r.low, &r.high, &r.step].into_iter().flatten().for_each(|x| self.expr(x, scope, constant)),
            Expr::Token(t) => self.token(t),
            Expr::Tokval(t) => {
                self.case(&t.ty, &t.name);
                self.expr(&t.value, scope, constant);
            }
            Expr::Rich(r) => self.nodes(&r.nodes, scope),
            Expr::Template(t) => {
                t.holes.iter().for_each(|x| self.expr(x, scope, constant));
                if t.ty.name != "core/text" {
                    // its pieces and its holes, as core's record of them
                    self.mention(&t.ty);
                    if self.record(&t.ty.name).is_some() {
                        self.built.insert(t.ty.name.clone());
                    }
                    self.visit("core/hole");
                    self.whole.insert("core/hole".into());
                }
            }
            Expr::View(v) => {
                self.visit(&v.widget);
                v.args.values().for_each(|x| self.expr(x, scope, false));
                v.slots.values().for_each(|n| self.nodes(n, scope));
                self.events(&v.events, scope);
            }
            Expr::Get(g) => match g {
                ir::Get::Token(t) => self.token(t),
                ir::Get::Let(n) => self.visit(n),
                ir::Get::Extern(n) => self.take(n),
                _ => {}
            },
            Expr::Field(f) => self.expr(&f.value, scope, false),
            Expr::Call(c) => self.call(c, scope),
            Expr::Apply(a) => {
                self.expr(&a.fn_, scope, false);
                a.args.iter().for_each(|x| self.expr(x, scope, false));
            }
            Expr::Op(o) => {
                o.args.iter().for_each(|x| self.expr(x, scope, false));
                let t = o.args.iter().find_map(|x| self.type_of(x, scope));
                match o.name {
                    ir::OpName::Eq | ir::OpName::Ne => self.compare(t.as_ref(), "same", 0),
                    ir::OpName::Lt | ir::OpName::Le | ir::OpName::Gt | ir::OpName::Ge => self.compare(t.as_ref(), "before", 0),
                    _ => {}
                }
            }
            Expr::Show(s) => {
                self.expr(&s.value, scope, false);
                let t = self.type_of(&s.value, scope);
                self.show(t.as_ref());
            }
            Expr::Fn(l) => {
                l.params.iter().for_each(|p| self.mention(&p.ty));
                if let Some(t) = &l.returns {
                    self.mention(t);
                }
                self.body(&l.body, &params(scope, &l.params));
            }
            Expr::Place(p) => self.place(p, scope, false),
            Expr::Try(t) => self.expr(&t.value, scope, false),
            Expr::With(w) => {
                self.expr(&w.value, scope, false);
                w.fields.values().for_each(|x| self.expr(x, scope, false));
                match self.type_of(&w.value, scope).map(|t| self.plain(&t)) {
                    Some(t) if self.record(&t.name).is_some() => {
                        self.built.insert(t.name);
                    }
                    _ => self.any_built = true,
                }
            }
        }
    }

    /// A value or a fn of a channel.
    fn take(&mut self, name: &str) {
        if let Some(&k) = self.of_channel.get(name) {
            self.taken.insert(name.to_string());
            if let Some(n) = name_of(&self.m.declarations[k].declare) {
                self.visit(n);
            }
        }
    }

    fn call(&mut self, c: &'m ir::Call, scope: &Scope<'m>) {
        c.args.iter().for_each(|x| self.expr(x, scope, false));
        let first = c.args.first().and_then(|x| self.type_of(x, scope));
        if let Some((_, how)) = COMPARES.iter().find(|(n, _)| *n == c.name) {
            // a list's items, a range's ends
            let t = first.as_ref().map(|t| self.plain(t)).and_then(|t| if matches!(t.name.as_str(), "core/list" | "core/range") { t.args.first().cloned() } else { Some(t) });
            self.compare(t.as_ref(), how, 0);
        }
        if self.of_channel.contains_key(&c.name) {
            return self.take(&c.name);
        }
        if let Some((i, f)) = c.name.rsplit_once('.') {
            if matches!(self.decls.get(i).map(|&k| &self.m.declarations[k].declare), Some(Decl::Interface(_))) && !self.decls.contains_key(c.name.as_str()) {
                self.visit(i);
                match (i, f) {
                    ("core/same", "same") => self.compare(first.as_ref(), "same", 0),
                    ("core/ordered", "before") => self.compare(first.as_ref(), "before", 0),
                    _ => {}
                }
                return self.method(first.as_ref(), f);
            }
        }
        self.visit(&c.name);
    }

    /// A place read, or written: a record written into has its rules run.
    fn place(&mut self, p: &'m ir::Place, scope: &Scope<'m>, written: bool) {
        for s in &p.path {
            if let ir::Step::Item(i) = s {
                self.expr(&i.at, scope, false);
            }
        }
        if written && p.path.iter().any(|s| matches!(s, ir::Step::Field(_))) {
            for k in 0..p.path.len() {
                let outer = ir::Place { root: p.root.clone(), path: p.path[..k].to_vec() };
                match self.place_type(&outer, scope).map(|t| self.plain(&t)) {
                    Some(t) => {
                        self.built.insert(t.name);
                    }
                    None => self.any_built = true,
                }
            }
        }
    }

    fn body(&mut self, body: &'m [Stmt], scope: &Scope<'m>) {
        let mut scope = scope.clone();
        for s in body {
            match s {
                Stmt::Declare(ir::StmtDeclare::Let(l)) | Stmt::Declare(ir::StmtDeclare::Mut(l)) => {
                    self.mention(&l.ty);
                    self.expr(&l.value, &scope, false);
                    scope = scope.with(&l.name, Some(l.ty.clone()));
                }
                Stmt::Set(x) => {
                    self.place(&x.place, &scope, true);
                    self.expr(&x.value, &scope, false);
                }
                Stmt::Return(r) => {
                    if let Some(e) = &r.value {
                        self.expr(e, &scope, false);
                    }
                }
                Stmt::If(i) => {
                    for b in &i.branches {
                        let inner = self.cond(&b.when, &scope);
                        self.body(&b.then, &inner);
                    }
                    self.body(&i.else_, &scope);
                }
                Stmt::For(f) => {
                    self.mention(&f.ty);
                    self.expr(&f.over, &scope, false);
                    let mut inner = scope.with(&f.name, Some(f.ty.clone()));
                    if let Some(p) = &f.position {
                        inner = inner.with(p, Some(Type::named("core/int")));
                    }
                    self.body(&f.body, &inner);
                }
                Stmt::Match(m) => {
                    self.expr(&m.value, &scope, false);
                    for c in &m.cases {
                        if let Some(inner) = self.patterns(&c.when, &scope) {
                            self.body(&c.then, &inner);
                        }
                    }
                    self.body(&m.else_, &scope);
                }
                Stmt::Invoke(i) => i.args.iter().for_each(|x| self.expr(x, &scope, false)),
                Stmt::Call(c) => self.call(c, &scope),
                Stmt::Apply(a) => {
                    self.expr(&a.fn_, &scope, false);
                    a.args.iter().for_each(|x| self.expr(x, &scope, false));
                }
            }
        }
    }

    fn cond(&mut self, c: &'m ir::Cond, scope: &Scope<'m>) -> Scope<'m> {
        match c {
            ir::Cond::Cond(e) => {
                self.expr(e, scope, false);
                scope.clone()
            }
            ir::Cond::Open(o) => {
                self.mention(&o.ty);
                self.expr(&o.opt, scope, false);
                scope.with(&o.name, Some(o.ty.clone()))
            }
        }
    }

    /// The scope a match's arm runs in, or none when nothing the page makes gets there.
    fn patterns(&self, when: &[ir::Pattern], scope: &Scope<'m>) -> Option<Scope<'m>> {
        let mut inner = scope.clone();
        let mut reached = false;
        for p in when {
            match p {
                ir::Pattern::Case(c) => {
                    if !self.made(&c.enum_, &c.name) && self.decls.contains_key(c.enum_.as_str()) {
                        continue;
                    }
                    let fields: &[ir::Field] = match self.decls.get(c.enum_.as_str()).map(|&k| &self.m.declarations[k].declare) {
                        Some(Decl::Enum(e)) if e.params.is_empty() => e.cases.iter().find(|x| x.name == c.name).map_or(&[], |x| &x.fields),
                        _ => &[],
                    };
                    for (k, b) in c.binds.iter().enumerate() {
                        inner.locals.push((b.clone(), fields.get(k).map(|f| f.ty.clone())));
                    }
                    reached = true;
                }
                _ => reached = true,
            }
        }
        reached.then_some(inner)
    }

    fn events(&mut self, events: &'m BTreeMap<String, String>, scope: &Scope<'m>) {
        for id in events.values() {
            let Some(h) = self.handlers.get(id.as_str()).copied() else { continue };
            self.kept.insert(id.clone());
            h.params.iter().for_each(|p| self.mention(&p.ty));
            self.body(&h.body, &params(scope, &h.params));
        }
    }

    fn nodes(&mut self, nodes: &'m [Node], scope: &Scope<'m>) {
        let mut scope = scope.clone();
        for n in nodes {
            match n {
                Node::Extern(x) => {
                    self.visit(&x.name);
                    x.props.values().for_each(|e| self.expr(e, &scope, true));
                    x.slots.values().for_each(|s| self.nodes(s, &scope));
                    x.writes.values().for_each(|p| self.place(p, &scope, true));
                    self.events(&x.events, &scope);
                }
                Node::Use(u) => {
                    self.visit(&u.name);
                    u.args.values().for_each(|e| self.expr(e, &scope, true));
                    u.slots.values().for_each(|s| self.nodes(s, &scope));
                    self.events(&u.events, &scope);
                }
                Node::For(f) => {
                    self.mention(&f.ty);
                    self.expr(&f.over, &scope, false);
                    let mut inner = scope.with(&f.name, Some(f.ty.clone()));
                    if let Some(p) = &f.position {
                        inner = inner.with(p, Some(Type::named("core/int")));
                    }
                    self.nodes(&f.body, &inner);
                }
                Node::If(i) => {
                    for b in &i.branches {
                        let inner = self.cond(&b.when, &scope);
                        self.nodes(&b.then, &inner);
                    }
                    self.nodes(&i.else_, &scope);
                }
                Node::Match(m) => {
                    self.expr(&m.value, &scope, false);
                    for c in &m.cases {
                        if let Some(inner) = self.patterns(&c.when, &scope) {
                            self.nodes(&c.then, &inner);
                        }
                    }
                    self.nodes(&m.else_, &scope);
                }
                Node::Place(p) => self.expr(&p.view, &scope, false),
                Node::Declare(d) => {
                    self.mention(&d.let_.ty);
                    self.expr(&d.let_.value, &scope, true);
                    scope = scope.with(&d.let_.name, Some(d.let_.ty.clone()));
                }
            }
        }
    }
}

fn params<'m>(scope: &Scope<'m>, params: &[ir::Param]) -> Scope<'m> {
    let mut s = scope.clone();
    s.locals.extend(params.iter().map(|p| (p.name.clone(), Some(p.ty.clone()))));
    s
}

/// A value written out whole, with nothing to work out.
fn is_constant(e: &Expr) -> bool {
    match e {
        Expr::Num(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Duration(_) | Expr::None(_) | Expr::Token(_) => true,
        Expr::List(xs) => xs.iter().all(is_constant),
        Expr::Record(r) => r.fields.values().all(is_constant),
        Expr::Case(c) => c.values.values().all(is_constant),
        Expr::Some(x) => is_constant(x),
        Expr::Range(r) => [&r.low, &r.high, &r.step].into_iter().flatten().all(is_constant),
        Expr::Tokval(t) => is_constant(&t.value),
        _ => false,
    }
}

/// A declaration's full name.
pub fn name_of(d: &Decl) -> Option<&str> {
    Some(match d {
        Decl::Record(x) => &x.name,
        Decl::Enum(x) => &x.name,
        Decl::Token(x) => &x.name,
        Decl::Interface(x) => &x.name,
        Decl::Fn(x) => &x.name,
        Decl::Extern(Extern::Fn(x)) => &x.name,
        Decl::Extern(Extern::Widget(x)) => &x.name,
        Decl::Extern(Extern::Channel(x)) => &x.name,
        Decl::Extern(Extern::Type(x)) => &x.name,
        Decl::Let(x) => &x.name,
        Decl::Type(x) => &x.name,
        Decl::State(_) | Decl::Expose(_) => return None,
    })
}

/// What a page can run.
struct Found {
    kept: BTreeSet<String>,
    cases: HashSet<(String, String)>,
    whole: HashSet<String>,
    built: HashSet<String>,
    defaulted: HashSet<String>,
    any_built: bool,
    taken: BTreeSet<String>,
}

impl Found {
    fn made(&self, ty: &str, case: &str) -> bool {
        self.whole.contains(ty) || self.cases.contains(&(ty.to_string(), case.to_string()))
    }

    fn cut(&self, m: &mut ir::Module) {
        // what a match can be on: an enum's case, or a token's, that the page makes
        let declared: HashSet<String> = m.declarations.iter().filter_map(|d| if matches!(d.declare, Decl::Enum(_) | Decl::Token(_)) { name_of(&d.declare).map(str::to_string) } else { None }).collect();
        m.declarations.retain(|d| name_of(&d.declare).map_or(true, |n| self.kept.contains(n)));
        m.templates.retain(|t| self.kept.contains(&t.name));
        m.handlers.retain(|h| self.kept.contains(&h.id));
        for d in &mut m.declarations {
            match &mut d.declare {
                Decl::Record(r) => {
                    let made = self.any_built || self.built.contains(&r.name) || self.whole.contains(&r.name);
                    if !made {
                        r.requires.clear();
                    }
                    if !made && !self.defaulted.contains(&r.name) {
                        r.fields.iter_mut().for_each(|f| f.default = None);
                    }
                    if r.same.as_ref().map_or(false, |s| !self.kept.contains(s)) {
                        r.same = None;
                    }
                }
                Decl::Enum(e) => {
                    let name = e.name.clone();
                    e.cases.retain(|c| self.made(&name, &c.name));
                }
                Decl::Token(t) => {
                    let name = t.name.clone();
                    t.cases.retain(|c| self.made(&name, &c.name));
                    t.values.retain(|c, _| self.made(&name, c));
                }
                Decl::Extern(Extern::Channel(c)) => {
                    let name = c.name.clone();
                    c.values.retain(|v| self.taken.contains(&format!("{}.{}", name, v.name)));
                    c.fns.retain(|f| self.taken.contains(&format!("{}.{}", name, f.name)));
                }
                _ => {}
            }
        }
        for t in &mut m.templates {
            t.fns.retain(|f| self.kept.contains(&f.name));
        }
        let arms = |when: &mut Vec<ir::Pattern>| {
            when.retain(|p| if let ir::Pattern::Case(c) = p { !declared.contains(&c.enum_) || self.made(&c.enum_, &c.name) } else { true });
            !when.is_empty()
        };
        crate::lower::walk::module(m, &mut |p| match p {
            crate::lower::walk::Part::Node(Node::Match(x)) => x.cases.retain_mut(|c| arms(&mut c.when)),
            crate::lower::walk::Part::Stmt(Stmt::Match(x)) => x.cases.retain_mut(|c| arms(&mut c.when)),
            _ => {}
        });
        self.needs(m);
    }

    /// What the page takes from whom.
    fn needs(&self, m: &mut ir::Module) {
        let mut core = BTreeSet::new();
        let mut host = BTreeSet::new();
        let mut packs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for d in &m.declarations {
            let Decl::Extern(x) = &d.declare else { continue };
            let (names, supplier, channel): (Vec<String>, ir::Supplier, bool) = match x {
                Extern::Fn(f) => (vec![f.name.clone()], f.supplier, false),
                Extern::Widget(w) => (vec![w.name.clone()], w.supplier, false),
                Extern::Channel(c) => (c.values.iter().map(|v| &v.name).chain(c.fns.iter().map(|f| &f.name)).map(|n| format!("{}.{}", c.name, n)).collect(), c.supplier, true),
                Extern::Type(_) => continue,
            };
            match supplier {
                ir::Supplier::Core if !channel => core.extend(names),
                // core's page is the host's to give
                ir::Supplier::Core | ir::Supplier::Host => host.extend(names),
                ir::Supplier::Pack => {
                    for n in names {
                        packs.entry(n.split('/').next().unwrap_or("").to_string()).or_default().insert(n);
                    }
                }
            }
        }
        m.needs.core = core.into_iter().collect();
        m.needs.host = host.into_iter().collect();
        for p in &mut m.needs.packs {
            p.externs = packs.remove(&p.name).unwrap_or_default().into_iter().collect();
        }
        m.needs.packs.retain(|p| !p.externs.is_empty());
    }
}
