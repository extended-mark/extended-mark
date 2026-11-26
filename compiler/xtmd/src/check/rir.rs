//! The resolved IR (schemas/rir.schema.json): what the checker understood of a build, written out.
//! It walks each file's syntax tree, with what the checker found for it: what each name
//! resolved to (the reference log), and each expression's type and what it compiled to.

use std::collections::HashMap;

use crate::formats::ir;
use crate::syntax::ast::{self, DeclKind, Expr, ExprKind, Item, Stmt};
use crate::syntax::lexer::Comment;
use crate::syntax::span::Span;
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::check::ty::Ty;
use crate::check::{compile_with, Checker, Compiled, DefKind, RefTarget, Source, TypeSym};

/// What else a resolved IR holds, for an editor.
#[derive(Clone, Copy, Debug, Default)]
pub struct RirOptions {
    /// Core's own files too, before the build's: what an editor completes, shows and goes to. They
    /// are resolved when the base was compiled by `compile_core_logged`.
    pub core: bool,
    /// Written when the build has errors too, with what the checker resolved before it stopped: an
    /// editor's draft is often broken, and its names and types still help.
    pub partial: bool,
}

/// Compiles as `compile_on` does, and writes the resolved IR of every file it adds when the
/// build compiles.
pub fn compile_rir(base: &Checker, packs: &[Vec<Source>], main: &[Source], module: &str) -> (Compiled, Option<Value>) {
    compile_rir_with(base, packs, main, module, RirOptions::default())
}

/// `compile_rir`, with core's files and a broken build's resolved IR when asked.
pub fn compile_rir_with(base: &Checker, packs: &[Vec<Source>], main: &[Source], module: &str, opts: RirOptions) -> (Compiled, Option<Value>) {
    let mut c = base.clone();
    // a base compiled by compile_core_logged keeps what core's names resolved to
    c.refs.get_or_insert_with(Vec::new);
    c.typed.get_or_insert_with(HashMap::new);
    let first = c.files.len();
    let (compiled, mut c) = compile_with(c, packs, main, module);
    if compiled.module.is_none() && !opts.partial {
        return (compiled, None);
    }
    let mut w = Writer::new(&mut c);
    let files: Vec<Value> = (if opts.core { 0 } else { first }..w.c.files.len()).map(|f| w.file(f)).collect();
    let packs = w.packs();
    let mut root = json!({ "rir": RIR_VERSION, "xtmd": env!("CARGO_PKG_VERSION"), "packs": packs, "files": files });
    // what a document needs to run, as its IR says: the levels
    if module == "doc" {
        if let Some(m) = &compiled.module {
            root["levels"] = serde_json::to_value(&m.levels).unwrap_or_default();
        }
    }
    (compiled, Some(root))
}

/// This format's version (schemas/rir.schema.json): a whole number of its own, raised when a part
/// is changed or taken away. The schemas of earlier ones are kept in schemas/rir-history.
pub const RIR_VERSION: u32 = 2;

struct Writer<'c> {
    c: &'c mut Checker,
    /// What each name at a span resolved to: (file, start, end) → the reference.
    refs: HashMap<(usize, u32, u32), RefTarget>,
    typed: HashMap<(usize, u32, u32), (Ty, ir::Expr)>,
    /// The free comments of the nodes being written, innermost last, each given to the first
    /// expression after it.
    free: Vec<Vec<Comment>>,
}

impl<'c> Writer<'c> {
    fn new(c: &'c mut Checker) -> Writer<'c> {
        let mut refs = HashMap::new();
        for r in c.refs.clone().unwrap_or_default() {
            refs.entry((r.file, r.at.start, r.at.end)).or_insert(r.target);
        }
        let typed = c.typed.clone().unwrap_or_default();
        Writer { c, refs, typed, free: Vec::new() }
    }

    fn packs(&self) -> Vec<Value> {
        let mut out = Vec::new();
        for (name, rec) in &self.c.packs {
            if name == "core" || *name == self.c.module || rec.files.is_empty() {
                continue;
            }
            let (base, surface) = match name.split_once('@') {
                Some((b, _)) => (b.to_string(), true),
                None => (name.clone(), false),
            };
            let mut p = Map::new();
            p.insert("name".into(), json!(base));
            let version = match name.split_once('@') {
                Some((_, v)) => v.parse::<i64>().unwrap_or(0),
                None => self.c.pack_block(name).map(|(_, _, v)| v as i64).unwrap_or(0),
            };
            p.insert("version".into(), json!(version));
            if surface {
                p.insert("surface".into(), json!(true));
            }
            let mut uses = Vec::new();
            for &f in &rec.files {
                for n in &self.c.files[f].ast.items {
                    if let Item::Decl(ast::Decl { kind: DeclKind::Pack(b), .. }) = &n.item {
                        for l in b.lines.iter() {
                            if let ast::PackLine::Uses(rs) = l {
                                for r in rs {
                                    uses.push(json!({ "name": r.name.name, "version": r.version.name.parse::<i64>().unwrap_or(0) }));
                                }
                            }
                        }
                    }
                }
            }
            if !uses.is_empty() {
                p.insert("uses".into(), json!(uses));
            }
            if let Some(pr) = &rec.provides {
                let mut v: Vec<&String> = pr.iter().collect();
                v.sort();
                p.insert("provides".into(), json!(v));
            }
            out.push(Value::Object(p));
        }
        out
    }

    // ---- comments

    /// A list's entries: each node's value with its comments, the comments with a blank line after
    /// them as comment items in their place, and those before the list's closing bracket last.
    fn entries<T: Serialize>(&mut self, nodes: &[ast::Node<T>], end: &[ast::Leading], mut each: impl FnMut(&mut Self, &ast::Node<T>) -> Value) -> Vec<Value> {
        let mut out = Vec::new();
        for n in nodes {
            let (c, before, free) = note(n);
            out.extend(before);
            self.free.push(free);
            let v = each(self, n);
            self.free.pop();
            out.push(with(v, c));
        }
        out.extend(free_items(end));
        out
    }

    /// A body whose members rir writes in lists of their own, by kind: `each` puts each where it
    /// goes, with its comments. The comment items, between them and at the end, are returned for
    /// the owner's `comment_items`.
    fn members<T: Serialize>(&mut self, nodes: &[ast::Node<T>], end: &[ast::Leading], mut each: impl FnMut(&mut Self, &ast::Node<T>, Option<Value>)) -> Vec<Value> {
        let mut items = Vec::new();
        for n in nodes {
            let (c, before, free) = note(n);
            items.extend(before);
            self.free.push(free);
            each(self, n, c);
            self.free.pop();
        }
        items.extend(free_items(end));
        items
    }

    /// The free comments written before `at`, which go on the expression that starts there.
    fn free_before(&mut self, at: u32) -> Vec<String> {
        let Some(top) = self.free.last_mut() else { return Vec::new() };
        let k = top.iter().take_while(|c| c.span.start < at).count();
        top.drain(..k).map(|c| c.text).collect()
    }

    // ---- files

    fn file(&mut self, f: usize) -> Value {
        let rec = self.c.files[f].clone();
        let ast = rec.ast.clone();
        let mut o = Map::new();
        o.insert("path".into(), json!(rec.path));
        let kind = if ast.surface.is_some() {
            "surface"
        } else if matches!(ast.kind, ast::FileKind::Document) {
            "document"
        } else {
            "pack"
        };
        o.insert("kind".into(), json!(kind));
        if let Some(p) = &ast.package {
            o.insert("package".into(), json!(p.item.name));
        }
        o.insert("sha256".into(), json!(sha256_hex(rec.src.as_bytes())));
        if let Some(fm) = ast.front_matter {
            o.insert("front_matter".into(), json!({ "text": front_matter_text(&rec.src[fm.range()]), "span": span(fm) }));
        }
        if let Some(p) = &ast.package {
            let (c, head, _) = note(p);
            if let Some(c) = c {
                o.insert("package_comments".into(), c);
            }
            if !head.is_empty() {
                o.insert("head".into(), json!(head));
            }
        }
        let (mut imports, mut decls, mut expose, mut state) = (Vec::new(), Vec::new(), Vec::new(), None);
        let items = self.members(&ast.items, &ast.dangling, |w, n, c| match &n.item {
            Item::Import(i) => imports.push(with(w.import(f, i), c)),
            Item::Decl(d) => decls.push(with(w.decl(f, d), c)),
            Item::State(s) => state = Some(with(w.state(f, s), c)),
            Item::Expose(e) => expose.push(with(w.expose(f, e), c)),
            _ => {}
        });
        if let Some(s) = state {
            o.insert("state".into(), s);
        }
        o.insert("imports".into(), json!(imports));
        o.insert("declarations".into(), json!(decls));
        if !expose.is_empty() {
            o.insert("expose".into(), json!(expose));
        }
        if let Some(p) = &ast.prose {
            o.insert("prose".into(), json!(self.prose(f, p)));
        }
        if !items.is_empty() {
            o.insert("comment_items".into(), json!(items));
        }
        Value::Object(o)
    }

    fn import(&mut self, f: usize, i: &ast::Import) -> Value {
        let mut o = Map::new();
        o.insert("pack".into(), json!(i.path[0].name));
        if let Some(one) = i.path.get(1) {
            o.insert("name".into(), json!(one.name));
            if let Some(t) = self.target_at(f, one.span) {
                o.insert("target".into(), json!(t));
            }
        }
        if let Some(a) = &i.alias {
            o.insert("as".into(), json!(a.name));
        }
        o.insert("span".into(), span(i.span));
        Value::Object(o)
    }

    // ---- declarations

    fn decl(&mut self, f: usize, d: &ast::Decl) -> Value {
        let name = d.kind.name().clone();
        let id = self.def_at(f, name.span).map(|d| self.c.defs[d].full.clone()).unwrap_or_else(|| format!("{}/{}", self.c.files[f].pack, name.name));
        let mut o = Map::new();
        o.insert("id".into(), json!(id));
        o.insert("name_span".into(), span(name.span));
        o.insert("span".into(), span(d.span));
        let key = match &d.kind {
            DeclKind::Enum(e) => {
                o.insert("cases".into(), json!(self.entries(&e.cases.items, &e.cases.dangling, |w, c| w.case_decl(f, &id, &c.item))));
                "enum"
            }
            DeclKind::Token(t) => {
                o.insert("value".into(), json!(self.ty_of_ast(f, &t.ty)));
                o.insert("cases".into(), json!(self.entries(&t.cases.items, &t.cases.dangling, |w, c| w.case_decl(f, &id, &c.item))));
                "token"
            }
            DeclKind::Record(r) => {
                let (mut fields, mut requires) = (Vec::new(), Vec::new());
                let items = self.members(&r.members.items, &r.members.dangling, |w, m, c| match &m.item {
                    ast::Member::Field(fl) => fields.push(with(w.field(f, &id, fl), c)),
                    ast::Member::Require(q) => requires.push(with(w.require(f, q), c)),
                });
                o.insert("fields".into(), json!(fields));
                if !requires.is_empty() {
                    o.insert("requires".into(), json!(requires));
                }
                put_items(&mut o, items);
                "record"
            }
            DeclKind::Alias(a) => {
                o.insert("is".into(), json!(self.ty_of_ast(f, &a.ty)));
                "type"
            }
            DeclKind::Interface(i) => {
                if i.widget {
                    o.insert("widget".into(), json!(true));
                }
                let (mut fields, mut fns, mut events) = (Vec::new(), Vec::new(), Vec::new());
                let items = self.members(&i.members.items, &i.members.dangling, |w, m, c| match &m.item {
                    ast::IfaceMember::Field(fl) => fields.push(with(w.field(f, &id, fl), c)),
                    ast::IfaceMember::Fn(sig) => fns.push(with(w.signature(f, &id, sig), c)),
                    ast::IfaceMember::Event(e) => events.push(with(w.event(f, &id, e), c)),
                });
                o.insert("fields".into(), json!(fields));
                o.insert("fns".into(), json!(fns));
                o.insert("events".into(), json!(events));
                put_items(&mut o, items);
                "interface"
            }
            DeclKind::Widget(w) | DeclKind::ExternWidget(w) => {
                self.widget(f, &id, w, &mut o);
                if matches!(d.kind, DeclKind::Widget(_)) {
                    "widget"
                } else {
                    "extern_widget"
                }
            }
            DeclKind::Fn(fd) | DeclKind::ExternFn(fd) => {
                let fid = self.fn_id(f, &fd.sig).unwrap_or(id.clone());
                o.insert("id".into(), json!(fid));
                self.fn_parts(f, fd, &mut o);
                if matches!(d.kind, DeclKind::Fn(_)) {
                    "fn"
                } else {
                    "extern_fn"
                }
            }
            DeclKind::ExternType(_) => "extern_type",
            DeclKind::ExternBlock(b) => {
                let (mut values, mut fns, mut parts) = (Vec::new(), Vec::new(), Vec::new());
                let mut tokens: Vec<String> = Vec::new();
                let items = self.members(&b.members.items, &b.members.dangling, |w, m, c| match &m.item {
                    ast::ExternMember::Field(fl) => values.push(with(w.field(f, &id, fl), c)),
                    ast::ExternMember::Fn(sig) => fns.push(with(w.signature(f, &id, sig), c)),
                    ast::ExternMember::Tokens(refs) => {
                        tokens.extend(refs.iter().map(token_text));
                        part(&mut parts, "tokens", c);
                    }
                });
                o.insert("values".into(), json!(values));
                o.insert("fns".into(), json!(fns));
                if !tokens.is_empty() {
                    o.insert("tokens".into(), json!(tokens));
                }
                put_items(&mut o, items);
                put_parts(&mut o, parts);
                "extern"
            }
            DeclKind::Let(v) => {
                o.insert("type".into(), json!(self.ty_of_ast(f, &v.ty)));
                o.insert("value".into(), self.expr(f, &v.value));
                "let"
            }
            DeclKind::Tokens(t) => {
                let lines = self.entries(&t.lines.items, &t.lines.dangling, |w, l| {
                    let token = w.type_full(f, &l.item.ty.name).unwrap_or_else(|| l.item.ty.name.clone());
                    let mut lo = Map::new();
                    lo.insert("token".into(), json!(token));
                    lo.insert("case".into(), json!(l.item.case.name));
                    lo.insert("value".into(), w.expr(f, &l.item.value));
                    lo.insert("span".into(), span(l.item.ty.span.to(l.item.value.span)));
                    Value::Object(lo)
                });
                o.insert("lines".into(), json!(lines));
                "tokens"
            }
            DeclKind::Pack(p) => {
                o.insert("version".into(), json!(p.version.name.parse::<i64>().unwrap_or(0)));
                let (mut uses, mut provides, mut implements, mut parts) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                let items = self.members(&p.lines.items, &p.lines.dangling, |_, n, c| {
                    let key = match &n.item {
                        ast::PackLine::Uses(_) => "uses",
                        ast::PackLine::Provides(_) => "provides",
                        ast::PackLine::Implements(_) => "implements",
                    };
                    part(&mut parts, key, c);
                });
                for l in p.lines.iter() {
                    match l {
                        ast::PackLine::Uses(rs) => uses.extend(rs.iter().map(|r| json!({ "name": r.name.name, "version": r.version.name.parse::<i64>().unwrap_or(0), "span": span(r.name.span.to(r.version.span)) }))),
                        ast::PackLine::Provides(ns) => provides.extend(ns.iter().map(|n| json!(n.name))),
                        ast::PackLine::Implements(rs) => implements.extend(rs.iter().map(|r| json!(format!("{}.{}@{}", r.pack.name, r.level.name, r.version.name)))),
                    }
                }
                o.insert("uses".into(), json!(uses));
                o.insert("provides".into(), json!(provides));
                if !implements.is_empty() {
                    o.insert("implements".into(), json!(implements));
                }
                put_items(&mut o, items);
                put_parts(&mut o, parts);
                "pack"
            }
            DeclKind::Migration(m) => {
                o.remove("id");
                o.remove("name_span");
                o.insert("from".into(), json!({ "name": m.from.name.name, "version": m.from.version.name.parse::<i64>().unwrap_or(0) }));
                o.insert("to".into(), json!({ "name": m.to.name.name, "version": m.to.version.name.parse::<i64>().unwrap_or(0) }));
                let (mut fns, mut blocks) = (Vec::new(), Vec::new());
                let items = self.members(&m.items.items, &m.items.dangling, |w, item, c| match &item.item {
                    ast::MigrationItem::Fn(fd) => {
                        let mut fo = Map::new();
                        fo.insert("id".into(), json!(format!("{}/{}", w.c.files[f].pack, fd.sig.name.name)));
                        fo.insert("name_span".into(), span(fd.sig.name.span));
                        fo.insert("span".into(), span(fd.sig.span));
                        w.fn_parts(f, fd, &mut fo);
                        fns.push(with(Value::Object(fo), c));
                    }
                    ast::MigrationItem::Block(b) => {
                        let key = b.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".");
                        let mut bo = Map::new();
                        bo.insert("kind".into(), json!(b.kind.name));
                        bo.insert("target".into(), json!(format!("{}@{}/{}", m.from.name.name, m.from.version.name, key)));
                        bo.insert("name_span".into(), span(b.path[0].span.to(b.path[b.path.len() - 1].span)));
                        bo.insert("body".into(), json!(w.migration_body(f, &b.body)));
                        bo.insert("span".into(), span(b.kind.span.to(b.body.span)));
                        blocks.push(with(Value::Object(bo), c));
                    }
                });
                if !fns.is_empty() {
                    o.insert("fns".into(), json!(fns));
                }
                o.insert("blocks".into(), json!(blocks));
                put_items(&mut o, items);
                "migration"
            }
            DeclKind::MigrationFn(mf) => {
                o.insert("id".into(), json!(format!("{}/{}", self.c.files[f].pack, mf.sig.name.name)));
                o.insert("params".into(), json!(self.params(f, &mf.sig.params)));
                o.insert("body".into(), json!(self.migration_body(f, &mf.body)));
                "migration_fn"
            }
        };
        json!({ key: Value::Object(o) })
    }

    fn widget(&mut self, f: usize, id: &str, w: &ast::WidgetDecl, o: &mut Map<String, Value>) {
        let mut inputs = self.entries(&w.inputs.items, &w.inputs.dangling, |wr, i| wr.field(f, id, &i.item));
        let (mut slots, mut events, mut requires, mut own, mut lets, mut fns, mut view, mut markdown) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut parts = Vec::new();
        let no_members: Vec<ast::Node<ast::WidgetMember>> = Vec::new();
        let (members, end) = match &w.body {
            Some(b) => (&b.items, b.dangling.as_slice()),
            None => (&no_members, &[][..]),
        };
        let items = self.members(members, end, |wr, m, c| match &m.item {
            ast::WidgetMember::Param(fl) => {
                let mut v = wr.field(f, id, fl);
                v["param"] = json!(true);
                inputs.push(with(v, c));
            }
            ast::WidgetMember::Slot(s) => {
                let mut so = Map::new();
                so.insert("name".into(), json!(s.name.name));
                so.insert("id".into(), json!(format!("{}.{}", id, s.name.name)));
                so.insert("type".into(), json!(wr.ty_of_ast(f, &s.ty)));
                so.insert("span".into(), span(s.name.span.to(s.ty.span)));
                slots.push(with(Value::Object(so), c));
            }
            ast::WidgetMember::Event(e) => events.push(with(wr.event(f, id, e), c)),
            ast::WidgetMember::Require(q) => requires.push(with(wr.require(f, q), c)),
            ast::WidgetMember::Own(v) => own.push(with(wr.var(f, v, true), c)),
            ast::WidgetMember::Let(v) => lets.push(with(wr.var(f, v, false), c)),
            ast::WidgetMember::Fn(fd) => {
                let mut fo = Map::new();
                fo.insert("id".into(), json!(wr.fn_id(f, &fd.sig).unwrap_or_else(|| format!("{}.{}", id, fd.sig.name.name))));
                fo.insert("name_span".into(), span(fd.sig.name.span));
                fo.insert("span".into(), span(fd.sig.span));
                wr.fn_parts(f, fd, &mut fo);
                fns.push(with(Value::Object(fo), c));
            }
            ast::WidgetMember::View(b) => {
                view = wr.stmts(f, b);
                part(&mut parts, "view", c);
            }
            ast::WidgetMember::Tokens(refs) => {
                let names: Vec<String> = refs.iter().map(token_text).collect();
                o.insert("tokens".into(), json!(names));
                part(&mut parts, "tokens", c);
            }
            ast::WidgetMember::Uses(refs) => {
                o.insert("uses".into(), json!(refs.iter().map(|u| format!("{}.{}", u.pack.name, u.level.name)).collect::<Vec<_>>()));
                part(&mut parts, "uses", c);
            }
            ast::WidgetMember::Markdown(md) => {
                if let Some(p) = &md.piece {
                    o.insert("piece".into(), json!(p.name));
                }
                markdown = wr.prose(f, &md.prose);
                part(&mut parts, "markdown", c);
            }
        });
        o.insert("inputs".into(), json!(inputs));
        for (k, v) in [("slots", slots), ("events", events), ("requires", requires), ("own", own), ("lets", lets), ("fns", fns), ("view", view), ("markdown", markdown)] {
            if !v.is_empty() || k == "slots" || k == "events" {
                o.insert(k.into(), json!(v));
            }
        }
        put_items(o, items);
        put_parts(o, parts);
    }

    fn fn_parts(&mut self, f: usize, fd: &ast::FnDecl, o: &mut Map<String, Value>) {
        if let Some((on, _)) = &fd.sig.on {
            let t = self.type_full(f, &on.name).unwrap_or_else(|| on.name.clone());
            o.insert("on".into(), json!(t));
        }
        o.insert("params".into(), json!(self.params(f, &fd.sig.params)));
        if let Some(ast::RetType::Type(t)) = &fd.sig.ret {
            o.insert("returns".into(), json!(self.ty_of_ast(f, t)));
        }
        if let Some(b) = &fd.body {
            o.insert("body".into(), json!(self.stmts(f, b)));
        }
    }

    fn signature(&mut self, f: usize, owner: &str, s: &ast::FnSig) -> Value {
        let mut o = Map::new();
        o.insert("name".into(), json!(s.name.name));
        o.insert("id".into(), json!(format!("{}.{}", owner, s.name.name)));
        o.insert("params".into(), json!(self.params(f, &s.params)));
        if let Some(ast::RetType::Type(t)) = &s.ret {
            o.insert("returns".into(), json!(self.ty_of_ast(f, t)));
        }
        o.insert("span".into(), span(s.span));
        Value::Object(o)
    }

    fn event(&mut self, f: usize, owner: &str, e: &ast::EventDecl) -> Value {
        json!({
            "name": e.name.name,
            "id": format!("{}.{}", owner, e.name.name),
            "params": self.params(f, &e.params),
            "span": span(e.name.span.to(e.params.span)),
        })
    }

    fn params(&mut self, f: usize, g: &ast::Group<ast::Param>) -> Vec<Value> {
        self.entries(&g.items, &g.dangling, |w, p| w.param(f, &p.item))
    }

    fn param(&mut self, f: usize, p: &ast::Param) -> Value {
        let mut o = Map::new();
        o.insert("name".into(), json!(p.name.name));
        if let Some(t) = &p.ty {
            o.insert("type".into(), json!(self.ty_of_ast(f, t)));
        }
        if p.mutable {
            o.insert("mutable".into(), json!(true));
        }
        if p.is_self {
            o.insert("self".into(), json!(true));
        }
        o.insert("span".into(), span(p.ty.as_ref().map_or(p.name.span, |t| p.name.span.to(t.span))));
        Value::Object(o)
    }

    fn case_decl(&mut self, f: usize, owner: &str, c: &ast::CaseDecl) -> Value {
        let mut o = Map::new();
        o.insert("name".into(), json!(c.name.name));
        let at = format!("{}.{}", owner, c.name.name);
        let fields = match &c.fields {
            Some(g) => self.entries(&g.items, &g.dangling, |w, fl| w.field(f, &at, &fl.item)),
            None => Vec::new(),
        };
        if !fields.is_empty() {
            o.insert("fields".into(), json!(fields));
        }
        let sp = c.fields.as_ref().map_or(c.name.span, |g| c.name.span.to(g.span));
        o.insert("span".into(), span(sp));
        Value::Object(o)
    }

    fn field(&mut self, f: usize, owner: &str, fl: &ast::Field) -> Value {
        let mut o = Map::new();
        o.insert("name".into(), json!(fl.name.name));
        o.insert("id".into(), json!(format!("{}.{}", owner, fl.name.name)));
        o.insert("type".into(), json!(self.ty_of_ast(f, &fl.ty)));
        if let Some(d) = &fl.default {
            o.insert("default".into(), self.expr(f, d));
        }
        if fl.mutable {
            o.insert("mutable".into(), json!(true));
        }
        if fl.param {
            o.insert("param".into(), json!(true));
        }
        o.insert("name_span".into(), span(fl.name.span));
        o.insert("span".into(), span(fl.span));
        Value::Object(o)
    }

    fn require(&mut self, f: usize, q: &ast::Require) -> Value {
        let mut v = json!({ "message": text_of(&q.message), "body": self.stmts(f, &q.body), "span": span(q.message.span.to(q.body.span)) });
        if q.warn {
            v["warn"] = json!(true);
        }
        v
    }

    fn var(&mut self, f: usize, v: &ast::Var, mutable: bool) -> Value {
        let mut o = Map::new();
        o.insert("name".into(), json!(v.name.name));
        o.insert("type".into(), json!(self.ty_of_ast(f, &v.ty)));
        o.insert("value".into(), self.expr(f, &v.value));
        if mutable {
            o.insert("mutable".into(), json!(true));
        }
        o.insert("span".into(), span(v.name.span.to(v.value.span)));
        Value::Object(o)
    }

    fn state(&mut self, f: usize, s: &ast::StateBlock) -> Value {
        let (mut fields, mut requires) = (Vec::new(), Vec::new());
        let items = self.members(&s.members.items, &s.members.dangling, |w, m, c| match &m.item {
            ast::Member::Field(fl) => {
                let mut v = w.field(f, "$state", fl);
                v["id"] = json!(format!("$state.{}", fl.name.name));
                v["mutable"] = json!(true);
                fields.push(with(v, c));
            }
            ast::Member::Require(q) => requires.push(with(w.require(f, q), c)),
        });
        let mut o = Map::new();
        o.insert("fields".into(), json!(fields));
        if !requires.is_empty() {
            o.insert("requires".into(), json!(requires));
        }
        o.insert("span".into(), span(s.span));
        put_items(&mut o, items);
        Value::Object(o)
    }

    fn expose(&mut self, f: usize, e: &ast::Expose) -> Value {
        let channel = self.channel_full(f, &e.name.name).unwrap_or_else(|| e.name.name.clone());
        let fns = self.entries(&e.fns.items, &e.fns.dangling, |w, fd| {
            let mut fo = Map::new();
            fo.insert("id".into(), json!(format!("{}.{}", channel, fd.item.sig.name.name)));
            fo.insert("name_span".into(), span(fd.item.sig.name.span));
            fo.insert("span".into(), span(fd.item.sig.span));
            w.fn_parts(f, &fd.item, &mut fo);
            Value::Object(fo)
        });
        let mut o = Map::new();
        o.insert("channel".into(), json!(channel));
        o.insert("fns".into(), json!(fns));
        o.insert("span".into(), span(e.name.span.to(e.fns.span)));
        Value::Object(o)
    }

    fn migration_body(&mut self, f: usize, b: &ast::Body<ast::MigrationStmt>) -> Vec<Value> {
        self.entries(&b.items, &b.dangling, |w, n| {
                let mut o = Map::new();
                let (key, value, sp) = match &n.item {
                    ast::MigrationStmt::Execute(blk) => ("execute", json!(w.stmts(f, blk)), blk.span),
                    ast::MigrationStmt::Event { name, body } => ("event", json!({ "name": name.name, "body": w.migration_body(f, body) }), name.span.to(body.span)),
                    ast::MigrationStmt::Stmt(Stmt::Expr(e)) if matches!(&e.kind, ExprKind::Call { callee, .. } if matches!(callee.kind, ExprKind::Name(_))) => {
                        let ExprKind::Call { callee, args } = &e.kind else { unreachable!() };
                        let ExprKind::Name(nm) = &callee.kind else { unreachable!() };
                        let args = w.entries(&args.items, &args.dangling, |w, a| w.named(f, a.item.name.as_ref(), &a.item.value, None));
                        ("call", json!({ "name": nm.name, "args": args }), e.span)
                    }
                    ast::MigrationStmt::Stmt(s) => ("stmt", w.stmt(f, s), stmt_span(s)),
                };
                o.insert(key.into(), value);
                o.insert("span".into(), span(sp));
                Value::Object(o)
            })
    }

    // ---- statements

    fn stmts(&mut self, f: usize, b: &ast::Block) -> Vec<Value> {
        self.entries(&b.stmts, &b.dangling, |w, n| {
                let mut v = w.stmt(f, &n.item);
                // a statement ends at its `;`, where it has one
                if let Some(end) = v["span"][1].as_u64() {
                    let src = &w.c.files[f].src;
                    let rest = &src[end as usize..];
                    let gap = rest.len() - rest.trim_start_matches([' ', '\t']).len();
                    if rest[gap..].starts_with(';') {
                        v["span"][1] = json!(end as usize + gap + 1);
                    }
                }
                v
            })
    }

    fn stmt(&mut self, f: usize, s: &Stmt) -> Value {
        let sp = stmt_span(s);
        let (key, value) = match s {
            Stmt::Let(v) => ("let", self.var(f, v, false)),
            Stmt::Mut(v) => ("mut", self.var(f, v, true)),
            Stmt::Return(e) => ("return", match e {
                Some(e) => json!({ "value": self.expr(f, e) }),
                None => json!({}),
            }),
            Stmt::Invoke { name, args } => ("invoke", json!({ "event": name.name, "args": self.entries(&args.items, &args.dangling, |w, a| w.expr(f, &a.item.value)) })),
            Stmt::If(i) => {
                let branches: Vec<Value> = i
                    .branches
                    .iter()
                    .map(|(c, b)| {
                        let mut bo = Map::new();
                        match c {
                            ast::Cond::Expr(e) => {
                                bo.insert("cond".into(), self.expr(f, e));
                            }
                            ast::Cond::Open { mutable, name, ty, value } => {
                                let v = ast::Var { name: name.clone(), ty: ty.clone(), value: value.clone() };
                                bo.insert("opens".into(), self.var(f, &v, *mutable));
                            }
                        }
                        bo.insert("body".into(), json!(self.stmts(f, b)));
                        Value::Object(bo)
                    })
                    .collect();
                let mut io = Map::new();
                io.insert("branches".into(), json!(branches));
                if let Some(o) = &i.otherwise {
                    io.insert("else".into(), json!(self.stmts(f, o)));
                }
                ("if", Value::Object(io))
            }
            Stmt::For(fo) => {
                let mut o = Map::new();
                o.insert("name".into(), json!(fo.name.name));
                if let Some(k) = &fo.key {
                    o.insert("key".into(), json!(k.name));
                }
                if fo.mutable {
                    o.insert("mutable".into(), json!(true));
                }
                o.insert("over".into(), self.expr(f, &fo.over));
                o.insert("body".into(), json!(self.stmts(f, &fo.body)));
                ("for", Value::Object(o))
            }
            Stmt::Match(m) => {
                let cases = self.entries(&m.cases.items, &m.cases.dangling, |w, c| json!({ "patterns": c.item.patterns.iter().map(|p| w.pattern(f, p)).collect::<Vec<_>>(), "body": w.stmts(f, &c.item.body), "span": span(c.item.span) }));
                ("match", json!({ "subject": self.expr(f, &m.subject), "cases": cases }))
            }
            Stmt::Expr(Expr { kind: ExprKind::View(v), .. }) => ("use", self.use_(f, v)),
            Stmt::Expr(e) => ("expr", self.expr(f, e)),
            Stmt::Assign { place, value } => ("set", json!({ "place": self.expr(f, place), "value": self.expr(f, value) })),
        };
        json!({ key: value, "span": span(sp) })
    }

    fn pattern(&mut self, f: usize, p: &ast::Pattern) -> Value {
        match p {
            ast::Pattern::Case { ty, case, binds } => {
                let mut c = Map::new();
                c.insert("enum".into(), json!(self.type_full(f, &ty.name).unwrap_or_else(|| ty.name.clone())));
                c.insert("name".into(), json!(case.name));
                if let Some(b) = binds {
                    c.insert("binds".into(), json!(b.iter().map(|i| i.name.clone()).collect::<Vec<_>>()));
                }
                json!({ "case": c })
            }
            ast::Pattern::Number { negative, text, .. } => json!({ "num": if *negative { format!("-{}", text) } else { text.clone() } }),
            ast::Pattern::Text(t) => json!({ "text": text_of(t) }),
            ast::Pattern::Bool(b, _) => json!({ "bool": b }),
        }
    }

    // ---- uses

    fn use_(&mut self, f: usize, v: &ast::ViewCall) -> Value {
        let widget = match self.refs.get(&(f, v.name.span.start, v.name.span.end)) {
            Some(RefTarget::Decl(d)) => Some(*d),
            _ => None,
        };
        let full = widget.map(|d| self.c.defs[d].full.clone()).unwrap_or_else(|| v.name.name.clone());
        let mut args = match &v.args {
            Some(g) => self.entries(&g.items, &g.dangling, |w, a| w.named(f, a.item.name.as_ref(), &a.item.value, Some(&full))),
            None => Vec::new(),
        };
        let mut slots = Vec::new();
        let mut items = Vec::new();
        match &v.block {
            Some(ast::CallBlock::Children(b)) => {
                let slot = widget.and_then(|d| self.only_slot(d)).unwrap_or_default();
                slots.push(json!({ "slot": format!("{}.{}", full, slot), "view": self.stmts(f, b), "span": span(b.span) }));
            }
            Some(ast::CallBlock::Named(body)) => {
                items = self.members(&body.items, &body.dangling, |w, e, c| match &e.item {
                    ast::NamedEntry::Input { name, value } => args.push(with(w.named(f, Some(name), value, Some(&full)), c)),
                    ast::NamedEntry::Slot { name, block } => slots.push(with(json!({ "slot": format!("{}.{}", full, name.name), "view": w.stmts(f, block), "span": span(name.span.to(block.span)) }), c)),
                });
            }
            None => {}
        }
        let mut o = Map::new();
        o.insert("widget".into(), json!(full));
        o.insert("name_span".into(), span(v.name.span));
        o.insert("args".into(), json!(args));
        if !slots.is_empty() {
            o.insert("slots".into(), json!(slots));
        }
        o.insert("span".into(), span(v.span));
        put_items(&mut o, items);
        Value::Object(o)
    }

    fn only_slot(&self, d: usize) -> Option<String> {
        match &self.c.defs[d].kind {
            DefKind::Widget { slots, .. } if slots.len() == 1 => Some(slots[0].name.clone()),
            _ => None,
        }
    }

    /// A named argument: of a use (`owner` is the widget), a record's field, or a migration's.
    fn named(&mut self, f: usize, name: Option<&ast::Ident>, value: &Expr, owner: Option<&str>) -> Value {
        let mut o = Map::new();
        let nm = name.map(|i| i.name.clone()).unwrap_or_default();
        o.insert("name".into(), json!(nm));
        if let (Some(owner), Some(_)) = (owner, name) {
            o.insert("target".into(), json!(format!("{}.{}", owner, nm)));
        }
        if let Some(i) = name {
            o.insert("name_span".into(), span(i.span));
        }
        o.insert("value".into(), self.expr(f, value));
        let sp = name.map_or(value.span, |i| i.span.to(value.span));
        o.insert("span".into(), span(sp));
        Value::Object(o)
    }

    // ---- expressions

    fn expr(&mut self, f: usize, e: &Expr) -> Value {
        let free = self.free_before(e.span.start);
        let mut o = Map::new();
        let (key, value) = self.expr_kind(f, e);
        o.insert(key.into(), value);
        if let Some((t, _)) = self.typed.get(&(f, e.span.start, e.span.end)).cloned() {
            if !matches!(t, Ty::Err | Ty::Var(_)) {
                let t = self.c.vars.resolve(&t);
                o.insert("type".into(), json!(self.c.ir_ty(&t)));
            }
        }
        o.insert("span".into(), span(e.span));
        if !free.is_empty() {
            o.insert("comments".into(), json!({ "free": free }));
        }
        Value::Object(o)
    }

    fn expr_kind(&mut self, f: usize, e: &Expr) -> (&'static str, Value) {
        match &e.kind {
            ExprKind::Number(n) => ("num", json!(n)),
            ExprKind::Duration(d) => ("duration", json!(d)),
            ExprKind::Bool(b) => ("bool", json!(b)),
            ExprKind::Text(t) if !t.has_holes() => ("text", json!(text_of(t))),
            ExprKind::Text(t) => {
                let mut pieces = vec![String::new()];
                let mut holes = Vec::new();
                for p in &t.parts {
                    match p {
                        ast::TextPart::Piece { raw, .. } => pieces.last_mut().unwrap().push_str(&unescape(raw)),
                        ast::TextPart::Hole(h) => {
                            holes.push(self.expr(f, h));
                            pieces.push(String::new());
                        }
                    }
                }
                ("template", json!({ "pieces": pieces, "holes": holes }))
            }
            ExprKind::SelfValue => ("get", json!({ "self": true })),
            ExprKind::State => ("get", json!({ "state": "" })),
            ExprKind::Name(n) => ("get", self.get(f, e, n)),
            ExprKind::Paren(x) => self.expr_kind(f, x),
            ExprKind::List(g) => ("list", json!(self.entries(&g.items, &g.dangling, |w, x| w.expr(f, &x.item)))),
            ExprKind::Record { ty, inits } => {
                let of = ty.as_ref().and_then(|t| self.target_at(f, t.span));
                let fields = self.entries(&inits.items, &inits.dangling, |w, i| w.named(f, Some(&i.item.name), &i.item.value, of.as_deref()));
                let mut r = Map::new();
                if let Some(of) = of {
                    r.insert("of".into(), json!(of));
                }
                r.insert("fields".into(), json!(fields));
                ("record", Value::Object(r))
            }
            ExprKind::Field { target, name } => {
                if let ExprKind::State = target.kind {
                    return ("get", json!({ "state": name.name, "name_span": span(name.span) }));
                }
                if let ExprKind::Name(tn) = &target.kind {
                    if let Some(ch) = self.channel_full(f, &tn.name) {
                        if !self.typed.contains_key(&(f, target.span.start, target.span.end)) {
                            return ("get", json!({ "extern": format!("{}.{}", ch, name.name), "name_span": span(name.span) }));
                        }
                    }
                }
                let mut fo = Map::new();
                fo.insert("of".into(), self.expr(f, target));
                fo.insert("field".into(), json!(name.name));
                if let Some(RefTarget::Member(d, m)) = self.refs.get(&(f, name.span.start, name.span.end)).cloned() {
                    fo.insert("member".into(), json!(format!("{}.{}", self.c.defs[d].full, m)));
                }
                fo.insert("name_span".into(), span(name.span));
                ("field", Value::Object(fo))
            }
            ExprKind::Case { target, name, args } => {
                if let ExprKind::Name(tn) = &target.kind {
                    if tn.name == "opt" && name.name == "NONE" {
                        return ("none", json!({}));
                    }
                    if tn.name == "opt" && name.name == "SOME" {
                        if let Some(a) = args.as_ref().and_then(|g| g.items.first()) {
                            return ("some", self.expr(f, &a.item.value));
                        }
                    }
                }
                let of = self.target_at(f, target.span).unwrap_or_default();
                let mut co = Map::new();
                co.insert("of".into(), json!(of));
                co.insert("case".into(), json!(name.name));
                co.insert("case_span".into(), span(name.span));
                if let Some(g) = args {
                    co.insert("args".into(), json!(self.entries(&g.items, &g.dangling, |w, a| w.expr(f, &a.item.value))));
                }
                ("case", Value::Object(co))
            }
            ExprKind::Index { target, index } => ("index", json!({ "of": self.expr(f, target), "index": self.expr(f, index) })),
            ExprKind::Call { callee, args } => {
                // the callee before the arguments, as written, so free comments meet their expressions in order
                let callee_first = match &callee.kind {
                    ExprKind::Field { target, .. } => Some(self.expr(f, target)),
                    _ => None,
                };
                let call_args = self.entries(&args.items, &args.dangling, |w, a| w.expr(f, &a.item.value));
                match &callee.kind {
                    ExprKind::Name(n) => {
                        if let Some(RefTarget::Fn(fid)) = self.refs.get(&(f, n.span.start, n.span.end)).cloned() {
                            return ("call", json!({ "fn": self.c.fns[fid].full, "args": call_args, "name_span": span(n.span) }));
                        }
                    }
                    ExprKind::Field { target, name } => {
                        if let Some(RefTarget::Fn(fid)) = self.refs.get(&(f, name.span.start, name.span.end)).cloned() {
                            let mut co = Map::new();
                            co.insert("fn".into(), json!(self.c.fns[fid].full));
                            // a fn called on a value has it as self; one called on a type has none
                            if self.typed.contains_key(&(f, target.span.start, target.span.end)) {
                                co.insert("self".into(), callee_first.clone().unwrap_or_else(|| self.expr(f, target)));
                            }
                            co.insert("args".into(), json!(call_args));
                            co.insert("name_span".into(), span(name.span));
                            return ("call", Value::Object(co));
                        }
                    }
                    _ => {}
                }
                ("apply", json!({ "fn": self.expr(f, callee), "args": call_args }))
            }
            ExprKind::With { target, inits } => {
                let of = self.typed.get(&(f, target.span.start, target.span.end)).and_then(|(t, _)| if let Ty::Def(d, _) = t { Some(self.c.defs[*d].full.clone()) } else { None });
                let target_v = self.expr(f, target);
                let fields = self.entries(&inits.items, &inits.dangling, |w, i| w.named(f, Some(&i.item.name), &i.item.value, of.as_deref()));
                ("with", json!({ "of": target_v, "fields": fields }))
            }
            ExprKind::Unary { op, operand } => ("op", json!({ "op": match op { ast::UnOp::Not => "!", ast::UnOp::Neg => "-" }, "args": [self.expr(f, operand)] })),
            ExprKind::Binary { op, left, right } => ("op", json!({ "op": bin_symbol(*op), "args": [self.expr(f, left), self.expr(f, right)] })),
            ExprKind::Ternary { cond, then, otherwise } => ("pick", json!({ "cond": self.expr(f, cond), "then": self.expr(f, then), "else": self.expr(f, otherwise) })),
            ExprKind::Range { low, high, step } => {
                let mut r = Map::new();
                if let Some(l) = low {
                    r.insert("low".into(), self.expr(f, l));
                }
                if let Some(h) = high {
                    r.insert("high".into(), self.expr(f, h));
                }
                if let Some(s) = step {
                    r.insert("step".into(), self.expr(f, s));
                }
                ("range", Value::Object(r))
            }
            ExprKind::Try(x) => ("try", self.expr(f, x)),
            ExprKind::Lambda(l) => {
                let params = self.params(f, &l.params);
                let mut lo = Map::new();
                lo.insert("params".into(), json!(params));
                if let Some(ast::RetType::Type(t)) = &l.ret {
                    lo.insert("returns".into(), json!(self.ty_of_ast(f, t)));
                }
                lo.insert("body".into(), json!(self.stmts(f, &l.body)));
                ("lambda", Value::Object(lo))
            }
            ExprKind::View(v) => ("use", self.use_(f, v)),
        }
    }

    /// What a name is, from what it compiled to.
    fn get(&mut self, f: usize, e: &Expr, n: &ast::Ident) -> Value {
        let compiled = self.typed.get(&(f, e.span.start, e.span.end)).map(|(_, x)| x.clone());
        let mut o = Map::new();
        match compiled {
            Some(ir::Expr::Get(g)) => match g {
                ir::Get::Local(l) => {
                    o.insert("local".into(), json!(l));
                }
                ir::Get::Input(i) => {
                    o.insert("input".into(), json!(i));
                }
                ir::Get::Cell(c) => {
                    o.insert("own".into(), json!(c));
                }
                ir::Get::State(s) => {
                    o.insert("state".into(), json!(s));
                }
                ir::Get::Let(l) => {
                    o.insert("let".into(), json!(l));
                }
                ir::Get::Slot(s) => {
                    o.insert("slot".into(), json!(s));
                }
                ir::Get::Extern(x) => {
                    o.insert("extern".into(), json!(x));
                }
                _ => {
                    o.insert("local".into(), json!(n.name));
                }
            },
            _ => {
                o.insert("local".into(), json!(n.name));
            }
        }
        o.insert("name_span".into(), span(n.span));
        Value::Object(o)
    }

    // ---- prose

    fn prose(&mut self, f: usize, p: &ast::Prose) -> Vec<Value> {
        let src = self.c.files[f].src.clone();
        let mut out = Vec::new();
        let mut run: Vec<&ast::TextLine> = Vec::new();
        let flush = |w: &mut Self, run: &mut Vec<&ast::TextLine>, out: &mut Vec<Value>| {
            // blank lines around a run are the layout's, not the run's
            let blank = |l: &&ast::TextLine| l.holes.is_empty() && src[l.span.range()].trim().is_empty();
            while run.last().map_or(false, blank) {
                run.pop();
            }
            let lead = run.iter().take_while(|l| blank(l)).count();
            run.drain(..lead);
            if run.is_empty() {
                return;
            }
            let mut text = String::new();
            let mut holes = Vec::new();
            for (k, line) in run.iter().enumerate() {
                if k > 0 {
                    text.push('\n');
                }
                let mut at = line.span.start as usize;
                for h in &line.holes {
                    text.push_str(&src[at..h.span.start as usize]);
                    text.push('\u{fffc}');
                    holes.push(w.expr(f, &h.expr));
                    at = h.span.end as usize;
                }
                text.push_str(&src[at..line.span.end as usize]);
            }
            let sp = run[0].span.to(run[run.len() - 1].span);
            out.push(json!({ "markdown": { "markdown": text, "holes": holes }, "span": span(sp) }));
            run.clear();
        };
        for n in &p.nodes {
            match n {
                ast::ProseNode::Text(l) => run.push(l),
                ast::ProseNode::Fence(ls) | ast::ProseNode::Maths(ls) => run.extend(ls.iter()),
                other => {
                    flush(self, &mut run, &mut out);
                    out.push(self.prose_node(f, other));
                }
            }
        }
        flush(self, &mut run, &mut out);
        out
    }

    fn prose_node(&mut self, f: usize, n: &ast::ProseNode) -> Value {
        match n {
            ast::ProseNode::Call(c) => {
                let mut u = self.use_(f, &c.call);
                if let Some((body, _)) = &c.block {
                    let widget = match self.refs.get(&(f, c.call.name.span.start, c.call.name.span.end)) {
                        Some(RefTarget::Decl(d)) => Some(*d),
                        _ => None,
                    };
                    let full = u["widget"].as_str().unwrap_or("").to_string();
                    // a named slot's prose is its own; the rest is the one slot's
                    let mut slots: Vec<Value> = Vec::new();
                    let mut rest = ast::Prose { nodes: Vec::new() };
                    for node in &body.nodes {
                        if let ast::ProseNode::Slot { name, body: sb, span: sp, .. } = node {
                            slots.push(json!({ "slot": format!("{}.{}", full, name.name), "prose": self.prose(f, sb), "span": span(*sp) }));
                        } else {
                            rest.nodes.push(node.clone());
                        }
                    }
                    if !rest.nodes.is_empty() {
                        let slot = widget.and_then(|d| self.only_slot(d)).unwrap_or_default();
                        slots.insert(0, json!({ "slot": format!("{}.{}", full, slot), "prose": self.prose(f, &rest), "span": span(c.span) }));
                    }
                    if !slots.is_empty() {
                        u["slots"] = json!(slots);
                    }
                }
                let mut b = json!({ "call": u, "span": span(c.span) });
                // the brackets of a call in prose, which is no node of a list
                let mut shape = Shape::default();
                walk(&serde_json::to_value(&c.call).unwrap_or_default(), &mut shape, false);
                let mut co = Map::new();
                shape.put(&mut co);
                if !co.is_empty() {
                    b["comments"] = Value::Object(co);
                }
                b
            }
            ast::ProseNode::If { branches, otherwise, span: sp, .. } => {
                let bs: Vec<Value> = branches
                    .iter()
                    .map(|(c, body, _)| {
                        let mut bo = Map::new();
                        match c {
                            ast::Cond::Expr(e) => {
                                bo.insert("cond".into(), self.expr(f, e));
                            }
                            ast::Cond::Open { mutable, name, ty, value } => {
                                let v = ast::Var { name: name.clone(), ty: ty.clone(), value: value.clone() };
                                bo.insert("opens".into(), self.var(f, &v, *mutable));
                            }
                        }
                        bo.insert("prose".into(), json!(self.prose(f, body)));
                        Value::Object(bo)
                    })
                    .collect();
                let mut io = Map::new();
                io.insert("branches".into(), json!(bs));
                if let Some((o, _)) = otherwise {
                    io.insert("else".into(), json!(self.prose(f, o)));
                }
                json!({ "if": io, "span": span(*sp) })
            }
            ast::ProseNode::For { mutable, key, name, over, body, span: sp, .. } => {
                let mut o = Map::new();
                o.insert("name".into(), json!(name.name));
                if let Some(k) = key {
                    o.insert("key".into(), json!(k.name));
                }
                if *mutable {
                    o.insert("mutable".into(), json!(true));
                }
                o.insert("over".into(), self.expr(f, over));
                o.insert("prose".into(), json!(self.prose(f, body)));
                json!({ "for": o, "span": span(*sp) })
            }
            ast::ProseNode::Match { subject, cases, span: sp, .. } => {
                let cs: Vec<Value> = cases.iter().map(|c| json!({ "patterns": c.patterns.iter().map(|p| self.pattern(f, p)).collect::<Vec<_>>(), "prose": self.prose(f, &c.body), "span": span(c.span) })).collect();
                json!({ "match": { "subject": self.expr(f, subject), "cases": cs }, "span": span(*sp) })
            }
            ast::ProseNode::Slot { name, body, span: sp, .. } => json!({ "slot": { "slot": name.name, "prose": self.prose(f, body) }, "span": span(*sp) }),
            ast::ProseNode::Comment { span: sp, .. } => {
                let text = self.c.files[f].src[sp.range()].trim().to_string();
                json!({ "comment": text, "span": span(*sp) })
            }
            ast::ProseNode::Text(_) | ast::ProseNode::Fence(_) | ast::ProseNode::Maths(_) => Value::Null,
        }
    }

    // ---- names

    fn target_at(&self, f: usize, at: Span) -> Option<String> {
        match self.refs.get(&(f, at.start, at.end))? {
            RefTarget::Decl(d) => Some(self.c.defs[*d].full.clone()),
            RefTarget::Member(d, m) | RefTarget::Case(d, m) => Some(format!("{}.{}", self.c.defs[*d].full, m)),
            RefTarget::Fn(x) => Some(self.c.fns[*x].full.clone()),
        }
    }

    fn def_at(&self, f: usize, at: Span) -> Option<usize> {
        self.c.defs.iter().position(|d| d.file == f && d.span == at)
    }

    fn fn_id(&self, f: usize, s: &ast::FnSig) -> Option<String> {
        self.c.fns.iter().find(|x| x.file == f && x.span == s.name.span).map(|x| x.full.clone())
    }

    fn type_full(&self, f: usize, name: &str) -> Option<String> {
        match self.c.lookup_type(f, name)? {
            TypeSym::Def(d) => Some(self.c.defs[d].full.clone()),
            TypeSym::Builtin(b) => Some(b.to_string()),
        }
    }

    fn channel_full(&self, f: usize, name: &str) -> Option<String> {
        match self.c.lookup_value(f, name)? {
            crate::check::ValueSym::Channel(c) => Some(self.c.channels[c].full.clone()),
            _ => None,
        }
    }

    fn ty_of_ast(&mut self, f: usize, t: &ast::Type) -> ir::Type {
        // the checker's own reading of a type written in this file
        let before = self.c.diags.len();
        let generics: Vec<String> = Vec::new();
        let x = self.c.ir_type(f, t, &generics);
        self.c.diags.truncate(before);
        x
    }
}

// ---- comments

/// A node's comments: `doc` holds its `//@` lines and `leading` the others on the lines just
/// above it; `trailing` the one that ends its line after its `;` or `,`; `block_start` and
/// `block_end` those after an opening and a closing bracket that end their lines, each with `at`,
/// which of the node's own bracket pairs, counted from 0 in the order they are written; `free`
/// those inside it after its last expression. Also returned: the comments above it with a blank
/// line after them, as comment items, and its other free comments, for the expressions after them.
fn note<T: Serialize>(n: &ast::Node<T>) -> (Option<Value>, Vec<Value>, Vec<Comment>) {
    let mut groups: Vec<Vec<&Comment>> = vec![Vec::new()];
    for l in &n.leading {
        match l {
            ast::Leading::Blank => {
                if !groups.last().unwrap().is_empty() {
                    groups.push(Vec::new());
                }
            }
            ast::Leading::Comment(c) => groups.last_mut().unwrap().push(c),
        }
    }
    let own = if matches!(n.leading.last(), Some(ast::Leading::Blank)) { Vec::new() } else { groups.pop().unwrap_or_default() };
    let items: Vec<Value> = groups.iter().filter(|g| !g.is_empty()).map(|g| comment_item(g)).collect();
    let mut o = Map::new();
    let doc = doc_of(&own);
    if !doc.is_empty() {
        o.insert("doc".into(), Value::Object(doc));
    }
    let mut shape = Shape::default();
    walk(&serde_json::to_value(&n.item).unwrap_or_default(), &mut shape, false);
    let mut leading: Vec<String> = own.iter().filter(|c| !c.is_doc()).map(|c| c.text.clone()).collect();
    leading.extend(shape.typed.iter().cloned());
    if !leading.is_empty() {
        o.insert("leading".into(), json!(leading));
    }
    if let Some(c) = n.trailing.first() {
        o.insert("trailing".into(), json!(c.text));
    }
    shape.put(&mut o);
    let (inside, after): (Vec<Comment>, Vec<Comment>) = n.free.iter().cloned().partition(|c| shape.last_expr.map_or(false, |e| c.span.start < e));
    if !after.is_empty() {
        o.insert("free".into(), json!(after.iter().map(|c| c.text.clone()).collect::<Vec<_>>()));
    }
    ((!o.is_empty()).then(|| Value::Object(o)), items, inside)
}

/// What a node's parse tree holds outside the nodes in it: its bracket pairs, where its last
/// expression starts, and the comments inside a type it writes, which rir keeps above the node.
#[derive(Default)]
struct Shape {
    /// Each pair: where it opens, the comments after its opening bracket and after its closing one.
    brackets: Vec<(u64, Vec<String>, Vec<String>)>,
    last_expr: Option<u32>,
    typed: Vec<String>,
}

impl Shape {
    fn put(&mut self, o: &mut Map<String, Value>) {
        self.brackets.sort_by_key(|b| b.0);
        let mut starts = Vec::new();
        let mut ends = Vec::new();
        for (at, (_, open, close)) in self.brackets.iter().enumerate() {
            starts.extend(open.iter().map(|t| json!({ "at": at, "text": t })));
            ends.extend(close.iter().map(|t| json!({ "at": at, "text": t })));
        }
        if !starts.is_empty() {
            o.insert("block_start".into(), json!(starts));
        }
        if !ends.is_empty() {
            o.insert("block_end".into(), json!(ends));
        }
    }
}

/// Walks a parse tree as serde writes it: a node is `{leading, item, trailing, newline, free}`, a
/// bracketed list or block has `open`, `dangling` and `close`, a type has `base` and `contract`,
/// and an expression is `{kind, span}`.
fn walk(v: &Value, s: &mut Shape, in_type: bool) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| walk(x, s, in_type)),
        Value::Object(o) => {
            let node = ["leading", "item", "trailing", "newline"].iter().all(|k| o.contains_key(*k));
            let list = ["open", "dangling", "close", "span"].iter().all(|k| o.contains_key(*k));
            if in_type && (node || list) {
                every_comment(v, &mut s.typed);
                return;
            }
            if node {
                return;
            }
            if list {
                let texts = |k: &str| o[k].as_array().into_iter().flatten().filter_map(|c| c["text"].as_str().map(str::to_string)).collect::<Vec<_>>();
                s.brackets.push((o["span"]["start"].as_u64().unwrap_or(0), texts("open"), texts("close")));
                for (k, x) in o {
                    if k != "open" && k != "close" && k != "dangling" {
                        walk(x, s, in_type);
                    }
                }
                return;
            }
            if !in_type && o.len() == 2 && o.contains_key("kind") && o.contains_key("span") {
                if let Some(at) = o["span"]["start"].as_u64() {
                    s.last_expr = Some(s.last_expr.map_or(at as u32, |e| e.max(at as u32)));
                }
            }
            let ty = o.contains_key("base") && o.contains_key("contract");
            for x in o.values() {
                walk(x, s, in_type || ty);
            }
        }
        _ => {}
    }
}

/// Every comment in a part of a parse tree, in the order serde writes it.
fn every_comment(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| every_comment(x, out)),
        Value::Object(o) => {
            if o.contains_key("text") && o.contains_key("gap") && o.contains_key("span") {
                if let Some(t) = o["text"].as_str() {
                    out.push(t.to_string());
                }
                return;
            }
            o.values().for_each(|x| every_comment(x, out));
        }
        _ => {}
    }
}

/// Comments on lines of their own that belong to no node, as written: `{"comment": [...], "span"}`.
fn comment_item(group: &[&Comment]) -> Value {
    json!({ "comment": group.iter().map(|c| c.text.clone()).collect::<Vec<_>>(), "span": [group[0].span.start, group[group.len() - 1].span.end] })
}

/// The comments before a list's closing bracket, each run of them a comment item.
fn free_items(end: &[ast::Leading]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut run: Vec<&Comment> = Vec::new();
    for l in end {
        match l {
            ast::Leading::Comment(c) => run.push(c),
            ast::Leading::Blank => {
                if !run.is_empty() {
                    out.push(comment_item(&run));
                    run.clear();
                }
            }
        }
    }
    if !run.is_empty() {
        out.push(comment_item(&run));
    }
    out
}

/// A value with its node's comments: on a declaration, inside its one key; on anything else, beside
/// its span.
fn with(mut v: Value, c: Option<Value>) -> Value {
    let Some(c) = c else { return v };
    if let Value::Object(o) = &mut v {
        if !o.contains_key("span") && o.len() == 1 {
            if let Some(Value::Object(inner)) = o.values_mut().next() {
                inner.insert("comments".into(), c);
                return v;
            }
        }
        o.insert("comments".into(), c);
    }
    v
}

fn put_items(o: &mut Map<String, Value>, items: Vec<Value>) {
    if !items.is_empty() {
        o.insert("comment_items".into(), json!(items));
    }
}

/// The comments of a member written into its owner rather than as an entry: `view`, `markdown`,
/// `tokens`, `uses`, a pack's lines.
fn part(parts: &mut Vec<Value>, name: &str, c: Option<Value>) {
    if let Some(c) = c {
        parts.push(json!({ "part": name, "comments": c }));
    }
}

fn put_parts(o: &mut Map<String, Value>, parts: Vec<Value>) {
    if !parts.is_empty() {
        o.insert("part_comments".into(), json!(parts));
    }
}

/// Front matter without its `---` lines, as the IR has it.
fn front_matter_text(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    lines.iter().skip(1).take_while(|l| l.trim_end() != "---" && l.trim_end() != "...").cloned().collect::<Vec<_>>().join("\n")
}

// ---- small helpers

fn span(s: Span) -> Value {
    json!([s.start, s.end])
}

/// `//@about`, `//@param name …` and `//@deprecated` above a node. One written over several
/// lines, each starting the same way, is its lines joined by line breaks.
fn doc_of(comments: &[&Comment]) -> Map<String, Value> {
    let mut o = Map::new();
    let mut params = Map::new();
    let join = |m: &mut Map<String, Value>, k: &str, t: &str| {
        let t = t.trim();
        let v = match m.get(k).and_then(Value::as_str) {
            Some(had) => format!("{}\n{}", had, t),
            None => t.to_string(),
        };
        m.insert(k.to_string(), json!(v));
    };
    for c in comments {
        if let Some(t) = c.text.strip_prefix("//@about") {
            join(&mut o, "about", t);
        } else if let Some(t) = c.text.strip_prefix("//@deprecated") {
            join(&mut o, "deprecated", t);
        } else if let Some(t) = c.text.strip_prefix("//@param") {
            let t = t.trim();
            let (name, rest) = t.split_once(' ').unwrap_or((t, ""));
            join(&mut params, name, rest);
        }
    }
    if !params.is_empty() {
        o.insert("params".into(), Value::Object(params));
    }
    o
}

fn text_of(t: &ast::TextLit) -> String {
    t.parts.iter().map(|p| if let ast::TextPart::Piece { raw, .. } = p { unescape(raw) } else { String::new() }).collect()
}

/// A text's characters: its escapes read.
fn unescape(raw: &str) -> String {
    let mut out = String::new();
    let mut cs = raw.chars().peekable();
    while let Some(c) = cs.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match cs.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('u') if cs.peek() == Some(&'{') => {
                cs.next();
                let hex: String = cs.by_ref().take_while(|c| *c != '}').collect();
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn bin_symbol(op: ast::BinOp) -> &'static str {
    use ast::BinOp as B;
    match op {
        B::Or => "||",
        B::And => "&&",
        B::Eq => "==",
        B::Ne => "!=",
        B::Lt => "<",
        B::Le => "<=",
        B::Gt => ">",
        B::Ge => ">=",
        B::In => "in",
        B::Add => "+",
        B::Sub => "-",
        B::Mul => "*",
        B::Div => "/",
        B::Rem => "%",
    }
}

fn stmt_span(s: &Stmt) -> Span {
    match s {
        Stmt::Let(v) | Stmt::Mut(v) => v.name.span.to(v.value.span),
        Stmt::Return(Some(e)) | Stmt::Expr(e) => e.span,
        Stmt::Return(None) => Span::default(),
        Stmt::Invoke { name, args } => name.span.to(args.span),
        Stmt::If(i) => {
            let first = i.branches.first().map(|(_, b)| b.span).unwrap_or_default();
            let last = i.otherwise.as_ref().map(|b| b.span).or_else(|| i.branches.last().map(|(_, b)| b.span)).unwrap_or_default();
            first.to(last)
        }
        Stmt::For(f) => f.name.span.to(f.body.span),
        Stmt::Match(m) => m.span.to(m.cases.span),
        Stmt::Assign { place, value } => place.span.to(value.span),
    }
}

/// SHA-256, for the hash that names a file's source.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    h.iter().map(|x| format!("{:08x}", x)).collect()
}

/// A token as a `tokens` line writes it.
fn token_text(t: &ast::TokenRef) -> String {
    t.case.as_ref().map_or(t.token.name.clone(), |c| format!("{}.{}", t.token.name, c.name))
}
