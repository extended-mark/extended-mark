//! A migration's statements as they apply: a migration fn's call replaced by its body, its parameters
//! bound, and what each statement names.

use std::collections::BTreeMap;

use crate::syntax::ast::{self, Expr, ExprKind, MigrationStmt, Stmt};

/// A statement's `why`, as `: why` after what it did, or nothing.
pub(crate) fn why_of(args: &[(String, Expr)]) -> String {
    args.iter().find(|(n, _)| n == "why").map(|(_, v)| literal_text(v)).filter(|w| !w.is_empty()).map(|w| format!(": {}", w)).unwrap_or_default()
}

/// One statement, with migration fns' calls replaced by their bodies.
#[derive(Clone, Debug)]
pub(crate) enum Op {
    Call(String, Vec<(String, Expr)>),
    Execute(ast::Block),
    Event(String, Vec<Op>),
    /// A migration fn's `if`: each condition with what it does, then what `else` does.
    If(Vec<(Expr, Vec<Op>)>, Vec<Op>),
}


/// Statements in order, a migration fn's call replaced by its body with its parameters bound.
pub(crate) fn flatten(body: &ast::Body<MigrationStmt>, fns: &BTreeMap<String, ast::MigrationFn>, bound: &BTreeMap<String, Expr>, out: &mut Vec<Op>, depth: usize) {
    for st in body.iter() {
        flatten_one(st, fns, bound, out, depth);
    }
}

fn flatten_one(st: &MigrationStmt, fns: &BTreeMap<String, ast::MigrationFn>, bound: &BTreeMap<String, Expr>, out: &mut Vec<Op>, depth: usize) {
    match st {
        MigrationStmt::Execute(b) => {
            // a migration fn's parameters are what its call gives them, inside the block too
            let mut b = b.clone();
            if !bound.is_empty() {
                crate::syntax::names::replace_names_in(&mut b, &mut |n, tight| {
                    bound.get(n).map(|v| if tight && crate::syntax::names::needs_brackets(v) { Expr { kind: ExprKind::Paren(Box::new(v.clone())), span: v.span } } else { v.clone() })
                });
            }
            out.push(Op::Execute(b));
        }
        MigrationStmt::Event { name, body } => {
            let mut inner = Vec::new();
            flatten(body, fns, bound, &mut inner, depth);
            out.push(Op::Event(name.name.clone(), inner));
        }
        MigrationStmt::Stmt(Stmt::Expr(e)) => {
            let e = subst_bound(e, bound);
            let Some((op, args)) = call(&e) else { return };
            match fns.get(&op) {
                Some(f) if depth < 16 => {
                    let mut inner: BTreeMap<String, Expr> = BTreeMap::new();
                    for (n, v) in &args {
                        inner.insert(n.clone(), v.clone());
                    }
                    flatten(&f.body, fns, &inner, out, depth + 1);
                }
                _ => out.push(Op::Call(op, args)),
            }
        }
        MigrationStmt::Stmt(Stmt::For(f)) => {
            let over = subst_bound(&f.over, bound);
            if let ExprKind::List(xs) = &over.kind {
                for x in xs.iter() {
                    let mut inner = bound.clone();
                    inner.insert(f.name.name.clone(), x.clone());
                    for s in f.body.stmts.iter() {
                        flatten_one(&MigrationStmt::Stmt(s.item.clone()), fns, &inner, out, depth);
                    }
                }
            }
        }
        MigrationStmt::Stmt(Stmt::If(i)) => {
            let block = |b: &ast::Block| {
                let mut inner = Vec::new();
                for s in b.stmts.iter() {
                    flatten_one(&MigrationStmt::Stmt(s.item.clone()), fns, bound, &mut inner, depth);
                }
                inner
            };
            let mut branches = Vec::new();
            for (c, b) in &i.branches {
                // an `if` that opens an opt has nothing to open in a migration
                let ast::Cond::Expr(c) = c else { continue };
                branches.push((subst_bound(c, bound), block(b)));
            }
            out.push(Op::If(branches, i.otherwise.as_ref().map(block).unwrap_or_default()));
        }
        MigrationStmt::Stmt(_) => {}
    }
}

pub(crate) fn subst_bound(e: &Expr, bound: &BTreeMap<String, Expr>) -> Expr {
    let mut e = e.clone();
    walk_names(&mut e, &mut |n| bound.get(n).cloned());
    e
}

/// Each name that stands for a value, as `f` replaces it; a case's type and a fn's name stay.
pub(crate) fn walk_names(e: &mut Expr, f: &mut dyn FnMut(&str) -> Option<Expr>) {
    // through lambdas, holes and blocks too; a value put where its place reads tighter is bracketed
    crate::syntax::names::replace_names(e, &mut |n, tight| {
        f(n).map(|v| if tight && crate::syntax::names::needs_brackets(&v) { Expr { kind: ExprKind::Paren(Box::new(v.clone())), span: v.span } } else { v })
    });
}


pub(crate) fn call(e: &Expr) -> Option<(String, Vec<(String, Expr)>)> {
    let ExprKind::Call { callee, args } = &e.kind else { return None };
    let ExprKind::Name(n) = &callee.kind else { return None };
    Some((n.name.clone(), args.iter().map(|a| (a.name.as_ref().map(|i| i.name.clone()).unwrap_or_default(), a.value.clone())).collect()))
}

pub(crate) fn bare(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::Name(n) => Some(n.name.clone()),
        _ => None,
    }
}

pub(crate) fn arg_name(args: &[(String, Expr)], name: &str) -> Option<String> {
    args.iter().find(|(n, _)| n == name).and_then(|(_, v)| bare(v))
}

/// An argument written `type.name`, as one string.
pub(crate) fn dotted(args: &[(String, Expr)], name: &str) -> Option<String> {
    match &args.iter().find(|(n, _)| n == name)?.1.kind {
        ExprKind::Field { target, name } => bare(target).map(|t| format!("{}.{}", t, name.name)),
        _ => None,
    }
}

pub(crate) fn member(args: &[(String, Expr)]) -> Option<String> {
    args.iter().find(|(n, _)| matches!(n.as_str(), "input" | "slot" | "field" | "event" | "arg")).and_then(|(_, v)| bare(v))
}

pub(crate) fn case_of(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::Case { name, .. } => Some(name.name.clone()),
        // `state.B`, which reads as a field until the checker says it is a case
        ExprKind::Field { target, name } if matches!(target.kind, ExprKind::Name(_)) => Some(name.name.clone()),
        _ => None,
    }
}

/// A place: `color`, or `style.color`.
pub(crate) fn place(e: &Expr) -> Option<Vec<String>> {
    match &e.kind {
        ExprKind::Name(n) => Some(vec![n.name.clone()]),
        ExprKind::Field { target, name } => {
            let mut p = place(target)?;
            p.push(name.name.clone());
            Some(p)
        }
        _ => None,
    }
}

/// A text literal's words, as written.
pub(crate) fn literal_text(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Text(t) => t.parts.iter().map(|p| match p {
            ast::TextPart::Piece { raw, .. } => raw.clone(),
            _ => String::new(),
        }).collect(),
        _ => String::new(),
    }
}

