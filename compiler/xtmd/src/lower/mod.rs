//! Lowering: the IR a runtime runs, from a checked module.
//!
//! A page keeps only what it can run (`reach`), with what is the same on every page worked out
//! (`fold`). A widget whose Markdown is its view has it once, and the types the checker worked out for
//! expressions stay behind: a runtime runs values. What each place reads, what each handler writes
//! (`reads`) and which numbers are whole (`whole`) is told.

mod fold;
mod reach;
mod reads;
pub mod walk;
mod whole;

use crate::formats::ir::{self, Expr, Stmt};

/// Lowers a checked module. A pack's keeps all it declares and builds on.
pub fn lower(m: &mut ir::Module) {
    for t in &mut m.templates {
        if t.markdown.as_ref() == Some(&t.view) {
            t.markdown = None;
        }
    }
    if m.module == "doc" {
        reach::prune(m);
        if fold::fold(m) {
            reach::prune(m);
        }
    }
    tell(m);
    walk::module(m, &mut |p| match p {
        walk::Part::Expr(Expr::Call(c)) => c.ty = None,
        walk::Part::Expr(Expr::Op(o)) => o.ty = None,
        walk::Part::Expr(Expr::Apply(a)) => a.ty = None,
        walk::Part::Stmt(Stmt::Call(c)) => c.ty = None,
        _ => {}
    });
}

/// Tells in a module what is read, written and whole: what `lower` does last, for an IR made another
/// way.
pub fn tell(m: &mut ir::Module) {
    whole::tell(m);
    reads::tell(m);
}
