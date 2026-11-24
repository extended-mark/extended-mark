//! The bodies of every declaration, then the constants worked out by the evaluator.

use crate::body::{Cx, Local, LocalKind, LITERAL_MARK};
use crate::ty::Ty;
use crate::*;
use xtmd_eval::{interp::Constants, FailKind, Machine, Program};

/// What stands in for core/entities.xtmdpack: its table is [`entities`].
pub(crate) const NO_ENTITIES: &str = "package core;\n\nlet entities: map<text> = map { entries = [] };\n";

/// core's `entities`, as checking its file gives it.
fn entities() -> xtmd_eval::Value {
    use xtmd_eval::Value;
    let record = |ty: &str, fields: Vec<(&str, Value)>| Value::Record(Rc::new(xtmd_eval::value::Record { ty: Rc::from(ty), fields: fields.into_iter().map(|(n, v)| (Rc::from(n), v)).collect() }));
    let entries = xtmd_syntax::markdown::ENTITIES.iter().map(|(k, v)| record("record", vec![("key", Value::text(k)), ("value", Value::text(v))])).collect();
    record("core/map", vec![("entries", Value::list(entries, &mut 0))])
}

impl Checker {
    pub(crate) fn bodies(&mut self) {
        // fns, written in Extended Mark or extern with a default
        let fns_end = self.fns.len();
        for f in self.from.fns..fns_end {
            let s = self.fns[f].clone();
            if s.widget.is_some() {
                continue;
            }
            let Some(body) = &s.body else { continue };
            let mut cx = Cx::new(s.file);
            cx.generics = s.generics.iter().map(|g| g.name.clone()).collect();
            cx.bounds = s.generics.clone();
            cx.ret = Some(s.ret.clone());
            cx.ret_ir = s.ret_ir.clone();
            cx.action = s.ret.is_none();
            cx.fn_id = Some(f);
            // a document's own fns read and (as actions) write its $state
            if self.state.as_ref().map_or(false, |(file, _)| *file == s.file) {
                cx.state = self.state_fields();
            }
            cx.fn_depth = cx.scopes.len();
            if let Some(sig) = &s.sig {
                self.check_value_fn_params(s.file, sig);
            }
            cx.push();
            for p in &s.params {
                cx.scopes.last_mut().unwrap().push(Local { name: p.name.clone(), ty: p.ty.clone(), mutable: p.mutable, kind: LocalKind::Plain, known: Some(crate::proof::Known::of(&p.ir)), own_origin: false });
            }
            let b = self.block(&mut cx, body);
            self.out.fn_bodies.insert(f, b);
        }
        // records: defaults and rules; enums have neither
        for d in self.from.defs..self.defs.len() {
            let file = self.defs[d].file;
            let generics: Vec<String> = self.defs[d].generics.iter().map(|g| g.name.clone()).collect();
            match self.defs[d].kind.clone() {
                DefKind::Record { fields } => {
                    for fl in &fields {
                        if let Some(def) = &fl.default {
                            let mut cx = Cx::new(file);
                            cx.generics = generics.clone();
                            cx.bounds = self.defs[d].generics.clone();
                            let (x, _) = self.expr(&mut cx, def, Some(&fl.ty));
                            self.want(&cx, def, &x, &fl.ir, format!("`{}`'s default", fl.name));
                            self.out.defaults.insert((d, fl.name.clone()), x);
                        }
                    }
                    let ast::DeclKind::Record(r) = self.decl_of(d) else { continue };
                    let mut rules = Vec::new();
                    for m in r.members.iter() {
                        if let ast::Member::Require(req) = m {
                            let mut cx = Cx::new(file);
                            cx.generics = generics.clone();
                            cx.bounds = self.defs[d].generics.clone();
                            cx.ret = Some(Some(Ty::Bool));
                            cx.push();
                            for fl in &fields {
                                cx.scopes.last_mut().unwrap().push(Local { name: fl.name.clone(), ty: fl.ty.clone(), mutable: false, kind: LocalKind::Plain, known: Some(crate::proof::Known::of(&fl.ir)), own_origin: false });
                            }
                            let b = self.block(&mut cx, &req.body);
                            rules.push(ir::Require { warn: req.warn, message: text_of(&req.message), body: b });
                        }
                    }
                    self.out.requires.insert(d, rules);
                }
                DefKind::Widget { .. } => self.widget(d),
                _ => {}
            }
        }
        // tokens blocks
        for (d, case, file, value) in self.token_values.iter().skip(self.from.tokens).cloned().collect::<Vec<_>>() {
            let DefKind::Token { value: vt, cases, .. } = self.defs[d].kind.clone() else { continue };
            let mut cx = Cx::new(file);
            let carrying = cases.iter().find(|c| c.name == case).map_or(false, |c| !c.fields.is_empty());
            if carrying {
                self.error(file, codes::MISMATCH, value.span, format!("`{}.{}` carries its value, so it needs no line in a tokens block", self.defs[d].name, case));
                continue;
            }
            let (x, _) = self.expr(&mut cx, &value, Some(&vt));
            self.out.token_values.insert((d, case), x);
        }
        // extern channels: their values' defaults
        for c in self.from.channels..self.channels.len() {
            let file = self.channels[c].file;
            for v in self.channels[c].values.clone() {
                if let Some(def) = &v.default {
                    let mut cx = Cx::new(file);
                    let (x, _) = self.expr(&mut cx, def, Some(&v.ty));
                    self.out.channel_defaults.insert((c, v.name.clone()), x);
                }
            }
            let refs = self.channels[c].token_refs.clone();
            self.channels[c].tokens = self.extern_tokens(file, &refs);
        }
        // lets
        for l in self.from.lets..self.lets.len() {
            let s = self.lets[l].clone();
            let mut cx = Cx::new(s.file);
            cx.state = self.state_fields();
            let before = self.diags.len();
            let (x, _) = self.expr(&mut cx, &s.value, Some(&s.ty));
            self.want(&cx, &s.value, &x, &s.ir, format!("`{}`", s.name));
            // a let already reported isn't worked out again, to fail a second time
            if self.diags[before..].iter().any(|d| d.diagnostic.is_error()) {
                self.failed_lets.insert(l);
            }
            self.out.let_values.insert(l, x);
        }
        self.state_and_expose();
        self.document();
        self.settle_effects();
        // types written in bodies
        self.check_type_rules();
        self.constants();
    }

    /// The document's prose: its own template, `doc`.
    fn document(&mut self) {
        self.next_site = 0;
        self.template_now = "doc".into();
        let Some(file) = (0..self.files.len()).find(|f| self.files[*f].ast.prose.is_some()) else { return };
        let ast = self.files[file].ast.clone();
        let mut cx = Cx::new(file);
        cx.state = self.state_fields();
        let nodes = self.prose_nodes(&mut cx, ast.prose.as_ref().unwrap());
        self.out.doc = Some(ir::Template {
            name: "doc".into(),
            generics: Vec::new(),
            inputs: Vec::new(),
            slots: Vec::new(),
            events: Vec::new(),
            cells: Vec::new(),
            requires: Vec::new(),
            fns: Vec::new(),
            view: nodes,
            markdown: None,
            levels: Vec::new(),
        });
    }

    pub(crate) fn state_fields(&mut self) -> Option<Vec<(String, Ty)>> {
        let (file, s) = self.state.clone()?;
        let mut out = Vec::new();
        for m in s.members.iter() {
            if let ast::Member::Field(f) = m {
                out.push((f.name.name.clone(), self.resolve(file, &f.ty, &[])));
            }
        }
        Some(out)
    }

    fn state_and_expose(&mut self) {
        let fields = self.state_fields();
        if let Some((file, s)) = self.state.clone() {
            let mut irf = Vec::new();
            let mut rules = Vec::new();
            for m in s.members.iter() {
                match m {
                    ast::Member::Field(f) => {
                        let t = self.resolve(file, &f.ty, &[]);
                        let ti = self.ir_type(file, &f.ty, &[]);
                        let mut cx = Cx::new(file);
                        let d = f.default.as_ref().map(|e| {
                            let x = self.expr(&mut cx, e, Some(&t)).0;
                            self.want(&cx, e, &x, &ti, format!("`$state.{}`", f.name.name));
                            x
                        });
                        if self.holds_calls(&t, 0) {
                            self.error(file, codes::VIEW_IN_MUT, f.ty.span, "`$state` holds data, and a view or a rich_text is a list of calls: keep the text, and make the calls from it where they are placed");
                        }
                        irf.push(ir::Field { name: f.name.name.clone(), ty: ti, mutable: false, default: d });
                    }
                    ast::Member::Require(r) => {
                        let mut cx = Cx::new(file);
                        cx.state = fields.clone();
                        cx.state_rules = true;
                        cx.ret = Some(Some(Ty::Bool));
                        let b = self.block(&mut cx, &r.body);
                        rules.push(ir::Require { warn: r.warn, message: text_of(&r.message), body: b });
                    }
                }
            }
            self.out.state = Some(ir::State { fields: irf, requires: rules });
        }
        for (file, e) in self.exposes.clone() {
            let channel = match self.lookup_value(file, &e.name.name) {
                Some(ValueSym::Channel(c)) => self.channels[c].full.clone(),
                _ => format!("{}/{}", self.files[file].pack, e.name.name),
            };
            let mut fns = Vec::new();
            for f in e.fns.iter() {
                // the host calls it with values of its own and reads nothing back (page 6)
                if let Some(ast::RetType::Type(t)) = &f.sig.ret {
                    self.error(file, codes::RETURN, t.span, format!("`{}.{}` is called by the host, which reads nothing back: an exposed fn returns nothing, so leave out its return type", e.name.name, f.sig.name.name));
                }
                for p in f.sig.params.iter().filter(|p| p.mutable) {
                    self.error(file, codes::EXPOSED_MUT, p.name.span, format!("`{}` can't be `mut`: the host gives `{}.{}` its values, and an exposed fn takes them as constants", p.name.name, e.name.name, f.sig.name.name));
                }
                let mut cx = Cx::new(file);
                cx.state = fields.clone();
                // a return type written is reported above, once: its `return`s aren't again
                cx.ret = Some(if matches!(f.sig.ret, Some(ast::RetType::Type(_))) { Some(Ty::Err) } else { None });
                // the host calls an exposed fn: an action
                cx.action = true;
                cx.exposed = true;
                cx.fn_depth = cx.scopes.len();
                cx.push();
                let ps = self.params(file, &f.sig.params, &[], &Ty::Err, &ir::Type::named("T"));
                // a `mut` one, reported above, is taken as written, so writing it isn't a second error
                for p in &ps {
                    cx.scopes.last_mut().unwrap().push(Local { name: p.name.clone(), ty: p.ty.clone(), mutable: p.mutable, kind: LocalKind::Plain, known: Some(crate::proof::Known::of(&p.ir)), own_origin: false });
                }
                let body = f.body.as_ref().map(|b| self.block(&mut cx, b)).unwrap_or_default();
                fns.push(ir::ExposeFns { name: f.sig.name.name.clone(), params: ps.iter().map(|p| ir::Param { name: p.name.clone(), ty: p.ir.clone(), mutable: false }).collect(), body });
            }
            self.out.exposes.push(ir::Expose { channel, fns });
        }
    }

    /// The program so far, for the evaluator: core's, made once, and what this compile declares.
    pub fn program(&self) -> Program {
        let mut p = self.base.as_deref().cloned().unwrap_or_default();
        for d in self.decls_changed(&self.out).chain(self.decls_from(&self.out, self.from)) {
            p.add(&d);
        }
        for (k, v) in &self.constants {
            if !self.base.as_ref().map_or(false, |b| b.lets.contains_key(k)) {
                p.lets.insert(k.clone(), v.clone());
            }
        }
        p
    }

    /// Literals become values through their type's `from`, and constants are worked out.
    fn constants(&mut self) {
        let prog = self.program();
        // literals: `"2026-11-20"` where a date is wanted is date.from("2026-11-20"), now
        let mut errors: Vec<(usize, Span, String)> = Vec::new();
        // records' `warn` rules that constants break
        let mut warned: Vec<(usize, Span, String)> = Vec::new();
        let pending = std::mem::take(&mut self.pending);
        {
            let mut patch = |e: &mut ir::Expr| {
                let ir::Expr::Call(c) = e else { return };
                let Some(id) = c.name.strip_prefix(LITERAL_MARK).and_then(|s| s.strip_prefix('#')).and_then(|s| s.parse::<usize>().ok()) else { return };
                let inner = c.args[0].clone();
                let mut m = Machine::new(&prog, Box::new(Constants));
                match m.eval(&inner) {
                    Ok(xtmd_eval::Value::Opt(Some(v))) => {
                        let p = &pending[id];
                        warned.extend(m.warned.iter().map(|w| (p.file, p.span, w.clone())));
                        *e = v.to_ir()
                    }
                    Ok(_) => {
                        let p = &pending[id];
                        let why = m.last_rule.clone().map(|r| format!(": {}", r)).unwrap_or_default();
                        errors.push((p.file, p.span, format!("this literal is not a {}{}", p.ty, why)));
                        *e = inner;
                    }
                    Err(f) if f.kind == FailKind::NotConstant => *e = inner,
                    Err(f) => {
                        let p = &pending[id];
                        errors.push((p.file, p.span, format!("working out this literal failed: {}", f.message)));
                        *e = inner;
                    }
                }
            };
            walk_out(&mut self.out, self.from.defs, &mut patch);
            for h in &mut self.handlers {
                for s in &mut h.body {
                    walk_stmt(s, &mut patch);
                }
            }
            for p in &mut self.proofs {
                if let Some(v) = &mut p.value {
                    walk_expr(v, &mut patch);
                }
            }
        }
        // a literal in a proof's value is the same literal again
        errors.sort_by(|a, b| (a.0, a.1.start, &a.2).cmp(&(b.0, b.1.start, &b.2)));
        errors.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1 && a.2 == b.2);
        for (file, span, msg) in errors {
            self.error(file, codes::LITERAL, span, msg);
        }
        // prose: each paragraph read by core's rich_text.read, each maths block by latex.from
        let links = links_value(&self.prose_links);
        let mut errors: Vec<(usize, Span, String)> = Vec::new();
        let mut literal_errors: Vec<(usize, Span, String)> = Vec::new();
        let mut blocks_in: Vec<(usize, Span, String)> = Vec::new();
        {
            let mut patch = |e: &mut ir::Expr| {
                let ir::Expr::Call(c) = e else { return };
                let Some(id) = c.name.strip_prefix(crate::view::PROSE_MARK).and_then(|s| s.strip_prefix('#')).and_then(|s| s.parse::<usize>().ok()) else { return };
                let ir::Expr::Template(t) = c.args[0].clone() else { return };
                let holes = t.holes.clone();
                let placeholders: Vec<xtmd_eval::Value> = (0..holes.len()).map(|k| placeholder_hole(k)).collect();
                let mut m = Machine::new(&prog, Box::new(Constants));
                m.md_native = true;
                let tpl = match template_value(&mut m, &t.pieces, placeholders) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                let p = &pending[id];
                // in code, a literal is its type's `from`; a document's prose also sees its link definitions
                let literal = p.ty.strip_prefix("literal:");
                let rich = t.ty.name == "core/rich_text";
                let out = if rich && literal.is_none() {
                    m.call("core/md_inline", vec![xtmd_eval::Binding::Value(tpl), xtmd_eval::Binding::Value(links.clone())])
                } else if rich {
                    m.call("core/md_read", vec![xtmd_eval::Binding::Value(tpl)])
                } else {
                    m.call(&format!("{}.from", t.ty.name), vec![xtmd_eval::Binding::Value(tpl)])
                };
                match out {
                    Ok(Some(xtmd_eval::Value::Opt(Some(v)))) => {
                        let mut lower = crate::rich::Rich::new(&holes, &p.holes);
                        *e = match (rich, literal) {
                            (true, None) => lower.sentence(Some(&v)),
                            (true, Some(_)) => lower.literal(&v),
                            _ => put_holes(v.to_ir(), &holes),
                        };
                        if let (ir::Expr::Rich(r), Some(of)) = (&*e, &p.sentence_for) {
                            if r.blocks {
                                blocks_in.push((p.file, p.span, of.clone()));
                            }
                        }
                    }
                    Ok(_) => {
                        let why = m.last_rule.clone().map(|r| format!(": {}", r)).unwrap_or_default();
                        match literal {
                            Some(ty) => literal_errors.push((p.file, p.span, format!("this literal is not a {}{}", ty, why))),
                            None => errors.push((p.file, p.span, format!("this {} is refused{}", if p.ty == "latex" { "maths" } else { "text" }, why))),
                        }
                    }
                    Err(f) => errors.push((p.file, p.span, format!("reading this Markdown failed: {}", f.message))),
                }
            };
            walk_out(&mut self.out, self.from.defs, &mut patch);
            for h in &mut self.handlers {
                for s in &mut h.body {
                    walk_stmt(s, &mut patch);
                }
            }
        }
        for (file, span, msg) in errors {
            self.error(file, codes::INLINE_REFUSED, span, msg);
        }
        for (file, span, msg) in literal_errors {
            self.error(file, codes::LITERAL, span, msg);
        }
        for (file, span, of) in blocks_in {
            self.error(file, codes::SENTENCE, span, format!("`{}` takes a sentence, and this literal is blocks of Markdown", of));
        }
        // constant lets, each once the ones it reads are known
        let mut prog = self.program();
        let mut left: Vec<usize> = (self.from.lets..self.lets.len()).collect();
        if let Some(l) = left.iter().position(|l| &*self.files[self.lets[*l].file].src == NO_ENTITIES) {
            let l = left.swap_remove(l);
            let v = entities();
            let full = self.lets[l].full.clone();
            prog.lets.insert(full.clone(), v.clone());
            self.out.let_values.insert(l, v.to_ir());
            self.constants.insert(full, v);
        }
        loop {
            let mut progress = false;
            let mut still = Vec::new();
            for l in left {
                let Some(value) = self.out.let_values.get(&l).cloned() else { continue };
                let (out, told) = {
                    let mut m = Machine::new(&prog, Box::new(Constants));
                    (m.eval(&value), m.warned)
                };
                match out {
                    Ok(v) => {
                        let (file, span) = (self.lets[l].file, self.lets[l].span);
                        warned.extend(told.into_iter().map(|w| (file, span, w)));
                        let full = self.lets[l].full.clone();
                        prog.lets.insert(full.clone(), v.clone());
                        self.constants.insert(full, v.clone());
                        self.out.let_values.insert(l, v.to_ir());
                        progress = true;
                    }
                    Err(f) if f.kind == FailKind::NotConstant => still.push(l),
                    Err(_) if self.failed_lets.contains(&l) => {}
                    Err(f) => {
                        let (file, span) = (self.lets[l].file, self.lets[l].span);
                        self.error(file, codes::CONSTANT, span, format!("working out `{}` failed: {}", self.lets[l].name, f.message));
                    }
                }
            }
            left = still;
            if !progress || left.is_empty() {
                break;
            }
        }
        // token values: constants, with their records' rules checked now
        for (d, case, file, src) in self.token_values.iter().skip(self.from.tokens).cloned().collect::<Vec<_>>() {
            let Some(x) = self.out.token_values.get(&(d, case.clone())).cloned() else { continue };
            let mut m = Machine::new(&prog, Box::new(Constants));
            match m.eval(&x) {
                Ok(v) => {
                    warned.extend(m.warned.iter().map(|w| (file, src.span, w.clone())));
                    self.out.token_values.insert((d, case), v.to_ir());
                }
                Err(f) if f.kind == FailKind::NotConstant => {}
                Err(f) => self.error(file, codes::CONSTANT, src.span, format!("`{}.{}`'s value is refused: {}", self.defs[d].name, case, f.rule.unwrap_or(f.message))),
            }
        }
        // defaults that are constants become their values
        let keys: Vec<(DefId, String)> = self.out.defaults.own().keys().cloned().collect();
        for k in keys {
            let x = self.out.defaults.own()[&k].clone();
            let mut m = Machine::new(&prog, Box::new(Constants));
            match m.eval(&x) {
                Ok(v) if !matches!(v, xtmd_eval::Value::Fn(_) | xtmd_eval::Value::View(_)) => {
                    let (file, span) = self.field_span(k.0, &k.1);
                    warned.extend(m.warned.iter().map(|w| (file, span, w.clone())));
                    self.out.defaults.insert(k, v.to_ir());
                }
                Err(f) if !matches!(f.kind, FailKind::NotConstant | FailKind::Ir) => {
                    let (file, span) = self.field_span(k.0, &k.1);
                    self.error(file, codes::CONSTANT, span, format!("the default of `{}` is refused: {}", k.1, f.rule.unwrap_or(f.message)));
                }
                _ => {}
            }
        }
        let keys: Vec<(usize, String)> = self.out.channel_defaults.own().keys().cloned().collect();
        for k in keys {
            let x = self.out.channel_defaults.own()[&k].clone();
            let mut m = Machine::new(&prog, Box::new(Constants));
            if let Ok(v) = m.eval(&x) {
                self.out.channel_defaults.insert(k, v.to_ir());
            }
        }
        // contracts: every proof, now that the constants are known
        self.settle_proofs(&prog);
        self.settle_widget_requires(&prog);
        warned.sort_by(|a, b| (a.0, a.1.start, &a.2).cmp(&(b.0, b.1.start, &b.2)));
        warned.dedup();
        for (file, span, message) in warned {
            self.warning(file, codes::WARN, span, format!("this warns: {}", message));
        }
        self.known_at.clear();
    }

    fn field_span(&self, d: DefId, name: &str) -> (usize, Span) {
        let file = self.defs[d].file;
        let span = match &self.defs[d].kind {
            DefKind::Record { fields } => fields.iter().find(|f| f.name == name).map(|f| f.span),
            DefKind::Widget { inputs, .. } => inputs.iter().find(|f| f.name == name).map(|f| f.span),
            _ => None,
        };
        (file, span.unwrap_or(self.defs[d].span))
    }
}

/// The document's link definitions as core's `map<text>`.
fn links_value(links: &[(String, String)]) -> xtmd_eval::Value {
    use xtmd_eval::Value;
    use std::rc::Rc;
    let entries: Vec<Value> = links
        .iter()
        .map(|(k, v)| Value::Record(Rc::new(xtmd_eval::value::Record { ty: Rc::from("record"), fields: vec![(Rc::from("key"), Value::text(k)), (Rc::from("value"), Value::text(v))] })))
        .collect();
    let mut ids = 0;
    let list = Value::list(entries, &mut ids);
    Value::Record(Rc::new(xtmd_eval::value::Record { ty: Rc::from("core/map"), fields: vec![(Rc::from("entries"), list)] }))
}

fn placeholder(k: usize) -> String {
    char::from_u32(0xE000 + k as u32).unwrap_or('\u{E000}').to_string()
}

fn placeholder_hole(k: usize) -> xtmd_eval::Value {
    use std::rc::Rc;
    xtmd_eval::Value::Case(Rc::new(xtmd_eval::value::Case { ty: Rc::from("core/hole"), name: Rc::from("TEXT"), values: vec![(Rc::from("value"), xtmd_eval::Value::text(&placeholder(k)))] }))
}

fn template_value(m: &mut Machine, pieces: &[String], holes: Vec<xtmd_eval::Value>) -> Result<xtmd_eval::Value, xtmd_eval::Failure> {
    use std::rc::Rc;
    let pieces = m.make_list(pieces.iter().map(|p| xtmd_eval::Value::text(p)).collect())?;
    let holes = m.make_list(holes)?;
    Ok(xtmd_eval::Value::Record(Rc::new(xtmd_eval::value::Record { ty: Rc::from("core/template"), fields: vec![(Rc::from("pieces"), pieces), (Rc::from("holes"), holes)] })))
}

/// Which hole a placeholder text stands for.
pub(crate) fn hole_of(s: &str) -> Option<usize> {
    let mut it = s.chars();
    let c = it.next()?;
    if it.next().is_none() && ('\u{E000}'..='\u{F8FF}').contains(&c) {
        Some(c as usize - 0xE000)
    } else {
        None
    }
}

/// The IR with each placeholder text replaced by its hole's expression.
pub(crate) fn put_holes(mut x: ir::Expr, holes: &[ir::Expr]) -> ir::Expr {
    walk_expr(&mut x, &mut |e| {
        if let ir::Expr::Text(s) = e {
            if let Some(k) = hole_of(s) {
                if let Some(h) = holes.get(k) {
                    *e = h.clone();
                }
            }
        }
    });
    x
}

pub(crate) fn text_of(t: &ast::TextLit) -> String {
    t.parts.iter().map(|p| if let ast::TextPart::Piece { raw, .. } = p { xtmd_syntax::lexer::decode_piece(raw) } else { String::new() }).collect()
}

/// Every expression this compile made: those of what it is made on are done.
fn walk_out(out: &mut decl::Out, first: DefId, f: &mut dyn FnMut(&mut ir::Expr)) {
    for b in out.fn_bodies.own_mut().values_mut() {
        for s in b {
            walk_stmt(s, f);
        }
    }
    for x in out.let_values.own_mut().values_mut() {
        walk_expr(x, f);
    }
    for rs in out.requires.own_mut().values_mut() {
        for r in rs {
            for s in &mut r.body {
                walk_stmt(s, f);
            }
        }
    }
    for x in out.defaults.own_mut().values_mut().chain(out.token_values.own_mut().values_mut()).chain(out.channel_defaults.own_mut().values_mut()).chain(out.case_defaults.own_mut().values_mut()) {
        walk_expr(x, f);
    }
    if let Some(s) = &mut out.state {
        for fl in &mut s.fields {
            if let Some(d) = &mut fl.default {
                walk_expr(d, f);
            }
        }
        for r in &mut s.requires {
            for st in &mut r.body {
                walk_stmt(st, f);
            }
        }
    }
    for e in &mut out.exposes {
        for fun in &mut e.fns {
            for s in &mut fun.body {
                walk_stmt(s, f);
            }
        }
    }
    for (_, t) in out.templates.iter_mut().filter(|(d, _)| *d >= first) {
        walk_template(Rc::make_mut(t), f);
    }
    if let Some(t) = &mut out.doc {
        walk_template(t, f);
    }
    for ns in out.extern_markdown.own_mut().values_mut() {
        for n in ns {
            walk_node(n, f);
        }
    }
}

pub(crate) fn walk_template(t: &mut ir::Template, f: &mut dyn FnMut(&mut ir::Expr)) {
    for c in t.cells.iter_mut().chain(t.inputs.iter_mut()) {
        if let Some(d) = &mut c.default {
            walk_expr(d, f);
        }
    }
    for fun in &mut t.fns {
        for s in &mut fun.body {
            walk_stmt(s, f);
        }
    }
    for r in &mut t.requires {
        for s in &mut r.body {
            walk_stmt(s, f);
        }
    }
    for n in t.view.iter_mut().chain(t.markdown.iter_mut().flatten()) {
        walk_node(n, f);
    }
}

pub(crate) fn walk_expr(e: &mut ir::Expr, f: &mut dyn FnMut(&mut ir::Expr)) {
    use ir::Expr as X;
    match e {
        X::List(v) => v.iter_mut().for_each(|x| walk_expr(x, f)),
        X::Record(r) => r.fields.values_mut().for_each(|x| walk_expr(x, f)),
        X::Case(c) => c.values.values_mut().for_each(|x| walk_expr(x, f)),
        X::Some(x) => walk_expr(x, f),
        X::Range(r) => {
            for x in [&mut r.low, &mut r.high, &mut r.step].into_iter().flatten() {
                walk_expr(x, f);
            }
        }
        X::Tokval(t) => walk_expr(&mut t.value, f),
        X::Rich(r) => std::sync::Arc::make_mut(&mut r.nodes).iter_mut().for_each(|n| walk_node(n, f)),
        X::Sentence(x) => walk_expr(x, f),
        X::Template(t) => t.holes.iter_mut().for_each(|x| walk_expr(x, f)),
        X::View(v) => {
            v.args.values_mut().for_each(|x| walk_expr(x, f));
            for ns in v.slots.values_mut() {
                ns.iter_mut().for_each(|n| walk_node(n, f));
            }
        }
        X::Field(x) => walk_expr(&mut x.value, f),
        X::Call(c) => c.args.iter_mut().for_each(|x| walk_expr(x, f)),
        X::Apply(a) => {
            walk_expr(&mut a.fn_, f);
            a.args.iter_mut().for_each(|x| walk_expr(x, f));
        }
        X::Op(o) => o.args.iter_mut().for_each(|x| walk_expr(x, f)),
        X::Show(s) => walk_expr(&mut s.value, f),
        X::Fn(l) => l.body.iter_mut().for_each(|s| walk_stmt(s, f)),
        X::Place(p) => walk_place(p, f),
        X::Try(t) => walk_expr(&mut t.value, f),
        X::With(w) => {
            walk_expr(&mut w.value, f);
            w.fields.values_mut().for_each(|x| walk_expr(x, f));
        }
        _ => {}
    }
    f(e);
}

fn walk_place(p: &mut ir::Place, f: &mut dyn FnMut(&mut ir::Expr)) {
    for s in &mut p.path {
        if let ir::Step::Item(i) = s {
            walk_expr(&mut i.at, f);
        }
    }
}

fn walk_cond(c: &mut ir::Cond, f: &mut dyn FnMut(&mut ir::Expr)) {
    match c {
        ir::Cond::Cond(x) => walk_expr(x, f),
        ir::Cond::Open(o) => walk_expr(&mut o.opt, f),
    }
}

pub(crate) fn walk_stmt(s: &mut ir::Stmt, f: &mut dyn FnMut(&mut ir::Expr)) {
    use ir::Stmt as S;
    match s {
        S::Declare(ir::StmtDeclare::Let(l)) | S::Declare(ir::StmtDeclare::Mut(l)) => walk_expr(&mut l.value, f),
        S::Set(x) => {
            walk_place(&mut x.place, f);
            walk_expr(&mut x.value, f);
        }
        S::Return(r) => {
            if let Some(v) = &mut r.value {
                walk_expr(v, f);
            }
        }
        S::If(i) => {
            for b in &mut i.branches {
                walk_cond(&mut b.when, f);
                b.then.iter_mut().for_each(|s| walk_stmt(s, f));
            }
            i.else_.iter_mut().for_each(|s| walk_stmt(s, f));
        }
        S::For(x) => {
            walk_expr(&mut x.over, f);
            x.body.iter_mut().for_each(|s| walk_stmt(s, f));
        }
        S::Match(m) => {
            walk_expr(&mut m.value, f);
            for c in &mut m.cases {
                c.then.iter_mut().for_each(|s| walk_stmt(s, f));
            }
            m.else_.iter_mut().for_each(|s| walk_stmt(s, f));
        }
        S::Invoke(i) => i.args.iter_mut().for_each(|x| walk_expr(x, f)),
        S::Call(c) => c.args.iter_mut().for_each(|x| walk_expr(x, f)),
        S::Apply(a) => {
            walk_expr(&mut a.fn_, f);
            a.args.iter_mut().for_each(|x| walk_expr(x, f));
        }
    }
}

pub(crate) fn walk_node(n: &mut ir::Node, f: &mut dyn FnMut(&mut ir::Expr)) {
    use ir::Node as N;
    match n {
        N::Extern(e) => {
            e.props.values_mut().for_each(|x| walk_expr(x, f));
            for ns in e.slots.values_mut() {
                ns.iter_mut().for_each(|n| walk_node(n, f));
            }
            e.writes.values_mut().for_each(|p| walk_place(p, f));
        }
        N::Use(u) => {
            u.args.values_mut().for_each(|x| walk_expr(x, f));
            for ns in u.slots.values_mut() {
                ns.iter_mut().for_each(|n| walk_node(n, f));
            }
        }
        N::For(x) => {
            walk_expr(&mut x.over, f);
            x.body.iter_mut().for_each(|n| walk_node(n, f));
        }
        N::If(i) => {
            for b in &mut i.branches {
                walk_cond(&mut b.when, f);
                b.then.iter_mut().for_each(|n| walk_node(n, f));
            }
            i.else_.iter_mut().for_each(|n| walk_node(n, f));
        }
        N::Match(m) => {
            walk_expr(&mut m.value, f);
            for c in &mut m.cases {
                c.then.iter_mut().for_each(|n| walk_node(n, f));
            }
            m.else_.iter_mut().for_each(|n| walk_node(n, f));
        }
        N::Place(p) => walk_expr(&mut p.view, f),
        N::Declare(d) => walk_expr(&mut d.let_.value, f),
    }
}
