//! Views: widget calls, slots, events and placed values, as the IR's nodes; widgets written in
//! Extended Mark as templates; and markdown blocks, lowered onto core's Markdown blocks.

use crate::check::body::{Cx, LocalKind, WidgetCx};
pub const PROSE_MARK: &str = "@prose";
use crate::check::rich::holds_rich;
use crate::check::ty::Ty;
use crate::check::*;
use crate::syntax::ast::{ExprKind as E, ProseNode};

impl Checker {
    /// A node's site, counted within its template, so a widget's path doesn't move when
    /// another template changes.
    fn site(&mut self, prefix: &str) -> String {
        let n = self.next_site;
        self.next_site += 1;
        format!("{}{}", prefix, n)
    }

    pub(crate) fn widget_cx(&mut self, d: DefId) -> WidgetCx {
        let DefKind::Widget { inputs, slots, events, .. } = self.defs[d].kind.clone() else { unreachable!() };
        WidgetCx {
            def: d,
            inputs: inputs.iter().map(|f| (f.name.clone(), f.ty.clone(), f.mutable)).collect(),
            slots: slots.iter().map(|s| (s.name.clone(), s.ty.clone())).collect(),
            events,
            cells: Vec::new(),
            fns: HashMap::new(),
            declared: inputs.iter().map(|f| (f.name.clone(), f.ir.clone())).collect(),
        }
    }

    fn widget_of(&self, cx: &Cx, name: &str) -> Option<DefId> {
        match self.lookup_type(cx.file, name) {
            Some(TypeSym::Def(d)) if matches!(self.defs[d].kind, DefKind::Widget { .. }) => Some(d),
            _ => None,
        }
    }

    /// Whether a value of this type can follow `@`: a view, a widget's, one of an interface widgets
    /// fit, or a list or opt of one (page 3). Any other value is skipped, so an input `icon: glyph`
    /// never hides the widget `icon`.
    fn holds_view(&self, t: &Ty) -> bool {
        match self.vars.resolve(t) {
            Ty::View | Ty::Rich => true,
            Ty::Def(d, _) => matches!(self.defs[d].kind, DefKind::Widget { .. }),
            Ty::Any(d, _) => matches!(self.defs[d].kind, DefKind::Interface { widget: true, .. }),
            Ty::List(inner) | Ty::Opt(inner) => self.holds_view(&inner),
            // not known yet, or known to be wrong: as a view, so it is reported once, where it is used
            Ty::Param(_) | Ty::Var(_) | Ty::Err => true,
            _ => false,
        }
    }

    /// What follows `@` when it isn't a widget: a slot, a view held in a value, or a fn that returns one.
    fn placed_value(&mut self, cx: &mut Cx, call: &ast::ViewCall) -> Option<(ir::Expr, Ty)> {
        let name = &call.name.name;
        // a rich_text is mostly words, a `label` or a `title`: a widget of its name is the widget
        let widget = self.widget_of(cx, name).is_some();
        let rich = |c: &Self, t: &Ty| matches!(c.vars.resolve(t), Ty::Rich);
        if let Some(l) = cx.local(name).cloned() {
            if call.args.is_none() && self.holds_view(&l.ty) && !(widget && rich(self, &l.ty)) {
                return Some((ir::Expr::Get(ir::Get::Local(name.clone())), l.ty));
            }
            if let Ty::Fn(..) = self.vars.resolve(&l.ty) {
                let g = call.args.clone().unwrap_or_else(|| ast::Group::empty(call.span));
                let callee = ast::Expr { kind: E::Name(call.name.clone()), span: call.name.span };
                return Some(self.expr(cx, &ast::Expr { kind: E::Call { callee: Box::new(callee), args: g }, span: call.span }, None));
            }
        }
        if let Some(w) = cx.widget.clone() {
            if let Some((_, t)) = w.slots.iter().find(|s| &s.0 == name) {
                return Some((ir::Expr::Get(ir::Get::Slot(name.clone())), t.clone()));
            }
            if let Some((_, t, _)) = w.inputs.iter().find(|i| &i.0 == name) {
                if let Ty::Fn(..) = self.vars.resolve(t) {
                    let g = call.args.clone().unwrap_or_else(|| ast::Group::empty(call.span));
                    let callee = ast::Expr { kind: E::Name(call.name.clone()), span: call.name.span };
                    return Some(self.expr(cx, &ast::Expr { kind: E::Call { callee: Box::new(callee), args: g }, span: call.span }, None));
                }
                // a view held in an input takes no arguments: `@name(…)` calls the widget
                if call.args.is_none() && self.holds_view(t) && !(widget && rich(self, t)) {
                    return Some((ir::Expr::Get(ir::Get::Input(name.clone())), t.clone()));
                }
            }
            if w.fns.contains_key(name) {
                let g = call.args.clone().unwrap_or_else(|| ast::Group::empty(call.span));
                let callee = ast::Expr { kind: E::Name(call.name.clone()), span: call.name.span };
                return Some(self.expr(cx, &ast::Expr { kind: E::Call { callee: Box::new(callee), args: g }, span: call.span }, None));
            }
        }
        if let Some(ValueSym::Let(l)) = self.lookup_value(cx.file, name) {
            let ty = self.lets[l].ty.clone();
            if call.args.is_none() && self.holds_view(&ty) && !(widget && rich(self, &ty)) {
                self.warn_deprecated(cx.file, self.lets[l].file, self.lets[l].span, call.name.span);
                return Some((ir::Expr::Get(ir::Get::Let(self.lets[l].full.clone())), ty));
            }
        }
        if let Some(ValueSym::Fn(f)) = self.lookup_value(cx.file, name) {
            let ret = self.fns[f].ret.clone();
            if ret.map_or(false, |r| self.holds_view(&r)) {
                let g = call.args.clone().unwrap_or_else(|| ast::Group::empty(call.span));
                let callee = ast::Expr { kind: E::Name(call.name.clone()), span: call.name.span };
                return Some(self.expr(cx, &ast::Expr { kind: E::Call { callee: Box::new(callee), args: g }, span: call.span }, None));
            }
        }
        None
    }

    /// A view written as an expression: `return @label(…)`, `let a: view = @x;`.
    pub(crate) fn view_value(&mut self, cx: &mut Cx, call: &ast::ViewCall) -> (ir::Expr, Ty) {
        if let Some(out) = self.placed_value(cx, call) {
            if call.block.is_some() {
                self.error(cx.file, codes::CHILDREN_ONE_SLOT, call.name.span, format!("`{}` is a view held in a value, which takes no block", call.name.name));
            }
            return out;
        }
        let Some(d) = self.widget_of(cx, &call.name.name) else {
            let known = self.widget_names(cx.file);
            self.unknown(cx.file, codes::UNKNOWN_WIDGET, call.name.span, format!("there is no widget `{}`", call.name.name), &call.name.name, known.iter().map(String::as_str));
            return (ir::Expr::Num("0".into()), Ty::Err);
        };
        let parts = self.call_parts(cx, d, call, None);
        let full = self.defs[d].full.clone();
        let args = parts.props.into_iter().chain(parts.places.into_iter().map(|(k, p)| (k, ir::Expr::Place(p)))).collect();
        (ir::Expr::View(ir::ViewValue { widget: full, args, slots: parts.slots, events: parts.events, reads: Default::default() }), Ty::Def(d, Vec::new()))
    }

    /// The nodes a block of view code makes.
    pub(crate) fn view_nodes(&mut self, cx: &mut Cx, b: &ast::Block) -> Vec<ir::Node> {
        cx.push();
        let mut out = Vec::new();
        for s in &b.stmts {
            out.extend(self.view_stmt(cx, &s.item));
        }
        cx.pop();
        out
    }

    fn view_stmt(&mut self, cx: &mut Cx, s: &ast::Stmt) -> Vec<ir::Node> {
        use ast::Stmt as S;
        let file = cx.file;
        match s {
            S::Let(v) => {
                let ty = self.resolve(file, &v.ty, &cx.generics.clone());
                let ty_ir = self.ir_type(file, &v.ty, &cx.generics.clone());
                let (x, _) = self.expr(cx, &v.value, Some(&ty));
                self.want(cx, &v.value, &x, &ty_ir, format!("`{}`", v.name.name));
                self.declare_local(cx, &v.name, ty, false, LocalKind::Plain);
                crate::check::proof::set_known(cx, &v.name.name, crate::check::proof::Known::of(&ty_ir));
                vec![ir::Node::Declare(ir::NodeDeclare { let_: ir::Local { name: v.name.name.clone(), ty: ty_ir, value: x }, reads: Default::default() })]
            }
            S::If(i) => {
                let site = self.site("s");
                let mut branches = Vec::new();
                for (c, b) in &i.branches {
                    cx.push();
                    let when = self.cond(cx, c);
                    let then = self.view_nodes(cx, b);
                    cx.pop();
                    branches.push(ir::NodeIfBranches { when, then, reads: Default::default() });
                }
                let else_ = i.otherwise.as_ref().map(|b| self.view_nodes(cx, b)).unwrap_or_default();
                vec![ir::Node::If(ir::NodeIf { site, branches, else_ })]
            }
            S::For(f) => {
                let site = self.site("s");
                cx.push();
                let (over, item) = self.loop_over(cx, f.mutable, &f.over);
                let key = if matches!(self.vars.resolve(&self.expr_type_hint(&over)), Ty::Range(_)) { ir::NodeForKey::Number } else { ir::NodeForKey::Item };
                if let Some(k) = &f.key {
                    self.declare_local(cx, k, Ty::Num, false, LocalKind::Plain);
                }
                let var_ty = if f.mutable { Ty::opt(item.clone()) } else { item.clone() };
                self.declare_local(cx, &f.name, var_ty, false, if f.mutable { LocalKind::MutFor } else { LocalKind::Plain });
                if !f.mutable {
                    self.loop_known(cx, &f.over, f.key.as_ref(), &f.name);
                }
                self.begin_walk(cx, f.mutable, &f.over, &f.name);
                let body = self.view_nodes(cx, &f.body);
                cx.pop();
                vec![ir::Node::For(ir::NodeFor { mutable: f.mutable, name: f.name.name.clone(), position: f.key.as_ref().map(|k| k.name.clone()), ty: self.ir_ty(&item), over, key, site, body, reads: Default::default() })]
            }
            S::Match(m) => {
                let site = self.site("s");
                let (v, vt) = self.expr(cx, &m.subject, None);
                let mut cases = Vec::new();
                let mut else_ = Vec::new();
                let mut has_else = false;
                for c in m.cases.iter() {
                    cx.push();
                    if c.patterns.is_empty() {
                        has_else = true;
                        else_ = self.view_nodes(cx, &c.body);
                    } else {
                        let when = self.patterns(cx, &c.patterns, &vt);
                        let then = self.view_nodes(cx, &c.body);
                        cases.push(ir::NodeMatchCases { when, then });
                    }
                    cx.pop();
                }
                self.check_match(file, &vt, &m.cases.items.iter().map(|n| n.item.patterns.clone()).collect::<Vec<_>>(), has_else, m.span);
                vec![ir::Node::Match(ir::NodeMatch { site, value: v, cases, else_, reads: Default::default() })]
            }
            S::Expr(ast::Expr { kind: E::View(call), .. }) => self.view_call_node(cx, call),
            other => {
                let span = match other {
                    S::Expr(e) => e.span,
                    _ => Span::default(),
                };
                self.error(file, codes::NOT_A_VIEW, span, "a view holds let, if, for, match and views");
                Vec::new()
            }
        }
    }

    fn expr_type_hint(&self, e: &ir::Expr) -> Ty {
        match e {
            ir::Expr::Range(_) => Ty::Range(Box::new(Ty::Num)),
            _ => Ty::Err,
        }
    }

    pub(crate) fn view_call_node(&mut self, cx: &mut Cx, call: &ast::ViewCall) -> Vec<ir::Node> {
        self.view_call_filled(cx, call, None)
    }

    /// A call, its slots filled by its own block or, in prose, by a block of prose.
    fn view_call_filled(&mut self, cx: &mut Cx, call: &ast::ViewCall, prose: Option<ProseFill>) -> Vec<ir::Node> {
        if let Some((x, t)) = self.placed_value(cx, call) {
            let site = self.site("s");
            // a value has no slots, so a block would be lost
            if prose.is_some() || call.block.is_some() {
                self.error(cx.file, codes::CHILDREN_ONE_SLOT, call.name.span, format!("`{}` is a view held in a value, which takes no block", call.name.name));
            }
            // a slot that holds a list is placed item by item
            if let Ty::List(inner) = self.vars.resolve(&t) {
                let inner = self.vars.resolve(&inner);
                let name = format!("{}_item", call.name.name);
                return vec![ir::Node::For(ir::NodeFor {
                    mutable: false,
                    name: name.clone(),
                    position: None,
                    ty: ir::Type::named("core/view"),
                    over: x,
                    key: ir::NodeForKey::Item,
                    site: site.clone(),
                    body: match &inner {
                        Ty::Rich if !cx.sentence => crate::check::rich::alone(format!("{}p", site), ir::Expr::Get(ir::Get::Local(name))),
                        t => vec![ir::Node::Place(ir::NodePlace { sentence: cx.sentence && *t == Ty::Rich, site: format!("{}p", site), view: ir::Expr::Get(ir::Get::Local(name)), reads: Default::default() })],
                    },
                    reads: Default::default(),
                })];
            }
            // an opt of one is placed when it is there
            let rich = match self.vars.resolve(&t) {
                Ty::Rich => true,
                Ty::Opt(inner) => self.vars.resolve(&inner) == Ty::Rich,
                _ => false,
            };
            if rich && !cx.sentence {
                return crate::check::rich::alone(site, x);
            }
            return vec![ir::Node::Place(ir::NodePlace { sentence: rich, site, view: x, reads: Default::default() })];
        }
        let Some(d) = self.widget_of(cx, &call.name.name) else {
            let known = self.widget_names(cx.file);
            self.unknown(cx.file, codes::UNKNOWN_WIDGET, call.name.span, format!("there is no widget `{}`", call.name.name), &call.name.name, known.iter().map(String::as_str));
            return Vec::new();
        };
        self.warn_deprecated(cx.file, self.defs[d].file, self.defs[d].span, call.name.span);
        let site = self.site("s");
        self.sites.push(crate::check::SiteRec { template: self.template_now.clone(), site: site.clone(), file: cx.file, span: call.span });
        let parts = self.call_parts(cx, d, call, prose);
        let full = self.defs[d].full.clone();
        let external = matches!(self.defs[d].kind, DefKind::Widget { external: true, .. });
        if external {
            vec![ir::Node::Extern(ir::NodeExtern { name: full, site, props: parts.props, slots: parts.slots, events: parts.events, writes: parts.places, levels: Vec::new(), reads: Default::default() })]
        } else {
            let mut args = parts.props;
            for (k, p) in parts.places {
                args.insert(k, ir::Expr::Place(p));
            }
            vec![ir::Node::Use(ir::NodeUse { name: full, site, args, slots: parts.slots, events: parts.events, levels: Vec::new(), reads: Default::default() })]
        }
    }

    /// A widget call's inputs, `mut` inputs' places, events (as handlers) and slots.
    fn call_parts(&mut self, cx: &mut Cx, d: DefId, call: &ast::ViewCall, prose: Option<ProseFill>) -> Parts {
        let file = cx.file;
        let DefKind::Widget { inputs, slots, events, .. } = self.defs[d].kind.clone() else { unreachable!() };
        let wname = self.defs[d].name.clone();
        self.note_ref(file, call.name.span, call.span, crate::check::RefTarget::Decl(d));
        let mut parts = Parts::default();
        let mut given: HashSet<String> = HashSet::new();
        let mut named: Vec<(ast::Ident, ast::Expr)> = Vec::new();
        if let Some(args) = &call.args {
            for a in args.iter() {
                match &a.name {
                    Some(n) => named.push((n.clone(), a.value.clone())),
                    None => self.error(file, codes::POSITIONAL, a.value.span, format!("`{}` is a widget: name each argument, `{} = …`", wname, inputs.first().map(|i| i.name.as_str()).unwrap_or("input"))),
                }
            }
        }
        let mut slot_blocks: Vec<(String, ast::Block, Span)> = Vec::new();
        // a block or a name that went nowhere has its own error, so the slots it left aren't errors too
        let mut misplaced = false;
        match &call.block {
            None => {}
            Some(ast::CallBlock::Children(b)) => {
                if slots.len() == 1 {
                    slot_blocks.push((slots[0].name.clone(), b.clone(), b.span));
                } else if !b.stmts.is_empty() || slots.is_empty() {
                    misplaced = true;
                    self.error(file, codes::CHILDREN_ONE_SLOT, call.name.span, format!("`{}` has {} slots: fill each by its name, `slot {{ … }}`", wname, slots.len()));
                }
            }
            Some(ast::CallBlock::Named(body)) => {
                for e in body.iter() {
                    match e {
                        ast::NamedEntry::Input { name, value } => named.push((name.clone(), value.clone())),
                        ast::NamedEntry::Slot { name, block } => {
                            if slots.iter().any(|s| s.name == name.name) {
                                self.note_ref(file, name.span, name.span.to(block.span), crate::check::RefTarget::Member(d, name.name.clone()));
                                slot_blocks.push((name.name.clone(), block.clone(), name.span));
                            } else {
                                misplaced = true;
                                self.unknown(file, codes::UNKNOWN_SLOT, name.span, format!("`{}` has no slot `{}`", wname, name.name), &name.name, slots.iter().map(|s| s.name.as_str()));
                            }
                        }
                    }
                }
            }
        }
        let map: HashMap<String, Ty> = self.defs[d].generics.clone().iter().map(|g| (g.name.clone(), self.vars.fresh())).collect();
        let bounds = cx.bounds.clone();
        for (n, value) in &named {
            self.note_ref(file, n.span, n.span.to(value.span), crate::check::RefTarget::Member(d, n.name.clone()));
            given.insert(n.name.clone());
            if let Some(input) = inputs.iter().find(|i| i.name == n.name) {
                let it = input.ty.subst(&map);
                if input.mutable {
                    match self.place(cx, value) {
                        Some((p, pt)) => {
                            self.check_fields_mut(cx, value);
                            if self.accepts(&it, &pt, &bounds).is_none() {
                                let (a, b) = (self.show(&it), self.show(&pt));
                                self.error(file, codes::MISMATCH, value.span, format!("`{}` is a `mut {}`, given a {}", n.name, a, b));
                            }
                            // shared: the variable's own contract has to be inside the input's
                            if let Some(k) = self.place_known(cx, value) {
                                self.known_at.insert((file, value.span.start, value.span.end), k);
                                self.want_shared(cx, value, &input.ir, format!("`{}`'s `mut {}`", wname, n.name));
                            }
                            parts.places.insert(n.name.clone(), p);
                        }
                        None => {
                            self.expr(cx, value, Some(&it));
                            self.error(file, codes::NOT_MUT, value.span, format!("`{}` is a `mut` input: give it a variable it can write, or leave it out", n.name));
                        }
                    }
                } else {
                    let outer = self.sentence_for.take();
                    if self.piece_of(d).as_deref().map_or(false, |p| crate::check::pieces::takes_sentence(p, &n.name)) {
                        self.sentence_for = Some(wname.clone());
                    }
                    let (x, _) = self.expr(cx, value, Some(&it));
                    self.sentence_for = outer;
                    self.want(cx, value, &x, &input.ir, format!("`{}`'s `{}`", wname, n.name));
                    // the engine runs it, as often as it draws, within the step limit (page 5)
                    if matches!(self.defs[d].kind, DefKind::Widget { external: true, .. }) && matches!(&input.ty, Ty::Fn(_, Some(_))) {
                        let xtmd_fn = match &value.kind {
                            E::Lambda(_) => true,
                            E::Name(fname) => matches!(self.lookup_value(file, &fname.name), Some(crate::check::ValueSym::Fn(f)) if !self.fns[f].external),
                            _ => false,
                        };
                        if xtmd_fn {
                            self.warning(file, codes::ENGINE_XTMD_FN, value.span, format!("`{}`'s `{}` is an Extended Mark fn here, which an engine runs within the step limit, for each pixel of a shader: an extern fn runs natively", wname, n.name));
                        }
                    }
                    parts.props.insert(n.name.clone(), x);
                }
            } else if let Some(ev) = events.iter().find(|e| e.name == n.name) {
                let id = self.handler(cx, ev, value);
                parts.events.insert(n.name.clone(), id);
            } else if slots.iter().any(|s| s.name == n.name) {
                misplaced = true;
                self.error(file, codes::SLOT_EQUALS, n.span, format!("`{}` is a slot: fill it with a block, `{} {{ … }}`, never with `=`", n.name, n.name));
            } else {
                misplaced = true;
                let given: Vec<&str> = inputs.iter().map(|i| i.name.as_str()).chain(events.iter().map(|e| e.name.as_str())).collect();
                self.unknown(file, codes::UNKNOWN_INPUT, n.span, format!("`{}` has no input or event `{}`", wname, n.name), &n.name, given);
            }
        }
        let positional = call.args.as_ref().map_or(false, |a| a.iter().any(|x| x.name.is_none()));
        for i in &inputs {
            if i.default.is_none() && !given.contains(&i.name) && !positional {
                self.error(file, codes::MISSING_INPUT, call.name.span, format!("`{}` needs its input `{}`", wname, i.name));
            }
        }
        let mut filled: Vec<(String, Vec<ir::Node>, Span)> = Vec::new();
        for (name, block, at) in slot_blocks {
            // a rich_text slot's block holds a sentence's calls
            let outside = cx.sentence;
            cx.sentence = slots.iter().any(|s| s.name == name && holds_rich(&s.ir));
            let nodes = self.view_nodes(cx, &block);
            cx.sentence = outside;
            filled.push((name, nodes, at));
        }
        match prose {
            None => {}
            // a plain block fills the widget's one slot
            Some(ProseFill::Plain(nodes)) if slots.len() == 1 => {
                let nodes = self.prose_fill(file, d, &slots[0], nodes, call.name.span);
                filled.push((slots[0].name.clone(), nodes, call.name.span))
            }
            Some(ProseFill::Plain(nodes)) => {
                if !nodes.is_empty() || slots.is_empty() {
                    misplaced = true;
                    if let Some(first) = slots.first() {
                        self.slot_finds.push(crate::check::SlotRec { file, at: call.name.span, whole: call.span, widget: d, slot: first.name.clone(), kind: crate::check::SlotUse::PlainBlock });
                    }
                    let named = slots.iter().map(|s| format!("`@slot {} {{`", s.name)).collect::<Vec<_>>().join(" and ");
                    let how = if slots.is_empty() { "it takes no block".to_string() } else { format!("fill each by its name, with {}", named) };
                    self.error(file, codes::CHILDREN_ONE_SLOT, call.name.span, format!("`{}` has {} slots: {}", wname, slots.len(), how));
                }
            }
            Some(ProseFill::Named(body)) => {
                let (named, lost) = self.prose_slots(cx, d, call, body);
                misplaced |= lost;
                for (name, nodes, at) in named {
                    let nodes = match slots.iter().find(|s| s.name == name) {
                        Some(s) => self.prose_fill(file, d, s, nodes, at),
                        None => nodes,
                    };
                    filled.push((name, nodes, at));
                }
            }
        }
        // every slot the widget declares, written or not, as every input is
        for s in &slots {
            match filled.iter().find(|(n, ..)| *n == s.name) {
                Some((_, nodes, at)) => {
                    let (nodes, at) = (nodes.clone(), *at);
                    self.check_slot(file, &wname, s, &nodes, at);
                }
                None if s.ir.name == "core/opt" => {}
                None if misplaced => {}
                None => {
                    self.slot_finds.push(crate::check::SlotRec { file, at: call.name.span, whole: call.span, widget: d, slot: s.name.clone(), kind: crate::check::SlotUse::LeftOut });
                    self.error(file, codes::MISSING_SLOT, call.name.span, format!("`{}` needs its slot `{}`: only an `opt` slot may be left out", wname, s.name))
                }
            }
        }
        for (name, nodes, _) in filled {
            parts.slots.insert(name, nodes);
        }
        // its rules, run by the compiler when what they read is constant
        self.require_checks.push((file, call.name.span, d, parts.props.clone(), parts.slots.clone()));
        parts
    }

    /// An event set to a fn: its handler, by id.
    fn handler(&mut self, cx: &mut Cx, ev: &EventSym, value: &ast::Expr) -> String {
        let file = cx.file;
        let want = Ty::Fn(ev.params.iter().map(|p| ty::FnParam { ty: p.ty.clone(), mutable: p.mutable }).collect(), None);
        let (x, _) = self.expr(cx, value, Some(&want));
        let id = format!("h{}", self.next_handler);
        self.next_handler += 1;
        let (params, body) = match x {
            ir::Expr::Fn(l) => (l.params, l.body),
            other => {
                // a fn value: the handler applies it
                let params: Vec<ir::Param> = ev.params.iter().map(|p| ir::Param { name: p.name.clone(), ty: p.ir.clone(), mutable: false }).collect();
                let args = params.iter().map(|p| ir::Expr::Get(ir::Get::Local(p.name.clone()))).collect();
                (params, vec![ir::Stmt::Apply(ir::StmtApply { fn_: other, args })])
            }
        };
        let _ = file;
        self.handlers.push(ir::Handler { id: id.clone(), params, body, reads: Default::default(), writes: Default::default() });
        id
    }

    /// A widget written in Extended Mark, as a template; an extern widget's markdown block, as nodes.
    pub(crate) fn widget(&mut self, d: DefId) {
        // each template counts its own sites
        self.next_site = 0;
        self.template_now = self.defs[d].full.clone();
        let file = self.defs[d].file;
        let (ast::DeclKind::Widget(w) | ast::DeclKind::ExternWidget(w)) = self.decl_of(d) else { return };
        let external = matches!(self.defs[d].kind, DefKind::Widget { external: true, .. });
        let mut cx = Cx::new(file);
        cx.generics = self.defs[d].generics.iter().map(|g| g.name.clone()).collect();
        cx.bounds = self.defs[d].generics.clone();
        // a widget reads $state, and its handlers write it
        cx.state = self.state_fields();
        let mut wcx = self.widget_cx(d);
        let DefKind::Widget { inputs, slots, events, .. } = self.defs[d].kind.clone() else { unreachable!() };
        // input defaults
        for i in &inputs {
            if let Some(def) = &i.default {
                let mut c = cx.clone();
                let (x, _) = self.expr(&mut c, def, Some(&i.ty));
                self.want(&c, def, &x, &i.ir, format!("`{}`'s default", i.name));
                self.out.defaults.insert((d, i.name.clone()), x);
            }
        }
        let Some(body) = &w.body else { return };
        let mut cells = Vec::new();
        let mut requires = Vec::new();
        let mut fns = Vec::new();
        let mut view = Vec::new();
        let mut markdown = None;
        let mut view_ast: Option<(Cx, ast::Block)> = None;
        let mut prefix: Vec<ir::Node> = Vec::new();
        // own variables, lets and fns first, so the view sees them all
        for m in body.iter() {
            match m {
                ast::WidgetMember::Own(v) => {
                    let t = self.resolve(file, &v.ty, &cx.generics.clone());
                    let ti = self.ir_type(file, &v.ty, &cx.generics.clone());
                    if self.is_view_type(&t) || self.holds_calls(&t, 0) {
                        self.error(file, codes::VIEW_IN_MUT, v.name.span, "a view is a description, not a variable: hold it in a `let`, or keep the data it is drawn from in the variable instead");
                    }
                    wcx.cells.push((v.name.name.clone(), t));
                    wcx.declared.push((v.name.name.clone(), ti));
                }
                ast::WidgetMember::Fn(f) => {
                    let id = self.fns.len();
                    let full = format!("{}.{}", self.defs[d].full, f.sig.name.name);
                    let mut sym = FnSym {
                        full,
                        name: f.sig.name.name.clone(),
                        pack: self.defs[d].pack.clone(),
                        file,
                        span: f.sig.name.span,
                        on: None,
                        generics: self.defs[d].generics.clone(),
                        params: Vec::new(),
                        ret: None,
                        ret_ir: None,
                        body: f.body.clone(),
                        external: false,
                        widget: Some(d),
                        sig: Some(f.sig.clone()),
                    };
                    sym.params = self.params(file, &f.sig.params, &cx.generics.clone(), &Ty::Err, &ir::Type::named("T"));
                    if let Some(ast::RetType::Type(t)) = &f.sig.ret {
                        sym.ret = Some(self.resolve(file, t, &cx.generics.clone()));
                        sym.ret_ir = Some(self.ir_type(file, t, &cx.generics.clone()));
                    }
                    self.fns.push(sym);
                    wcx.fns.insert(f.sig.name.name.clone(), id);
                }
                _ => {}
            }
        }
        cx.widget = Some(wcx.clone());
        for m in body.iter() {
            match m {
                ast::WidgetMember::Own(v) => {
                    let t = self.resolve(file, &v.ty, &cx.generics.clone());
                    let ti = self.ir_type(file, &v.ty, &cx.generics.clone());
                    let mut c = cx.clone();
                    let (x, _) = self.expr(&mut c, &v.value, Some(&t));
                    self.want(&c, &v.value, &x, &ti, format!("`{}`", v.name.name));
                    cells.push(ir::Field { name: v.name.name.clone(), ty: ti, mutable: true, default: Some(x) });
                }
                ast::WidgetMember::Let(v) => {
                    let t = self.resolve(file, &v.ty, &cx.generics.clone());
                    let ti = self.ir_type(file, &v.ty, &cx.generics.clone());
                    let (x, _) = self.expr(&mut cx, &v.value, Some(&t));
                    self.want(&cx, &v.value, &x, &ti, format!("`{}`", v.name.name));
                    self.declare_local(&mut cx, &v.name, t, false, LocalKind::Plain);
                    crate::check::proof::set_known(&mut cx, &v.name.name, crate::check::proof::Known::of(&ti));
                    prefix.push(ir::Node::Declare(ir::NodeDeclare { let_: ir::Local { name: v.name.name.clone(), ty: ti, value: x }, reads: Default::default() }));
                }
                ast::WidgetMember::Require(r) => {
                    let mut c = cx.clone();
                    c.ret = Some(Some(Ty::Bool));
                    let b = self.block(&mut c, &r.body);
                    let message = r.message.parts.iter().map(|p| if let ast::TextPart::Piece { raw, .. } = p { crate::syntax::lexer::decode_piece(raw) } else { String::new() }).collect();
                    requires.push(ir::Require { warn: r.warn, message, body: b, reads: Default::default() });
                }
                ast::WidgetMember::Fn(f) => {
                    let id = wcx.fns[&f.sig.name.name];
                    let s = self.fns[id].clone();
                    let mut c = cx.clone();
                    c.push();
                    for p in &s.params {
                        c.scopes.last_mut().unwrap().push(body::Local { name: p.name.clone(), ty: p.ty.clone(), mutable: p.mutable, kind: LocalKind::Plain, known: Some(crate::check::proof::Known::of(&p.ir)), own_origin: false });
                    }
                    c.ret = Some(s.ret.clone());
                    c.ret_ir = s.ret_ir.clone();
                    c.action = s.ret.is_none();
                    c.fn_id = Some(id);
                    c.fn_depth = c.scopes.len() - 1;
                    self.check_value_fn_params(file, &f.sig);
                    let b = self.block(&mut c, f.body.as_ref().unwrap());
                    fns.push(ir::FnDecl {
                        name: s.full.clone(),
                        params: s.params.iter().map(|p| ir::Param { name: p.name.clone(), ty: p.ir.clone(), mutable: p.mutable }).collect(),
                        generics: Vec::new(),
                        returns: s.ret_ir.clone(),
                        body: b,
                        reads: Default::default(), writes: Default::default(),
                    });
                }
                ast::WidgetMember::View(b) => {
                    let mut c = cx.clone();
                    view = self.view_nodes(&mut c, b);
                    view_ast = Some((c, b.clone()));
                }
                ast::WidgetMember::Tokens(refs) => {
                    let tokens = self.extern_tokens(file, refs);
                    self.out.extern_tokens.entry_or_default(d).extend(tokens);
                }
                ast::WidgetMember::Uses(refs) => {
                    let levels = self.extern_uses(file, refs);
                    self.out.extern_uses.entry_or_default(d).extend(levels);
                }
                ast::WidgetMember::Markdown(md) => {
                    if let Some(p) = &md.piece {
                        if let Some(piece) = self.check_piece(d, p) {
                            self.out.extern_piece.insert(d, piece);
                        }
                    } else if md.empty {
                        self.warning(file, codes::EMPTY_MARKDOWN, w.name.span, format!("`{}` writes no Markdown (`markdown {{ }}`): the widget around it says what it shows", w.name.name));
                        markdown = Some(Vec::new());
                    } else {
                        let mut c = cx.clone();
                        markdown = Some(self.prose_nodes(&mut c, &md.prose));
                    }
                }
                _ => {}
            }
        }
        if external {
            if let Some(md) = markdown {
                self.out.extern_markdown.insert(d, md);
            }
            self.out.requires.insert(d, requires);
            return;
        }
        // a document's widget may leave its markdown out: it is built from the view (page 1)
        if markdown.is_none() {
            if let Some((c, b)) = &view_ast {
                self.warn_built_markdown(c, &b.stmts);
            }
        }
        let mut full_view = prefix.clone();
        full_view.extend(view);
        let md = markdown.map(|m| prefix.into_iter().chain(m).collect());
        let t = ir::Template {
            name: self.defs[d].full.clone(),
            generics: self.ir_generics(&self.defs[d].generics),
            inputs: inputs.iter().map(|f| ir::Field { name: f.name.clone(), ty: f.ir.clone(), mutable: f.mutable, default: self.out.defaults.get(&(d, f.name.clone())).cloned() }).collect(),
            slots: slots.iter().map(|s| ir::Slot { name: s.name.clone(), ty: s.ir.clone() }).collect(),
            events: events.iter().map(|e| ir::Event { name: e.name.clone(), params: e.params.iter().map(|p| ir::Param { name: p.name.clone(), ty: p.ir.clone(), mutable: p.mutable }).collect() }).collect(),
            cells,
            requires,
            fns,
            view: full_view,
            markdown: md,
            levels: Vec::new(),
        };
        self.out.templates.push((d, Rc::new(t)));
    }

    /// Prose as nodes. Each run of Markdown between Extended Mark's own lines is split into blocks, and
    /// each block becomes the extern widget of its kind. A paragraph's text is read by core's
    /// `rich_text.read` when the constants are worked out (passes.rs).
    pub(crate) fn prose_nodes(&mut self, cx: &mut Cx, prose: &ast::Prose) -> Vec<ir::Node> {
        let file = cx.file;
        let src = self.files[file].src.clone();
        let mut out = Vec::new();
        let mut run = String::new();
        let mut holes: Vec<(ir::Expr, Ty, Span)> = Vec::new();
        for node in &prose.nodes {
            match node {
                ProseNode::Text(line) => {
                    if run.is_empty() {
                        self.prose_span = Span::new(line.span.start as usize, line.span.end as usize);
                    }
                    // raw HTML is shown as text (page 1), so an HTML comment shows: `@//` is prose's
                    let text = &src[line.span.range()];
                    if let Some(k) = text.find("<!--") {
                        if text[..k].trim().is_empty() {
                            let at = line.span.start as usize + k;
                            self.warning(file, codes::HTML_COMMENT, Span::new(at, line.span.end as usize), "this HTML comment is shown as text: write `@//` at the start of a line for a comment, which nothing draws");
                        }
                    }
                    let mut at = line.span.start as usize;
                    for h in &line.holes {
                        run.push_str(&src[at..h.span.start as usize]);
                        let (x, t) = self.expr(cx, &h.expr, None);
                        run.push(char::from_u32(0xE000 + holes.len() as u32).unwrap_or('\u{E000}'));
                        holes.push((x, t, h.span));
                        at = h.span.end as usize;
                    }
                    run.push_str(&src[at..line.span.end as usize]);
                    run.push('\n');
                }
                ProseNode::Fence(lines) => {
                    for l in lines {
                        run.push_str(&src[l.span.range()]);
                        run.push('\n');
                    }
                }
                // a comment draws nothing and ends no paragraph
                ProseNode::Comment { .. } => {}
                other => {
                    self.flush_markdown(cx, &mut run, &mut holes, &mut out);
                    match other {
                        ProseNode::Maths(lines) => {
                            let mut pieces = vec![String::new()];
                            let mut hs = Vec::new();
                            let inner = &lines[1..lines.len().saturating_sub(1)];
                            for (k, l) in inner.iter().enumerate() {
                                if k > 0 {
                                    pieces.last_mut().unwrap().push('\n');
                                }
                                let mut at = l.span.start as usize;
                                for h in &l.holes {
                                    pieces.last_mut().unwrap().push_str(&src[at..h.span.start as usize]);
                                    let (x, _) = self.expr(cx, &h.expr, None);
                                    hs.push(ir::Expr::Show(Box::new(ir::ExprShow { value: x })));
                                    pieces.push(String::new());
                                    at = h.span.end as usize;
                                }
                                pieces.last_mut().unwrap().push_str(&src[at..l.span.end as usize]);
                            }
                            let site = self.site("m");
                            self.prose_span = Span::new(lines[0].span.start as usize, lines[0].span.end as usize);
                            let tex = ir::Expr::Template(Box::new(ir::ExprTemplate { pieces, holes: hs, ty: ir::Type::named("core/latex") }));
                            let tex = self.prose_mark(cx, tex, "latex", Vec::new());
                            out.push(extern_node("core/math", site, vec![("tex", tex)], BTreeMap::new()));
                        }
                        ProseNode::Call(c) => {
                            let mut call = c.call.clone();
                            let fill = match &c.block {
                                // `@slot name {` blocks fill the slots they name (page 1)
                                Some((body, _)) if body.nodes.iter().any(|n| matches!(n, ProseNode::Slot { .. })) => Some(ProseFill::Named(body)),
                                // a call's block of prose fills its one slot
                                Some((body, _)) => Some(ProseFill::Plain(self.prose_nodes(cx, body))),
                                None => {
                                    call.block = None;
                                    None
                                }
                            };
                            out.extend(self.view_call_filled(cx, &call, fill));
                        }
                        ProseNode::If { branches, otherwise, .. } => {
                            let site = self.site("m");
                            let mut bs = Vec::new();
                            for (c, body, _) in branches {
                                cx.push();
                                let when = self.cond(cx, c);
                                let then = self.prose_nodes(cx, body);
                                cx.pop();
                                bs.push(ir::NodeIfBranches { when, then, reads: Default::default() });
                            }
                            let else_ = otherwise.as_ref().map(|(b, _)| self.prose_nodes(cx, b)).unwrap_or_default();
                            out.push(ir::Node::If(ir::NodeIf { site, branches: bs, else_ }));
                        }
                        ProseNode::For { mutable, key, name, over, body, .. } => {
                            let site = self.site("m");
                            cx.push();
                            let (o, item) = self.loop_over(cx, *mutable, over);
                            if let Some(k) = key {
                                self.declare_local(cx, k, Ty::Num, false, LocalKind::Plain);
                            }
                            let vt = if *mutable { Ty::opt(item.clone()) } else { item.clone() };
                            self.declare_local(cx, name, vt, false, if *mutable { LocalKind::MutFor } else { LocalKind::Plain });
                            if !*mutable {
                                self.loop_known(cx, over, key.as_ref(), name);
                            }
                            self.begin_walk(cx, *mutable, over, name);
                            let b = self.prose_nodes(cx, body);
                            cx.pop();
                            out.push(ir::Node::For(ir::NodeFor { mutable: *mutable, name: name.name.clone(), position: key.as_ref().map(|k| k.name.clone()), ty: self.ir_ty(&item), over: o, key: ir::NodeForKey::Item, site, body: b, reads: Default::default() }));
                        }
                        ProseNode::Match { subject, cases, span, .. } => {
                            let site = self.site("m");
                            let (v, vt) = self.expr(cx, subject, None);
                            let mut cs = Vec::new();
                            let mut else_ = Vec::new();
                            let mut has_else = false;
                            for c in cases {
                                cx.push();
                                if c.patterns.is_empty() {
                                    has_else = true;
                                    else_ = self.prose_nodes(cx, &c.body);
                                } else {
                                    let when = self.patterns(cx, &c.patterns, &vt);
                                    let then = self.prose_nodes(cx, &c.body);
                                    cs.push(ir::NodeMatchCases { when, then });
                                }
                                cx.pop();
                            }
                            self.check_match(file, &vt, &cases.iter().map(|c| c.patterns.clone()).collect::<Vec<_>>(), has_else, *span);
                            out.push(ir::Node::Match(ir::NodeMatch { site, value: v, cases: cs, else_, reads: Default::default() }));
                        }
                        ProseNode::Slot { .. } | ProseNode::Text(_) | ProseNode::Fence(_) | ProseNode::Comment { .. } => {}
                    }
                }
            }
        }
        self.flush_markdown(cx, &mut run, &mut holes, &mut out);
        out
    }

    /// A run of Markdown, as blocks.
    fn flush_markdown(&mut self, cx: &mut Cx, run: &mut String, holes: &mut Vec<(ir::Expr, Ty, Span)>, out: &mut Vec<ir::Node>) {
        if run.trim().is_empty() {
            run.clear();
            holes.clear();
            return;
        }
        let doc = crate::syntax::markdown::parse(run);
        for l in &doc.links {
            let key = crate::syntax::markdown::normalize_label(&l.label);
            if !self.prose_links.iter().any(|(k, _)| *k == key) {
                self.prose_links.push((key, l.dest.clone()));
            }
        }
        let hs = std::mem::take(holes);
        for b in &doc.blocks {
            let n = self.md_block(cx, b, &hs);
            out.extend(n);
        }
        run.clear();
    }

    fn md_blocks(&mut self, cx: &mut Cx, blocks: &[crate::syntax::markdown::MdBlock], holes: &[(ir::Expr, Ty, Span)]) -> Vec<ir::Node> {
        let mut out = Vec::new();
        for b in blocks {
            out.extend(self.md_block(cx, b, holes));
        }
        out
    }

    fn md_block(&mut self, cx: &mut Cx, b: &crate::syntax::markdown::MdBlock, holes: &[(ir::Expr, Ty, Span)]) -> Vec<ir::Node> {
        use crate::syntax::markdown::{Align, MdBlock};
        let site = self.site("m");
        let node = match b {
            MdBlock::Paragraph(text) => {
                // a paragraph that is only a hole holding a view places that view
                let t: Vec<char> = text.chars().collect();
                if t.len() == 1 && ('\u{E000}'..='\u{F8FF}').contains(&t[0]) {
                    let k = t[0] as usize - 0xE000;
                    if let Some((x, ty, _)) = holes.get(k) {
                        if self.hole_kind(ty) == body::Hole::Rich {
                            return crate::check::rich::alone(site, x.clone());
                        }
                        if self.is_view_type(ty) {
                            return vec![ir::Node::Place(ir::NodePlace { sentence: false, site, view: x.clone(), reads: Default::default() })];
                        }
                    }
                }
                let value = self.rich_template(cx, text, holes);
                extern_node("core/paragraph", site, vec![("value", value)], BTreeMap::new())
            }
            MdBlock::Heading { level, text } => {
                let value = self.rich_template(cx, text, holes);
                extern_node("core/heading", site, vec![("level", ir::Expr::Num(level.to_string())), ("value", value)], BTreeMap::new())
            }
            MdBlock::Bullets { ordered, start, items, .. } => {
                let mut nodes = Vec::new();
                for it in items {
                    let isite = self.site("m");
                    let body = self.md_blocks(cx, &it.blocks, holes);
                    let done = match it.done {
                        Some(d) => ir::Expr::Some(Box::new(ir::Expr::Bool(d))),
                        None => ir::Expr::None(ir::Empty {}),
                    };
                    let mut slots = BTreeMap::new();
                    slots.insert("body".to_string(), body);
                    nodes.push(extern_node("core/item", isite, vec![("done", done)], slots));
                }
                let mut slots = BTreeMap::new();
                slots.insert("items".to_string(), nodes);
                extern_node("core/bullets", site, vec![("ordered", ir::Expr::Bool(*ordered)), ("start", ir::Expr::Num(start.to_string()))], slots)
            }
            MdBlock::Quote(blocks) => {
                let mut slots = BTreeMap::new();
                slots.insert("body".to_string(), self.md_blocks(cx, blocks, holes));
                extern_node("core/quote", site, vec![], slots)
            }
            MdBlock::Code { lang, src } => extern_node("core/code", site, vec![("src", ir::Expr::Text(unsentinel(src, holes))), ("lang", ir::Expr::Text(lang.clone()))], BTreeMap::new()),
            MdBlock::Html(text) => {
                // raw HTML is shown as text, never run
                let words = extern_node("core/words", "t0".into(), vec![("value", ir::Expr::Text(unsentinel(text, holes)))], BTreeMap::new());
                let rich = ir::Expr::Rich(ir::Rich { blocks: false, nodes: std::sync::Arc::new(vec![words]) });
                extern_node("core/paragraph", site, vec![("value", rich)], BTreeMap::new())
            }
            MdBlock::Table { align, head, rows } => {
                let head: Vec<ir::Expr> = head.iter().map(|c| self.rich_template(cx, c, holes)).collect();
                let rows: Vec<ir::Expr> = rows.iter().map(|r| ir::Expr::List(r.iter().map(|c| self.rich_template(cx, c, holes)).collect())).collect();
                let align: Vec<ir::Expr> = align
                    .iter()
                    .map(|a| {
                        let name = match a {
                            Align::Center => "CENTER",
                            Align::End => "END",
                            _ => "START",
                        };
                        ir::Expr::Case(ir::ExprCase { enum_: "core/cell_align".into(), name: name.into(), values: BTreeMap::new() })
                    })
                    .collect();
                extern_node("core/table", site, vec![("head", ir::Expr::List(head)), ("rows", ir::Expr::List(rows)), ("align", ir::Expr::List(align))], BTreeMap::new())
            }
            MdBlock::Footnote { mark, blocks } => {
                let mut slots = BTreeMap::new();
                slots.insert("body".to_string(), self.md_blocks(cx, blocks, holes));
                extern_node("core/footnote", site, vec![("mark", ir::Expr::Text(mark.clone()))], slots)
            }
            MdBlock::Divider => extern_node("core/divider", site, vec![], BTreeMap::new()),
        };
        vec![node]
    }

    /// Inline Markdown with holes, as a template core reads when the constants are worked out.
    fn rich_template(&mut self, cx: &mut Cx, text: &str, holes: &[(ir::Expr, Ty, Span)]) -> ir::Expr {
        // in a sentence `$` starts maths, so a bare `$state.x` shows as text (page 1)
        if bare_state(text) {
            self.warning(cx.file, codes::BARE_STATE, self.prose_span, "`$state.…` in a sentence is shown as text, since `$` starts maths there: put it in a hole, `\\{$state.tries}`");
        }
        let mut pieces = vec![String::new()];
        let mut hs = Vec::new();
        let mut kinds = Vec::new();
        for c in text.chars() {
            if ('\u{E000}'..='\u{F8FF}').contains(&c) {
                let k = c as usize - 0xE000;
                if let Some((x, ty, span)) = holes.get(k) {
                    let ty = self.vars.resolve(ty);
                    self.check_hole(cx, &ty, *span, true);
                    if let Ty::Opt(_) = ty {
                        self.error(cx.file, codes::OPT_HOLE, *span, "a paragraph takes rich_text, not an opt: give this hole a fallback, `\\{x.or(…)}`, or open it first with `@if (let …)`");
                    }
                    kinds.push(self.hole_kind(&ty));
                    hs.push(if ty == Ty::Text || self.is_view_type(&ty) { x.clone() } else { ir::Expr::Show(Box::new(ir::ExprShow { value: x.clone() })) });
                    pieces.push(String::new());
                    continue;
                }
            }
            pieces.last_mut().unwrap().push(c);
        }
        let t = ir::Expr::Template(Box::new(ir::ExprTemplate { pieces, holes: hs, ty: ir::Type::named("core/rich_text") }));
        self.prose_mark(cx, t, "rich_text", kinds)
    }

    /// A template core reads when the constants are worked out, marked with where it is.
    pub(crate) fn prose_mark(&mut self, cx: &Cx, t: ir::Expr, ty: &str, holes: Vec<body::Hole>) -> ir::Expr {
        let id = self.pending.len();
        self.pending.push(body::Pending { file: cx.file, span: self.prose_span, ty: ty.to_string(), holes, sentence_for: None });
        ir::Expr::Call(Box::new(ir::Call { name: format!("{}#{}", PROSE_MARK, id), args: vec![t], ty: None }))
    }

    pub(crate) fn is_view_type(&self, t: &Ty) -> bool {
        let t = self.vars.resolve(t);
        matches!(t, Ty::View | Ty::Rich | Ty::Any(..)) || matches!(&t, Ty::Def(d, _) if matches!(self.defs[*d].kind, DefKind::Widget { .. }))
    }

    /// A prose call's `@slot name {` blocks, each filling the slot it names. Anything else in the
    /// block but blank lines is an error: a widget with several slots fills each by its name.
    /// The slots a prose call's `@slot name {` blocks fill, each with where it is named, and whether
    /// anything in the block went to no slot.
    fn prose_slots(&mut self, cx: &mut Cx, d: DefId, call: &ast::ViewCall, body: &ast::Prose) -> (Vec<(String, Vec<ir::Node>, Span)>, bool) {
        let file = cx.file;
        let src = self.files[file].src.clone();
        let wname = self.defs[d].name.clone();
        let DefKind::Widget { slots, .. } = self.defs[d].kind.clone() else { return (Vec::new(), false) };
        let mut lost = false;
        let mut out: Vec<(String, Vec<ir::Node>, Span)> = Vec::new();
        for node in &body.nodes {
            match node {
                ProseNode::Slot { name, body, .. } => {
                    if !slots.iter().any(|s| s.name == name.name) {
                        lost = true;
                        self.unknown(file, codes::UNKNOWN_SLOT, name.span, format!("`{}` has no slot `{}`", wname, name.name), &name.name, slots.iter().map(|s| s.name.as_str()));
                        continue;
                    }
                    let nodes = self.prose_nodes(cx, body);
                    out.retain(|(n, ..)| *n != name.name);
                    out.push((name.name.clone(), nodes, name.span));
                }
                ProseNode::Text(line) if src[line.span.range()].trim().is_empty() => {}
                ProseNode::Comment { .. } => {}
                _ => {
                    lost = true;
                    self.error(file, codes::CHILDREN_ONE_SLOT, call.name.span, format!("`{}` takes `@slot name {{` blocks here: put this inside one of them", wname));
                    break;
                }
            }
        }
        (out, lost)
    }

}

pub(crate) fn extern_node(name: &str, site: String, props: Vec<(&str, ir::Expr)>, slots: BTreeMap<String, Vec<ir::Node>>) -> ir::Node {
    ir::Node::Extern(ir::NodeExtern {
        name: name.to_string(),
        site,
        props: props.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        slots,
        events: BTreeMap::new(),
        writes: BTreeMap::new(),
        levels: Vec::new(),
        reads: Default::default(),
    })
}

/// Text with any hole sentinels written back as holes would read.
fn unsentinel(s: &str, _holes: &[(ir::Expr, Ty, Span)]) -> String {
    s.chars().map(|c| if ('\u{E000}'..='\u{F8FF}').contains(&c) { '\u{FFFD}' } else { c }).collect()
}

/// What a block of prose gives a call's slots.
enum ProseFill<'a> {
    /// Its nodes, for the widget's one slot.
    Plain(Vec<ir::Node>),
    /// `@slot name {` blocks.
    Named(&'a ast::Prose),
}

#[derive(Default)]
struct Parts {
    props: BTreeMap<String, ir::Expr>,
    places: BTreeMap<String, ir::Place>,
    slots: BTreeMap<String, Vec<ir::Node>>,
    events: BTreeMap<String, String>,
}

// ---- slots' types and lengths, and a document widget's built Markdown

impl Checker {
    /// A slot with a widget type takes only calls of that type, through `for`, `if` and `match`;
    /// a length contract is checked here when the children are written out one by one.
    pub(crate) fn check_slot(&mut self, file: usize, widget: &str, slot: &crate::check::SlotSym, nodes: &[ir::Node], at: Span) {
        // the element type: a list's items, an opt's inside
        let mut el = &slot.ir;
        let mut list: Option<&ir::Type> = None;
        while matches!(el.name.as_str(), "core/list" | "core/opt") {
            if el.name == "core/list" {
                list = Some(el);
            }
            match el.args.first() {
                Some(a) => el = a,
                None => break,
            }
        }
        let want: Option<(String, Option<DefId>)> = match el.name.as_str() {
            "core/view" | "core/rich_text" => None,
            "any" => el.interface.clone().map(|i| (i.clone(), self.defs.iter().position(|d| d.full == i))),
            name => self.defs.iter().any(|d| d.full == name && matches!(d.kind, DefKind::Widget { .. })).then(|| (name.to_string(), None)),
        };
        let mut calls = Vec::new();
        let mut one_by_one = true;
        collect_calls(nodes, &mut calls, &mut one_by_one);
        if let Some((name, iface)) = &want {
            for c in &calls {
                let ok = match (c, iface) {
                    (Some(called), Some(i)) => match self.full_def(called) {
                        Some(w) => self.accepts(&Ty::Any(*i, Vec::new()), &Ty::Def(w, self.defs[w].generics.iter().map(|_| Ty::Err).collect()), &[]).is_some(),
                        None => false,
                    },
                    (Some(called), None) => called == name,
                    (None, _) => false,
                };
                if !ok {
                    let short = name.rsplit('/').next().unwrap_or(name);
                    let what = match c {
                        Some(called) => format!("`{}`", called.rsplit('/').next().unwrap_or(called)),
                        None => "text or a view held in a value".into(),
                    };
                    let takes = if iface.is_some() { format!("widgets that fit `{}`", short) } else { format!("`{}` widgets", short) };
                    self.error(file, codes::SLOT_TYPE, at, format!("`{}`'s slot `{}` takes only {}, with `for`, `if` and `match` around them; {} can't go there", widget, slot.name, takes, what));
                    break;
                }
            }
        }
        // a widget type holds exactly one call, and an `opt` of one at most one
        if one_by_one && want.is_some() && list.is_none() && calls.len() != 1 && !(slot.ir.name == "core/opt" && calls.is_empty()) {
            let many = if slot.ir.name == "core/opt" { "at most one" } else { "exactly one" };
            self.error(file, codes::SLOT_LENGTH, at, format!("`{}`'s slot `{}` holds {} {}, and {} {} written here", widget, slot.name, many, el.name.rsplit('/').next().unwrap_or("widget"), calls.len(), if calls.len() == 1 { "is" } else { "are" }));
        }
        // a length, when the children are written out one by one
        if one_by_one && matches!(slot.ir.name.as_str(), "core/list" | "core/opt") && list.is_some() {
            if let Some(len) = list.and_then(|l| l.contract.as_ref()).and_then(|c| c.length.clone()) {
                let n = calls.len() as i64;
                let num = |x: &Option<ir::Expr>| match x {
                    Some(ir::Expr::Num(t)) => t.parse::<i64>().ok(),
                    _ => None,
                };
                let (lo, hi) = (num(&len.low), num(&len.high));
                if lo.map_or(false, |l| n < l) || hi.map_or(false, |h| n > h) {
                    let shown = crate::check::proof::show_contract(&slot.ir);
                    self.error(file, codes::SLOT_LENGTH, at, format!("`{}`'s slot `{}` is a {}, and {} {} written here", widget, slot.name, shown, n, if n == 1 { "is" } else { "are" }));
                }
            }
        }
    }

    /// A document widget with no `markdown` block: whether its view's branches read a variable,
    /// which its built Markdown, written with the starting values, can't follow.
    pub(crate) fn warn_built_markdown(&mut self, cx: &Cx, stmts: &[ast::Node<ast::Stmt>]) {
        for s in stmts {
            match &s.item {
                ast::Stmt::If(i) => {
                    for (c, b) in &i.branches {
                        let e = match c {
                            ast::Cond::Expr(e) => e,
                            ast::Cond::Open { value, .. } => value,
                        };
                        if self.reads_variable(cx, e) {
                            self.warning(cx.file, codes::BUILT_MARKDOWN, e.span, "this branch reads a variable, and the widget's Markdown is built from its view with the starting values: write a `markdown` block to say what it is either way");
                        }
                        self.warn_built_markdown(cx, &b.stmts);
                    }
                    if let Some(b) = &i.otherwise {
                        self.warn_built_markdown(cx, &b.stmts);
                    }
                }
                ast::Stmt::Match(m) => {
                    if self.reads_variable(cx, &m.subject) {
                        self.warning(cx.file, codes::BUILT_MARKDOWN, m.span, "this match reads a variable, and the widget's Markdown is built from its view with the starting values: write a `markdown` block to say what it is either way");
                    }
                    for c in m.cases.iter() {
                        self.warn_built_markdown(cx, &c.body.stmts);
                    }
                }
                ast::Stmt::For(f) => self.warn_built_markdown(cx, &f.body.stmts),
                ast::Stmt::Expr(ast::Expr { kind: ast::ExprKind::View(v), .. }) => match &v.block {
                    Some(ast::CallBlock::Children(b)) => self.warn_built_markdown(cx, &b.stmts),
                    Some(ast::CallBlock::Named(body)) => {
                        for entry in body.iter() {
                            if let ast::NamedEntry::Slot { block, .. } = entry {
                                self.warn_built_markdown(cx, &block.stmts);
                            }
                        }
                    }
                    None => {}
                },
                _ => {}
            }
        }
    }

    /// Whether an expression reads `$state`, the widget's own variable or a `mut` input.
    fn reads_variable(&self, cx: &Cx, e: &ast::Expr) -> bool {
        use ast::ExprKind as E;
        let r = |x: &ast::Expr| self.reads_variable(cx, x);
        match &e.kind {
            E::State => true,
            E::Name(n) => cx.local(&n.name).is_none() && cx.widget.as_ref().map_or(false, |w| w.cells.iter().any(|c| c.0 == n.name) || w.inputs.iter().any(|i| i.0 == n.name && i.2)),
            E::Paren(x) | E::Try(x) => r(x),
            E::Field { target, .. } => r(target),
            E::Index { target, index } => r(target) || r(index),
            E::Call { callee, args } => r(callee) || args.iter().any(|a| r(&a.value)),
            E::Case { target, args, .. } => r(target) || args.as_ref().map_or(false, |a| a.iter().any(|x| r(&x.value))),
            E::With { target, inits } => r(target) || inits.iter().any(|i| r(&i.value)),
            E::Record { inits, .. } => inits.iter().any(|i| r(&i.value)),
            E::List(items) => items.iter().any(|x| r(x)),
            E::Unary { operand, .. } => r(operand),
            E::Binary { left, right, .. } => r(left) || r(right),
            E::Ternary { cond, then, otherwise } => r(cond) || r(then) || r(otherwise),
            E::Range { low, high, step } => [low, high, step].iter().any(|x| x.as_ref().map_or(false, |x| r(x))),
            E::Text(t) => t.parts.iter().any(|p| matches!(p, ast::TextPart::Hole(h) if r(h))),
            _ => false,
        }
    }
}

/// The calls a slot is filled with, through loops and branches; `None` for anything that isn't
/// a call. Whether they are written out one by one, with no loop or branch around them.
fn collect_calls(nodes: &[ir::Node], out: &mut Vec<Option<String>>, one_by_one: &mut bool) {
    for n in nodes {
        match n {
            ir::Node::Extern(e) => out.push(Some(e.name.clone())),
            ir::Node::Use(u) => out.push(Some(u.name.clone())),
            ir::Node::Place(_) => out.push(None),
            ir::Node::For(f) => {
                *one_by_one = false;
                collect_calls(&f.body, out, one_by_one);
            }
            ir::Node::If(i) => {
                *one_by_one = false;
                for b in &i.branches {
                    collect_calls(&b.then, out, one_by_one);
                }
                collect_calls(&i.else_, out, one_by_one);
            }
            ir::Node::Match(m) => {
                *one_by_one = false;
                for c in &m.cases {
                    collect_calls(&c.then, out, one_by_one);
                }
                collect_calls(&m.else_, out, one_by_one);
            }
            ir::Node::Declare(_) => {}
        }
    }
}

/// Whether a sentence holds `$state.` outside a code span.
fn bare_state(text: &str) -> bool {
    let mut in_code = false;
    let chars: Vec<char> = text.chars().collect();
    for (k, c) in chars.iter().enumerate() {
        if *c == '`' {
            in_code = !in_code;
        } else if !in_code && *c == '$' && chars[k + 1..].iter().take(6).collect::<String>() == "state." && (k == 0 || chars[k - 1] != '\\') {
            return true;
        }
    }
    false
}
