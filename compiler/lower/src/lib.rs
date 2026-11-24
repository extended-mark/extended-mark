//! Lowering: the IR an engine runs, from a checked module.
//!
//! A page keeps only what it can run (`reach`). A widget whose Markdown is its view has it once, and
//! the types the checker worked out for expressions stay behind: an engine runs values.

mod reach;
pub mod walk;

use xtmd_ir::ir::{self, Expr, Stmt};

/// Lowers a checked module. A pack's keeps all it declares and builds on.
pub fn lower(m: &mut ir::Module) {
    for t in &mut m.templates {
        if t.markdown.as_ref() == Some(&t.view) {
            t.markdown = None;
        }
    }
    if m.module == "doc" {
        reach::prune(m);
    }
    walk::module(m, &mut |p| match p {
        walk::Part::Expr(Expr::Call(c)) => c.ty = None,
        walk::Part::Expr(Expr::Op(o)) => o.ty = None,
        walk::Part::Expr(Expr::Apply(a)) => a.ty = None,
        walk::Part::Stmt(Stmt::Call(c)) => c.ty = None,
        _ => {}
    });
}
