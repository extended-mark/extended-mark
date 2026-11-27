//! What one version of a pack declares, read from its files or its surface: each declaration's
//! members with their types as written, which a migration's statements are checked and applied
//! against.

use crate::check::Source;
use crate::syntax::ast::{self, DeclKind, Item, WidgetMember};
use crate::syntax::printer::print_expr;
use crate::syntax::{parse, Entry};

/// A member: its name, its type as written, and its default as written.
#[derive(Clone, Debug, Default)]
pub struct Member {
    pub name: String,
    pub ty: String,
    pub default: Option<String>,
}

/// A declaration as one version has it.
#[derive(Clone, Debug, Default)]
pub struct Decl {
    /// `widget`, `record`, `enum`, `token`, `fn`.
    pub kind: String,
    /// A widget's inputs, a record's fields, a fn's parameters (`self` left out).
    pub inputs: Vec<Member>,
    pub slots: Vec<Member>,
    /// A widget's events, each with its parameters.
    pub events: Vec<(String, Vec<Member>)>,
    /// An enum's or a token type's cases.
    pub cases: Vec<String>,
    /// Whether a fn is called on a value, `t.slug()`.
    pub method: bool,
}

impl Decl {
    pub fn input(&self, name: &str) -> Option<&Member> {
        self.inputs.iter().find(|m| m.name == name)
    }

    pub fn slot(&self, name: &str) -> Option<&Member> {
        self.slots.iter().find(|m| m.name == name)
    }
}

/// The declaration `key` (`chart`, or `text.slug` for a fn on a type) in `sources`.
pub fn decl(sources: &[Source], key: &str) -> Option<Decl> {
    for s in sources {
        let Some(f) = parse(&s.text, Entry::Xtmd).file else { continue };
        let text = |sp: crate::syntax::Span| s.text[sp.range()].to_string();
        for n in &f.items {
            let Item::Decl(d) = &n.item else { continue };
            let field = |x: &ast::Field| Member { name: x.name.name.clone(), ty: text(x.ty.span), default: x.default.as_ref().map(print_expr) };
            let param = |p: &ast::Param| Member { name: p.name.name.clone(), ty: p.ty.as_ref().map(|t| text(t.span)).unwrap_or_default(), default: None };
            match &d.kind {
                DeclKind::Widget(w) | DeclKind::ExternWidget(w) if w.name.name == key => {
                    let mut out = Decl { kind: "widget".into(), inputs: w.inputs.iter().map(field).collect(), ..Decl::default() };
                    for m in w.body.iter().flat_map(|b| b.iter()) {
                        match m {
                            WidgetMember::Slot(sl) => out.slots.push(Member { name: sl.name.name.clone(), ty: text(sl.ty.span), default: None }),
                            WidgetMember::Event(e) => out.events.push((e.name.name.clone(), e.params.iter().map(param).collect())),
                            WidgetMember::Param(x) => out.inputs.push(field(x)),
                            _ => {}
                        }
                    }
                    return Some(out);
                }
                DeclKind::Record(r) if r.name.name == key => {
                    let inputs = r.members.iter().filter_map(|m| if let ast::Member::Field(x) = m { Some(field(x)) } else { None }).collect();
                    return Some(Decl { kind: "record".into(), inputs, ..Decl::default() });
                }
                DeclKind::Enum(e) if e.name.name == key => {
                    return Some(Decl { kind: "enum".into(), cases: e.cases.iter().map(|c| c.name.name.clone()).collect(), ..Decl::default() });
                }
                DeclKind::Fn(x) | DeclKind::ExternFn(x) => {
                    let name = match &x.sig.on {
                        Some((on, _)) => format!("{}.{}", on.name, x.sig.name.name),
                        None => x.sig.name.name.clone(),
                    };
                    if name == key {
                        let inputs = x.sig.params.iter().filter(|p| !p.is_self).map(param).collect();
                        return Some(Decl { kind: "fn".into(), inputs, method: x.sig.on.is_some(), ..Decl::default() });
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Every top-level name `sources` declare, as a pack would provide it.
pub fn names(sources: &[Source]) -> Vec<String> {
    let mut out = Vec::new();
    for s in sources {
        let Some(f) = parse(&s.text, Entry::Xtmd).file else { continue };
        for n in &f.items {
            let Item::Decl(d) = &n.item else { continue };
            match &d.kind {
                DeclKind::Fn(x) | DeclKind::ExternFn(x) if x.sig.on.is_some() => {}
                DeclKind::Tokens(_) | DeclKind::Pack(_) | DeclKind::Migration(_) | DeclKind::MigrationFn(_) => {}
                k => out.push(k.name().name.clone()),
            }
        }
    }
    out
}
