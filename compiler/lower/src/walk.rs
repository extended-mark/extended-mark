//! Every node, statement and expression of a module, to change in place: each before what is in it.

use xtmd_ir::ir::{self, Cond, Decl, Expr, Extern, Node, Stmt, Type};

pub enum Part<'a> {
    Node(&'a mut Node),
    Stmt(&'a mut Stmt),
    Expr(&'a mut Expr),
}

type On<'f> = &'f mut dyn FnMut(Part<'_>);

pub fn module(m: &mut ir::Module, on: On) {
    for d in &mut m.declarations {
        match &mut d.declare {
            Decl::Record(r) => {
                fields(&mut r.fields, on);
                rules(&mut r.requires, on);
            }
            Decl::Enum(e) => e.cases.iter_mut().for_each(|c| fields(&mut c.fields, on)),
            Decl::Token(t) => {
                ty(&mut t.value, on);
                t.values.values_mut().for_each(|e| expr(e, on));
                t.cases.iter_mut().for_each(|c| fields(&mut c.fields, on));
            }
            Decl::Interface(i) => {
                fields(&mut i.fields, on);
                i.events.iter_mut().for_each(|e| params(&mut e.params, on));
            }
            Decl::Fn(f) => fn_decl(f, on),
            Decl::Extern(Extern::Fn(f)) => {
                params(&mut f.params, on);
                f.returns.iter_mut().for_each(|t| ty(t, on));
                stmts(&mut f.default, on);
            }
            Decl::Extern(Extern::Widget(w)) => {
                fields(&mut w.inputs, on);
                w.slots.iter_mut().for_each(|s| ty(&mut s.ty, on));
                w.events.iter_mut().for_each(|e| params(&mut e.params, on));
                rules(&mut w.requires, on);
                nodes(&mut w.markdown, on);
            }
            Decl::Extern(Extern::Channel(c)) => {
                fields(&mut c.values, on);
                for f in &mut c.fns {
                    params(&mut f.params, on);
                    f.returns.iter_mut().for_each(|t| ty(t, on));
                }
            }
            Decl::Extern(Extern::Type(_)) => {}
            Decl::State(s) => {
                fields(&mut s.fields, on);
                rules(&mut s.requires, on);
            }
            Decl::Expose(e) => {
                for f in &mut e.fns {
                    params(&mut f.params, on);
                    stmts(&mut f.body, on);
                }
            }
            Decl::Let(l) => {
                ty(&mut l.ty, on);
                expr(&mut l.value, on);
            }
            Decl::Type(t) => ty(&mut t.is, on),
        }
    }
    for t in &mut m.templates {
        fields(&mut t.inputs, on);
        fields(&mut t.cells, on);
        t.slots.iter_mut().for_each(|s| ty(&mut s.ty, on));
        t.events.iter_mut().for_each(|e| params(&mut e.params, on));
        rules(&mut t.requires, on);
        t.fns.iter_mut().for_each(|f| fn_decl(f, on));
        nodes(&mut t.view, on);
        t.markdown.iter_mut().for_each(|m| nodes(m, on));
    }
    for h in &mut m.handlers {
        params(&mut h.params, on);
        stmts(&mut h.body, on);
    }
}

fn fn_decl(f: &mut ir::FnDecl, on: On) {
    params(&mut f.params, on);
    f.returns.iter_mut().for_each(|t| ty(t, on));
    stmts(&mut f.body, on);
}

fn fields(fs: &mut [ir::Field], on: On) {
    for f in fs {
        ty(&mut f.ty, on);
        f.default.iter_mut().for_each(|e| expr(e, on));
    }
}

fn params(ps: &mut [ir::Param], on: On) {
    ps.iter_mut().for_each(|p| ty(&mut p.ty, on));
}

fn rules(rs: &mut [ir::Require], on: On) {
    rs.iter_mut().for_each(|r| stmts(&mut r.body, on));
}

fn ty(t: &mut Type, on: On) {
    t.args.iter_mut().for_each(|a| ty(a, on));
    params(&mut t.params, on);
    t.returns.iter_mut().for_each(|r| ty(r, on));
    fields(&mut t.fields, on);
    if let Some(c) = &mut t.contract {
        for b in [&mut c.range, &mut c.length].into_iter().flatten() {
            b.low.iter_mut().chain(b.high.iter_mut()).for_each(|e| expr(e, on));
        }
        c.step.iter_mut().chain(c.only.iter_mut()).for_each(|e| expr(e, on));
    }
}

fn cond(c: &mut Cond, on: On) {
    match c {
        Cond::Cond(e) => expr(e, on),
        Cond::Open(o) => {
            ty(&mut o.ty, on);
            expr(&mut o.opt, on);
        }
    }
}

fn place(p: &mut ir::Place, on: On) {
    for s in &mut p.path {
        if let ir::Step::Item(i) = s {
            expr(&mut i.at, on);
        }
    }
}

pub fn nodes(ns: &mut [Node], on: On) {
    for n in ns {
        on(Part::Node(n));
        match n {
            Node::Extern(x) => {
                x.props.values_mut().for_each(|e| expr(e, on));
                x.slots.values_mut().for_each(|s| nodes(s, on));
                x.writes.values_mut().for_each(|p| place(p, on));
            }
            Node::Use(u) => {
                u.args.values_mut().for_each(|e| expr(e, on));
                u.slots.values_mut().for_each(|s| nodes(s, on));
            }
            Node::For(f) => {
                ty(&mut f.ty, on);
                expr(&mut f.over, on);
                nodes(&mut f.body, on);
            }
            Node::If(i) => {
                for b in &mut i.branches {
                    cond(&mut b.when, on);
                    nodes(&mut b.then, on);
                }
                nodes(&mut i.else_, on);
            }
            Node::Match(m) => {
                expr(&mut m.value, on);
                m.cases.iter_mut().for_each(|c| nodes(&mut c.then, on));
                nodes(&mut m.else_, on);
            }
            Node::Place(p) => expr(&mut p.view, on),
            Node::Declare(d) => {
                ty(&mut d.let_.ty, on);
                expr(&mut d.let_.value, on);
            }
        }
    }
}

pub fn stmts(ss: &mut [Stmt], on: On) {
    for s in ss {
        on(Part::Stmt(s));
        match s {
            Stmt::Declare(ir::StmtDeclare::Let(l)) | Stmt::Declare(ir::StmtDeclare::Mut(l)) => {
                ty(&mut l.ty, on);
                expr(&mut l.value, on);
            }
            Stmt::Set(x) => {
                place(&mut x.place, on);
                expr(&mut x.value, on);
            }
            Stmt::Return(r) => r.value.iter_mut().for_each(|e| expr(e, on)),
            Stmt::If(i) => {
                for b in &mut i.branches {
                    cond(&mut b.when, on);
                    stmts(&mut b.then, on);
                }
                stmts(&mut i.else_, on);
            }
            Stmt::For(f) => {
                ty(&mut f.ty, on);
                expr(&mut f.over, on);
                stmts(&mut f.body, on);
            }
            Stmt::Match(m) => {
                expr(&mut m.value, on);
                m.cases.iter_mut().for_each(|c| stmts(&mut c.then, on));
                stmts(&mut m.else_, on);
            }
            Stmt::Invoke(i) => i.args.iter_mut().for_each(|e| expr(e, on)),
            Stmt::Call(c) => c.args.iter_mut().for_each(|e| expr(e, on)),
            Stmt::Apply(a) => {
                expr(&mut a.fn_, on);
                a.args.iter_mut().for_each(|e| expr(e, on));
            }
        }
    }
}

pub fn expr(e: &mut Expr, on: On) {
    on(Part::Expr(e));
    match e {
        Expr::Num(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Duration(_) | Expr::None(_) | Expr::Token(_) | Expr::Get(_) => {}
        Expr::List(xs) => xs.iter_mut().for_each(|x| expr(x, on)),
        Expr::Record(r) => r.fields.values_mut().for_each(|x| expr(x, on)),
        Expr::Case(c) => c.values.values_mut().for_each(|x| expr(x, on)),
        Expr::Some(x) | Expr::Sentence(x) => expr(x, on),
        Expr::Range(r) => [&mut r.low, &mut r.high, &mut r.step].into_iter().flatten().for_each(|x| expr(x, on)),
        Expr::Tokval(t) => expr(&mut t.value, on),
        Expr::Rich(r) => {
            let calls: &mut Vec<Node> = std::sync::Arc::make_mut(&mut r.nodes);
            nodes(calls, on);
        }
        Expr::Template(t) => t.holes.iter_mut().for_each(|x| expr(x, on)),
        Expr::View(v) => {
            v.args.values_mut().for_each(|x| expr(x, on));
            v.slots.values_mut().for_each(|s| nodes(s, on));
        }
        Expr::Field(f) => expr(&mut f.value, on),
        Expr::Call(c) => c.args.iter_mut().for_each(|x| expr(x, on)),
        Expr::Apply(a) => {
            expr(&mut a.fn_, on);
            a.args.iter_mut().for_each(|x| expr(x, on));
        }
        Expr::Op(o) => o.args.iter_mut().for_each(|x| expr(x, on)),
        Expr::Show(s) => expr(&mut s.value, on),
        Expr::Fn(l) => {
            params(&mut l.params, on);
            l.returns.iter_mut().for_each(|t| ty(t, on));
            stmts(&mut l.body, on);
        }
        Expr::Place(p) => place(p, on),
        Expr::Try(t) => expr(&mut t.value, on),
        Expr::With(w) => {
            expr(&mut w.value, on);
            w.fields.values_mut().for_each(|x| expr(x, on));
        }
    }
}
