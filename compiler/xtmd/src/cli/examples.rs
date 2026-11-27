//! `example(before = …, after = …)` in a pack's migrations (page 6): `xtmd check` runs each `before`
//! through its migration, as `xtmd migrate` would move a use of it, and an `after` it doesn't come to
//! is an error at the example.

use crate::check::Source;
use crate::syntax::ast::{self, DeclKind, Expr, ExprKind, Item, MigrationItem, MigrationStmt, Stmt};
use crate::syntax::printer::print_expr;
use crate::syntax::{format, parse, Diagnostic, Entry, Span};

use crate::cli::migrate::packs_of;

/// The code of an example that fails.
pub(crate) const EXAMPLE: &str = "pack.migration-example";

/// Every example in a pack file's migrations, run; an error for each that doesn't hold.
pub(crate) fn check(base: &crate::check::Checker, file: &str, text: &str, packs: &[Source]) -> Vec<Diagnostic> {
    let Some(f) = parse(text, Entry::Xtmd).file else { return Vec::new() };
    let mut out = Vec::new();
    for n in &f.items {
        let Item::Decl(ast::Decl { kind: DeclKind::Migration(m), .. }) = &n.item else { continue };
        for item in &m.items.items {
            let MigrationItem::Block(b) = &item.item else { continue };
            let mut examples = Vec::new();
            examples_in(&b.body, &mut examples);
            for (at, before, after) in examples {
                if let Err(why) = holds(base, file, text, packs, m, b, before, after) {
                    out.push(Diagnostic::error(EXAMPLE, at, why));
                }
            }
        }
    }
    out
}

/// The examples in a block, its event blocks' included: where each is, its `before` and its `after`.
fn examples_in<'a>(body: &'a ast::Body<MigrationStmt>, out: &mut Vec<(Span, &'a Expr, &'a Expr)>) {
    for s in &body.items {
        match &s.item {
            MigrationStmt::Stmt(Stmt::Expr(e)) => {
                let ExprKind::Call { callee, args } = &e.kind else { continue };
                if !matches!(&callee.kind, ExprKind::Name(n) if n.name == "example") {
                    continue;
                }
                let arg = |name: &str| args.items.iter().find(|a| a.item.name.as_ref().map_or(false, |n| n.name == name)).map(|a| &a.item.value);
                if let (Some(b), Some(a)) = (arg("before"), arg("after")) {
                    out.push((e.span, b, a));
                }
            }
            MigrationStmt::Event { body, .. } => examples_in(body, out),
            _ => {}
        }
    }
}

/// A document that holds one value: a use as a line of prose, anything else as a `let` of the
/// block's own type.
fn holder(pack: &str, ty: &str, value: &str, view: bool) -> String {
    if view {
        format!("import {};\n@@@\n{};\n", pack, value)
    } else {
        format!("import {};\n\nlet example: {} = {};\n@@@\n", pack, ty, value)
    }
}

/// What the holder holds, read back.
fn held(doc: &str, view: bool) -> String {
    if view {
        doc.split_once("@@@\n").map_or(doc, |(_, p)| p).trim().trim_end_matches(';').to_string()
    } else {
        doc.split_once(" = ").map_or(doc, |(_, v)| v).split_once(";\n").map_or(doc, |(v, _)| v).trim().to_string()
    }
}

#[allow(clippy::too_many_arguments)]
fn holds(base: &crate::check::Checker, file: &str, text: &str, packs: &[Source], m: &ast::MigrationDecl, b: &ast::MigrationBlock, before: &Expr, after: &Expr) -> Result<(), String> {
    let name = m.from.name.name.clone();
    let from: u32 = m.from.version.name.parse().map_err(|_| "the migration's version isn't a number".to_string())?;
    let me = std::fs::canonicalize(file).ok();
    let mut sources: Vec<Source> = packs.iter().filter(|s| s.path != file && (me.is_none() || std::fs::canonicalize(&s.path).ok() != me)).cloned().collect();
    sources.push(Source { path: file.to_string(), text: text.to_string() });
    let pack = packs_of(&sources).into_iter().find(|p| p.name == name).ok_or_else(|| format!("{} isn't in the build", name))?;
    let view = matches!(before.kind, ExprKind::View(_));
    let key = b.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".");
    if !view && matches!(b.kind.name.as_str(), "widget" | "fn") {
        return Err(format!("an example of a {} block is a use of it: `before = @{}(…)`", b.kind.name, key));
    }
    let (doc, want) = (holder(&name, &key, &print_expr(before), view), holder(&name, &key, &print_expr(after), view));
    let path = format!("{}.example.xtmd", file);
    let (moved, done) = crate::migrate::packs::move_file(base, &sources, &pack, from, &path, &[Source { path: path.clone(), text: doc.clone() }], "doc");
    if let Some(why) = done.refused.first() {
        if why.starts_with("it doesn't compile against") {
            return Err(format!("`before` doesn't compile against {}@{}: {}", name, from, why.rsplit(": ").next().unwrap_or(why)));
        }
        return Err(format!("`before` can't be moved: {}", why.split_once(": ").map_or(why.as_str(), |(_, w)| w)));
    }
    let moved = moved.unwrap_or(doc);
    let canonical = |t: &str| format(t, Entry::Document).unwrap_or_else(|_| t.to_string());
    // what the holder holds, its imports aside: a use moved to another pack's widget imports it
    let (got, want) = (held(&canonical(&moved), view), held(&canonical(&want), view));
    if got != want {
        return Err(format!("`before` becomes `{}`, not `after`", got));
    }
    Ok(())
}
