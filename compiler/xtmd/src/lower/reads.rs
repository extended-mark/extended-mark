//! What each place in a page reads, what each handler writes: told in the IR, so whoever runs a page or
//! makes code of it has nothing to work out.
//!
//! A read is of what can be another value later: `$state`, a widget's inputs, variables and slots, an
//! extern value, a local of the view around it. A fn reads what it reads itself and what the fns it
//! calls do; a fn called on a value whose type isn't known here may be any type's. What is inside a
//! view or a rich_text held in a value says its own, where it is placed.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::formats::ir::{self, Cond, Decl, Expr, Extern, Get, Node, Root, Stmt, Type};

use crate::lower::walk;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum At {
    Local(String),
    Input(String),
    Cell(String),
    State(String),
    Slot(String),
    Extern(String),
    Any,
}

impl At {
    fn read(&self) -> ir::Read {
        match self {
            At::Local(n) => ir::Read::Local(n.clone()),
            At::Input(n) => ir::Read::Input(n.clone()),
            At::Cell(n) => ir::Read::Cell(n.clone()),
            At::State(n) => ir::Read::State(n.clone()),
            At::Slot(n) => ir::Read::Slot(n.clone()),
            At::Extern(n) => ir::Read::Extern(n.clone()),
            At::Any => ir::Read::Any(Default::default()),
        }
    }

    fn root(&self) -> Option<Root> {
        Some(match self {
            At::Local(n) => Root::Local(n.clone()),
            At::Input(n) => Root::Input(n.clone()),
            At::Cell(n) => Root::Cell(n.clone()),
            At::State(n) => Root::State(n.clone()),
            _ => return None,
        })
    }

    fn of(r: &Root) -> At {
        match r {
            Root::Local(n) => At::Local(n.clone()),
            Root::Input(n) => At::Input(n.clone()),
            Root::Cell(n) => At::Cell(n.clone()),
            Root::State(n) => At::State(n.clone()),
        }
    }
}

/// What some code reads and writes, its own locals apart.
#[derive(Clone, Default, PartialEq)]
struct Touched {
    reads: BTreeSet<At>,
    writes: BTreeSet<At>,
}

impl Touched {
    fn add(&mut self, o: &Touched) {
        self.reads.extend(o.reads.iter().cloned());
        self.writes.extend(o.writes.iter().cloned());
    }

    fn reads(&self) -> Vec<ir::Read> {
        self.reads.iter().map(At::read).collect()
    }

    fn writes(&self) -> Vec<Root> {
        self.writes.iter().filter_map(At::root).collect()
    }
}

/// What is known of the module's fns, lets and records, by full name.
#[derive(Default)]
struct Known {
    of: HashMap<String, Touched>,
    /// The fns of a name, whatever type they are on: `show`, `same`.
    named: HashMap<String, Vec<String>>,
    /// What any record's rules touch.
    rules: Touched,
    /// A variable may hold a fn, so a fn called may be one that was kept.
    kept: bool,
}

/// Writes in a module what its code reads and writes. What was written before is written anew.
pub fn tell(m: &mut ir::Module) {
    let known = Known::new(m);
    for d in &mut m.declarations {
        match &mut d.declare {
            Decl::Fn(f) => known.tell_fn(f),
            Decl::Record(r) => known.tell_rules(&mut r.requires, r.fields.iter().map(|f| f.name.as_str()).collect()),
            Decl::State(s) => known.tell_rules(&mut s.requires, Vec::new()),
            Decl::Extern(Extern::Widget(w)) => known.tell_rules(&mut w.requires, Vec::new()),
            Decl::Expose(e) => {
                for f in &mut e.fns {
                    f.writes = known.body(&f.body, f.params.iter().map(|p| p.name.as_str()).collect()).writes();
                }
            }
            _ => {}
        }
    }
    for t in &mut m.templates {
        t.fns.iter_mut().for_each(|f| known.tell_fn(f));
        known.tell_rules(&mut t.requires, Vec::new());
    }
    for h in &mut m.handlers {
        let touched = known.body(&h.body, h.params.iter().map(|p| p.name.as_str()).collect());
        h.reads = touched.reads();
        h.writes = touched.writes();
    }
    walk::module(m, &mut |p| match p {
        walk::Part::Node(n) => known.tell_node(n),
        walk::Part::Expr(Expr::View(v)) => v.reads = known.each(&v.args, None),
        _ => {}
    });
    // Markdown is written whole, when somebody asks for it
    let mut clear = |p: walk::Part<'_>| match p {
        walk::Part::Node(n) => quiet(n),
        walk::Part::Expr(Expr::View(v)) => v.reads.clear(),
        _ => {}
    };
    for t in &mut m.templates {
        t.markdown.iter_mut().for_each(|ns| walk::nodes(ns, &mut clear));
    }
    for d in &mut m.declarations {
        if let Decl::Extern(Extern::Widget(w)) = &mut d.declare {
            walk::nodes(&mut w.markdown, &mut clear);
        }
    }
}

fn quiet(n: &mut Node) {
    match n {
        Node::Extern(x) => x.reads.clear(),
        Node::Use(u) => u.reads.clear(),
        Node::For(f) => f.reads.clear(),
        Node::If(i) => i.branches.iter_mut().for_each(|b| b.reads.clear()),
        Node::Match(m) => m.reads.clear(),
        Node::Place(p) => p.reads.clear(),
        Node::Declare(d) => d.reads.clear(),
    }
}

impl Known {
    fn new(m: &ir::Module) -> Known {
        let mut k = Known { kept: keeps_fns(m), ..Known::default() };
        let mut fns: Vec<(&str, Vec<&str>, &[Stmt])> = Vec::new();
        let mut lets: Vec<(&str, &Expr)> = Vec::new();
        let mut records: Vec<(Vec<&str>, &ir::Record)> = Vec::new();
        for d in &m.declarations {
            match &d.declare {
                Decl::Fn(f) => fns.push((&f.name, names(&f.params), &f.body)),
                Decl::Extern(Extern::Fn(f)) => fns.push((&f.name, names(&f.params), &f.default)),
                Decl::Let(l) => lets.push((&l.name, &l.value)),
                Decl::Record(r) => records.push((r.fields.iter().map(|f| f.name.as_str()).collect(), r)),
                _ => {}
            }
        }
        for f in m.templates.iter().flat_map(|t| &t.fns) {
            fns.push((&f.name, names(&f.params), &f.body));
        }
        for (n, ..) in &fns {
            if let Some((_, last)) = n.rsplit_once('.') {
                k.named.entry(last.to_string()).or_default().push(n.to_string());
            }
        }
        // each may call the others: they are gone through until nothing new is found
        loop {
            let mut changed = false;
            let mut rules = Touched::default();
            for (fields, r) in &records {
                for q in &r.requires {
                    rules.add(&k.body(&q.body, fields.clone()));
                }
                for f in r.fields.iter().filter_map(|f| f.default.as_ref()) {
                    rules.add(&k.value(f));
                }
            }
            changed |= rules != k.rules;
            k.rules = rules;
            for (n, params, body) in &fns {
                let t = k.body(body, params.clone());
                if k.of.get(*n) != Some(&t) {
                    changed = true;
                    k.of.insert(n.to_string(), t);
                }
            }
            for (n, e) in &lets {
                let t = k.value(e);
                if k.of.get(*n) != Some(&t) {
                    changed = true;
                    k.of.insert(n.to_string(), t);
                }
            }
            if !changed {
                return k;
            }
        }
    }

    fn tell_fn(&self, f: &mut ir::FnDecl) {
        let t = self.body(&f.body, names(&f.params));
        f.reads = t.reads();
        f.writes = t.writes();
    }

    fn tell_rules(&self, rules: &mut [ir::Require], own: Vec<&str>) {
        for r in rules {
            r.reads = self.body(&r.body, own.clone()).reads();
        }
    }

    fn tell_node(&self, n: &mut Node) {
        match n {
            Node::Extern(x) => x.reads = self.each(&x.props, Some(&x.writes)),
            Node::Use(u) => u.reads = self.each(&u.args, None),
            Node::For(f) => f.reads = self.value(&f.over).reads(),
            Node::If(i) => {
                for b in &mut i.branches {
                    b.reads = match &b.when {
                        Cond::Cond(e) => self.value(e).reads(),
                        Cond::Open(o) => self.value(&o.opt).reads(),
                    };
                }
            }
            Node::Match(m) => m.reads = self.value(&m.value).reads(),
            Node::Place(p) => p.reads = self.value(&p.view).reads(),
            Node::Declare(d) => d.reads = self.value(&d.let_.value).reads(),
        }
    }

    /// Each input's reads, the ones that read nothing left out.
    fn each(&self, args: &BTreeMap<String, Expr>, writes: Option<&BTreeMap<String, ir::Place>>) -> BTreeMap<String, Vec<ir::Read>> {
        let mut out: BTreeMap<String, Touched> = args.iter().map(|(k, e)| (k.clone(), self.value(e))).collect();
        for (k, p) in writes.into_iter().flatten() {
            self.place(p, &mut Vec::new(), out.entry(k.clone()).or_default());
        }
        out.into_iter().filter(|(_, t)| !t.reads.is_empty()).map(|(k, t)| (k, t.reads())).collect()
    }

    fn value(&self, e: &Expr) -> Touched {
        let mut t = Touched::default();
        self.expr(e, &mut Vec::new(), false, &mut t);
        t
    }

    fn body(&self, body: &[Stmt], own: Vec<&str>) -> Touched {
        let mut t = Touched::default();
        let mut own: Vec<String> = own.into_iter().map(str::to_string).collect();
        self.stmts(body, &mut own, &mut t);
        t
    }

    fn called(&self, name: &str, t: &mut Touched) {
        if let Some(of) = self.of.get(name) {
            t.add(of);
        } else if let Some((_, last)) = name.rsplit_once('.') {
            self.method(last, t);
        }
    }

    /// A fn of this name, on whichever type.
    fn method(&self, name: &str, t: &mut Touched) {
        for n in self.named.get(name).into_iter().flatten() {
            if let Some(of) = self.of.get(n) {
                t.add(of);
            }
        }
    }

    fn local(&self, n: &str, own: &[String], t: &mut BTreeSet<At>) {
        if !own.iter().any(|o| o == n) {
            t.insert(At::Local(n.to_string()));
        }
    }

    fn place(&self, p: &ir::Place, own: &mut Vec<String>, t: &mut Touched) {
        match &p.root {
            Root::Local(n) => self.local(n, own, &mut t.reads),
            r => {
                t.reads.insert(At::of(r));
            }
        }
        for s in &p.path {
            if let ir::Step::Item(i) = s {
                self.expr(&i.at, own, true, t);
            }
        }
    }

    fn written(&self, p: &ir::Place, own: &mut Vec<String>, t: &mut Touched) {
        match &p.root {
            Root::Local(n) => self.local(n, own, &mut t.writes),
            r => {
                t.writes.insert(At::of(r));
            }
        }
        for s in &p.path {
            if let ir::Step::Item(i) = s {
                self.expr(&i.at, own, true, t);
            }
        }
    }

    /// `inside`: its value is worked out further here, so a view or a rich_text in it is read whole.
    fn expr(&self, e: &Expr, own: &mut Vec<String>, inside: bool, t: &mut Touched) {
        match e {
            Expr::Num(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Duration(_) | Expr::None(_) | Expr::Token(_) => {}
            Expr::List(xs) => xs.iter().for_each(|x| self.expr(x, own, inside, t)),
            Expr::Record(r) => {
                t.add(&self.rules);
                r.fields.values().for_each(|x| self.expr(x, own, inside, t));
            }
            Expr::Case(c) => c.values.values().for_each(|x| self.expr(x, own, inside, t)),
            Expr::Some(x) => self.expr(x, own, inside, t),
            Expr::Sentence(x) => self.expr(x, own, true, t),
            Expr::Range(r) => [&r.low, &r.high, &r.step].into_iter().flatten().for_each(|x| self.expr(x, own, true, t)),
            Expr::Tokval(x) => self.expr(&x.value, own, inside, t),
            Expr::Rich(r) => {
                if inside {
                    self.nodes(&r.nodes, own, t);
                }
            }
            Expr::Template(x) => x.holes.iter().for_each(|x| self.expr(x, own, true, t)),
            Expr::View(v) => {
                v.args.values().for_each(|x| self.expr(x, own, inside, t));
                if inside {
                    v.slots.values().for_each(|ns| self.nodes(ns, own, t));
                }
            }
            Expr::Get(g) => match g {
                Get::Local(n) => self.local(n, own, &mut t.reads),
                Get::Input(n) => {
                    t.reads.insert(At::Input(n.clone()));
                }
                Get::Cell(n) => {
                    t.reads.insert(At::Cell(n.clone()));
                }
                Get::State(n) => {
                    t.reads.insert(At::State(n.clone()));
                }
                Get::Slot(n) => {
                    t.reads.insert(At::Slot(n.clone()));
                }
                Get::Extern(n) => {
                    t.reads.insert(At::Extern(n.clone()));
                }
                Get::Let(n) => {
                    if let Some(of) = self.of.get(n) {
                        t.add(of);
                    }
                }
                Get::Token(_) => {}
            },
            Expr::Field(f) => self.expr(&f.value, own, true, t),
            Expr::Call(c) => self.call(c, own, t),
            Expr::Apply(a) => self.apply(&a.fn_, &a.args, own, t),
            Expr::Op(o) => {
                o.args.iter().for_each(|x| self.expr(x, own, inside, t));
                match o.name {
                    ir::OpName::Eq | ir::OpName::Ne => self.method("same", t),
                    ir::OpName::Lt | ir::OpName::Le | ir::OpName::Gt | ir::OpName::Ge => self.method("before", t),
                    _ => {}
                }
            }
            Expr::Show(s) => {
                self.expr(&s.value, own, true, t);
                self.method("show", t);
            }
            Expr::Fn(l) => {
                let n = own.len();
                own.extend(l.params.iter().map(|p| p.name.clone()));
                self.stmts(&l.body, own, t);
                own.truncate(n);
            }
            // a `mut` argument
            Expr::Place(p) => {
                self.place(p, own, t);
            }
            Expr::Try(x) => self.expr(&x.value, own, inside, t),
            Expr::With(w) => {
                t.add(&self.rules);
                self.expr(&w.value, own, true, t);
                w.fields.values().for_each(|x| self.expr(x, own, inside, t));
            }
        }
    }

    fn call(&self, c: &ir::Call, own: &mut Vec<String>, t: &mut Touched) {
        for x in &c.args {
            self.expr(x, own, true, t);
            if let Expr::Place(p) = x {
                self.written(p, own, t);
            }
        }
        self.called(&c.name, t);
        // fns of core that compare what they are given
        match c.name.as_str() {
            "core/list.remove" => self.method("same", t),
            "core/list.sort" | "core/ordered.clamp" | "core/range.has" => self.method("before", t),
            _ => {}
        }
    }

    /// A fn held in a value. What it reads was read where it was written, unless it was kept in a variable.
    fn apply(&self, f: &Expr, args: &[Expr], own: &mut Vec<String>, t: &mut Touched) {
        self.expr(f, own, true, t);
        for x in args {
            self.expr(x, own, true, t);
            if let Expr::Place(p) = x {
                self.written(p, own, t);
            }
        }
        if self.kept {
            t.reads.insert(At::Any);
        }
    }

    /// Nodes held in a value that is worked out further: what they read counts here.
    fn nodes(&self, ns: &[Node], own: &mut Vec<String>, t: &mut Touched) {
        let n = own.len();
        for x in ns {
            match x {
                Node::Extern(x) => {
                    x.props.values().for_each(|e| self.expr(e, own, true, t));
                    x.slots.values().for_each(|s| self.nodes(s, own, t));
                }
                Node::Use(u) => {
                    u.args.values().for_each(|e| self.expr(e, own, true, t));
                    u.slots.values().for_each(|s| self.nodes(s, own, t));
                }
                Node::For(f) => {
                    self.expr(&f.over, own, true, t);
                    own.push(f.name.clone());
                    own.extend(f.position.clone());
                    self.nodes(&f.body, own, t);
                    own.truncate(own.len() - 1 - f.position.iter().count());
                }
                Node::If(i) => {
                    for b in &i.branches {
                        let k = own.len();
                        self.cond(&b.when, own, t);
                        self.nodes(&b.then, own, t);
                        own.truncate(k);
                    }
                    self.nodes(&i.else_, own, t);
                }
                Node::Match(m) => {
                    self.expr(&m.value, own, true, t);
                    for c in &m.cases {
                        let k = own.len();
                        binds(&c.when, own);
                        self.nodes(&c.then, own, t);
                        own.truncate(k);
                    }
                    self.nodes(&m.else_, own, t);
                }
                Node::Place(p) => self.expr(&p.view, own, true, t),
                Node::Declare(d) => {
                    self.expr(&d.let_.value, own, true, t);
                    own.push(d.let_.name.clone());
                }
            }
        }
        own.truncate(n);
    }

    fn cond(&self, c: &Cond, own: &mut Vec<String>, t: &mut Touched) {
        match c {
            Cond::Cond(e) => self.expr(e, own, true, t),
            Cond::Open(o) => {
                self.expr(&o.opt, own, true, t);
                // what is opened to be written is written through
                if let (true, Expr::Place(p)) = (o.mutable, &o.opt) {
                    self.written(p, own, t);
                }
                own.push(o.name.clone());
            }
        }
    }

    fn stmts(&self, ss: &[Stmt], own: &mut Vec<String>, t: &mut Touched) {
        let n = own.len();
        for s in ss {
            match s {
                Stmt::Declare(ir::StmtDeclare::Let(l)) | Stmt::Declare(ir::StmtDeclare::Mut(l)) => {
                    self.expr(&l.value, own, true, t);
                    own.push(l.name.clone());
                }
                Stmt::Set(x) => {
                    self.expr(&x.value, own, true, t);
                    self.written(&x.place, own, t);
                    // a record written into has its rules run
                    t.add(&self.rules);
                }
                Stmt::Return(r) => r.value.iter().for_each(|e| self.expr(e, own, true, t)),
                Stmt::If(i) => {
                    for b in &i.branches {
                        let k = own.len();
                        self.cond(&b.when, own, t);
                        self.stmts(&b.then, own, t);
                        own.truncate(k);
                    }
                    self.stmts(&i.else_, own, t);
                }
                Stmt::For(f) => {
                    self.expr(&f.over, own, true, t);
                    if let (true, Expr::Place(p)) = (f.mutable, &f.over) {
                        self.written(p, own, t);
                    }
                    let k = own.len();
                    own.push(f.name.clone());
                    own.extend(f.position.clone());
                    self.stmts(&f.body, own, t);
                    own.truncate(k);
                }
                Stmt::Match(m) => {
                    self.expr(&m.value, own, true, t);
                    for c in &m.cases {
                        let k = own.len();
                        binds(&c.when, own);
                        self.stmts(&c.then, own, t);
                        own.truncate(k);
                    }
                    self.stmts(&m.else_, own, t);
                }
                Stmt::Invoke(i) => i.args.iter().for_each(|e| self.expr(e, own, true, t)),
                Stmt::Call(c) => self.call(c, own, t),
                Stmt::Apply(a) => self.apply(&a.fn_, &a.args, own, t),
            }
        }
        own.truncate(n);
    }
}

fn names(ps: &[ir::Param]) -> Vec<&str> {
    ps.iter().map(|p| p.name.as_str()).collect()
}

/// What a match's patterns bind.
fn binds(ps: &[ir::Pattern], own: &mut Vec<String>) {
    for p in ps {
        if let ir::Pattern::Case(c) = p {
            own.extend(c.binds.iter().cloned());
        }
    }
}

/// Whether a widget's variable or a `$state` field may hold a fn.
fn keeps_fns(m: &ir::Module) -> bool {
    let decls: HashMap<&str, &Decl> = m.declarations.iter().filter_map(|d| crate::lower::reach::name_of(&d.declare).map(|n| (n, &d.declare))).collect();
    let mut types: Vec<&Type> = m.templates.iter().flat_map(|t| &t.cells).map(|f| &f.ty).collect();
    for d in &m.declarations {
        if let Decl::State(s) = &d.declare {
            types.extend(s.fields.iter().map(|f| &f.ty));
        }
    }
    let mut seen = BTreeSet::new();
    while let Some(t) = types.pop() {
        if t.name == "fn" || t.name == "any" || t.name.starts_with(|c: char| c.is_ascii_uppercase()) {
            return true;
        }
        types.extend(&t.args);
        types.extend(t.fields.iter().map(|f| &f.ty));
        if !seen.insert(t.name.as_str()) {
            continue;
        }
        match decls.get(t.name.as_str()) {
            Some(Decl::Record(r)) => types.extend(r.fields.iter().filter(|f| !r.params.iter().any(|p| p.name == f.ty.name)).map(|f| &f.ty)),
            Some(Decl::Enum(e)) => types.extend(e.cases.iter().flat_map(|c| &c.fields).filter(|f| !e.params.iter().any(|p| p.name == f.ty.name)).map(|f| &f.ty)),
            Some(Decl::Token(x)) => types.extend(x.cases.iter().flat_map(|c| &c.fields).map(|f| &f.ty)),
            Some(Decl::Type(a)) => types.push(&a.is),
            _ => {}
        }
    }
    false
}
