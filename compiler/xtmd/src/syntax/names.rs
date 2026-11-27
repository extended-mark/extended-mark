//! The names an expression reads, replaced: how a migration puts a use's own values into its code.

use crate::syntax::ast::*;

/// Whether an expression needs brackets where something else is done with its value: `a + b` in
/// `(a + b) * 2`.
pub fn needs_brackets(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Binary { .. } | ExprKind::Unary { .. } | ExprKind::Ternary { .. } | ExprKind::Range { .. } | ExprKind::Try(_) | ExprKind::With { .. } | ExprKind::Lambda(_) | ExprKind::Record { .. } | ExprKind::View(_))
}

/// Every name that stands for a value, given to `f` with whether its place needs brackets around
/// anything but the simplest; what `f` gives back goes there. A case's type, a fn called by its name
/// and a widget's name stay.
pub fn replace_names(e: &mut Expr, f: &mut dyn FnMut(&str, bool) -> Option<Expr>) {
    expr(e, false, f);
}

/// As `replace_names`, through a block's statements.
pub fn replace_names_in(b: &mut Block, f: &mut dyn FnMut(&str, bool) -> Option<Expr>) {
    block(b, f);
}

/// As `replace_names`, through one statement.
pub fn replace_names_of(s: &mut Stmt, f: &mut dyn FnMut(&str, bool) -> Option<Expr>) {
    stmt(s, f);
}

type F<'a> = dyn FnMut(&str, bool) -> Option<Expr> + 'a;

fn expr(e: &mut Expr, tight: bool, f: &mut F) {
    match &mut e.kind {
        ExprKind::Name(n) => {
            if let Some(v) = f(&n.name, tight) {
                *e = v;
            }
        }
        ExprKind::Number(_) | ExprKind::Duration(_) | ExprKind::Bool(_) | ExprKind::SelfValue | ExprKind::State => {}
        ExprKind::Text(t) => text(t, f),
        ExprKind::Paren(x) => expr(x, false, f),
        ExprKind::List(xs) => xs.items.iter_mut().for_each(|x| expr(&mut x.item, false, f)),
        ExprKind::Record { inits, .. } => inits.items.iter_mut().for_each(|i| expr(&mut i.item.value, false, f)),
        ExprKind::With { target, inits } => {
            expr(target, true, f);
            inits.items.iter_mut().for_each(|i| expr(&mut i.item.value, false, f));
        }
        ExprKind::Field { target, .. } | ExprKind::Try(target) => expr(target, true, f),
        ExprKind::Case { args, .. } => args.iter_mut().for_each(|g| all(g, f)),
        ExprKind::Index { target, index } => {
            expr(target, true, f);
            expr(index, false, f);
        }
        ExprKind::Call { callee, args } => {
            if !matches!(callee.kind, ExprKind::Name(_)) {
                expr(callee, true, f);
            }
            all(args, f);
        }
        ExprKind::Unary { operand, .. } => expr(operand, true, f),
        ExprKind::Binary { left, right, .. } => {
            expr(left, true, f);
            expr(right, true, f);
        }
        ExprKind::Ternary { cond, then, otherwise } => {
            expr(cond, true, f);
            expr(then, true, f);
            expr(otherwise, true, f);
        }
        ExprKind::Range { low, high, step } => [low, high, step].into_iter().flatten().for_each(|x| expr(x, true, f)),
        ExprKind::Lambda(l) => block(&mut l.body, f),
        ExprKind::View(v) => view(v, f),
    }
}

fn all(g: &mut Group<Arg>, f: &mut F) {
    g.items.iter_mut().for_each(|a| expr(&mut a.item.value, false, f));
}

fn text(t: &mut TextLit, f: &mut F) {
    for p in t.parts.iter_mut() {
        if let TextPart::Hole(x) = p {
            expr(x, false, f);
        }
    }
}

fn view(v: &mut ViewCall, f: &mut F) {
    v.args.iter_mut().for_each(|g| all(g, f));
    match v.block.as_mut() {
        Some(CallBlock::Children(b)) => block(b, f),
        Some(CallBlock::Named(b)) => {
            for n in b.items.iter_mut() {
                match &mut n.item {
                    NamedEntry::Input { value, .. } => expr(value, false, f),
                    NamedEntry::Slot { block: b, .. } => block(b, f),
                }
            }
        }
        None => {}
    }
}

fn block(b: &mut Block, f: &mut F) {
    b.stmts.iter_mut().for_each(|s| stmt(&mut s.item, f));
}

fn cond(c: &mut Cond, f: &mut F) {
    match c {
        Cond::Expr(e) | Cond::Open { value: e, .. } => expr(e, false, f),
    }
}

fn stmt(s: &mut Stmt, f: &mut F) {
    match s {
        Stmt::Let(v) | Stmt::Mut(v) => expr(&mut v.value, false, f),
        Stmt::Return(e) => e.iter_mut().for_each(|e| expr(e, false, f)),
        Stmt::Invoke { args, .. } => all(args, f),
        Stmt::Expr(e) => expr(e, false, f),
        Stmt::Assign { place, value } => {
            expr(place, false, f);
            expr(value, false, f);
        }
        Stmt::If(i) => {
            for (c, b) in i.branches.iter_mut() {
                cond(c, f);
                block(b, f);
            }
            i.otherwise.iter_mut().for_each(|b| block(b, f));
        }
        Stmt::For(x) => {
            expr(&mut x.over, false, f);
            block(&mut x.body, f);
        }
        Stmt::Match(m) => {
            expr(&mut m.subject, false, f);
            m.cases.items.iter_mut().for_each(|c| block(&mut c.item.body, f));
        }
    }
}

/// Every call by a name, `pct(x)`, given to `f`; what it gives back goes in its place.
pub fn replace_calls(e: &mut Expr, f: &mut dyn FnMut(&str, &Group<Arg>) -> Option<Expr>) {
    each(e, &mut |x| {
        if let ExprKind::Call { callee, args } = &x.kind {
            if let ExprKind::Name(n) = &callee.kind {
                if let Some(to) = f(&n.name, args) {
                    *x = to;
                }
            }
        }
    });
}

/// Every expression in `e`, itself first.
pub fn each(e: &mut Expr, f: &mut dyn FnMut(&mut Expr)) {
    f(e);
    match &mut e.kind {
        ExprKind::Name(_) | ExprKind::Number(_) | ExprKind::Duration(_) | ExprKind::Bool(_) | ExprKind::SelfValue | ExprKind::State => {}
        ExprKind::Text(t) => t.parts.iter_mut().for_each(|p| {
            if let TextPart::Hole(x) = p {
                each(x, f);
            }
        }),
        ExprKind::Paren(x) | ExprKind::Field { target: x, .. } | ExprKind::Try(x) | ExprKind::Unary { operand: x, .. } => each(x, f),
        ExprKind::List(xs) => xs.items.iter_mut().for_each(|x| each(&mut x.item, f)),
        ExprKind::Record { inits, .. } => inits.items.iter_mut().for_each(|i| each(&mut i.item.value, f)),
        ExprKind::With { target, inits } => {
            each(target, f);
            inits.items.iter_mut().for_each(|i| each(&mut i.item.value, f));
        }
        ExprKind::Case { target, args, .. } => {
            each(target, f);
            args.iter_mut().for_each(|g| g.items.iter_mut().for_each(|a| each(&mut a.item.value, f)));
        }
        ExprKind::Index { target, index } => {
            each(target, f);
            each(index, f);
        }
        ExprKind::Call { callee, args } => {
            each(callee, f);
            args.items.iter_mut().for_each(|a| each(&mut a.item.value, f));
        }
        ExprKind::Binary { left, right, .. } => {
            each(left, f);
            each(right, f);
        }
        ExprKind::Ternary { cond, then, otherwise } => {
            each(cond, f);
            each(then, f);
            each(otherwise, f);
        }
        ExprKind::Range { low, high, step } => [low, high, step].into_iter().flatten().for_each(|x| each(x, f)),
        ExprKind::Lambda(l) => each_in(&mut l.body, f),
        ExprKind::View(v) => {
            v.args.iter_mut().for_each(|g| g.items.iter_mut().for_each(|a| each(&mut a.item.value, f)));
            match v.block.as_mut() {
                Some(CallBlock::Children(b)) => each_in(b, f),
                Some(CallBlock::Named(b)) => {
                    for n in b.items.iter_mut() {
                        match &mut n.item {
                            NamedEntry::Input { value, .. } => each(value, f),
                            NamedEntry::Slot { block, .. } => each_in(block, f),
                        }
                    }
                }
                None => {}
            }
        }
    }
}

/// As `each`, through a block's statements.
pub fn each_in(b: &mut Block, f: &mut dyn FnMut(&mut Expr)) {
    for s in b.stmts.iter_mut() {
        match &mut s.item {
            Stmt::Let(v) | Stmt::Mut(v) => each(&mut v.value, f),
            Stmt::Return(e) => e.iter_mut().for_each(|e| each(e, f)),
            Stmt::Invoke { args, .. } => args.items.iter_mut().for_each(|a| each(&mut a.item.value, f)),
            Stmt::Expr(e) => each(e, f),
            Stmt::Assign { place, value } => {
                each(place, f);
                each(value, f);
            }
            Stmt::If(i) => {
                for (c, b) in i.branches.iter_mut() {
                    match c {
                        Cond::Expr(e) | Cond::Open { value: e, .. } => each(e, f),
                    }
                    each_in(b, f);
                }
                i.otherwise.iter_mut().for_each(|b| each_in(b, f));
            }
            Stmt::For(x) => {
                each(&mut x.over, f);
                each_in(&mut x.body, f);
            }
            Stmt::Match(m) => {
                each(&mut m.subject, f);
                m.cases.items.iter_mut().for_each(|c| each_in(&mut c.item.body, f));
            }
        }
    }
}