//! rich_text as calls: what core's parser read of a literal or a paragraph becomes the externs of its
//! marks and parts, and a rich_text placed on its own is a paragraph when it is a sentence.

use std::collections::BTreeMap;
use std::sync::Arc;

use xtmd_eval::Value;

use crate::body::Hole;
use crate::passes::{hole_of, put_holes};
use crate::ty::Ty;
use crate::view::extern_node;
use crate::*;

/// A rich_text, or an opt of one.
pub(crate) fn holds_rich(t: &ir::Type) -> bool {
    t.name == "core/rich_text" || (t.name == "core/opt" && t.args.first().map_or(false, holds_rich))
}

impl Checker {
    /// Whether a value of this type holds a rich_text or a view, however deep.
    pub(crate) fn holds_calls(&self, t: &Ty, depth: usize) -> bool {
        let t = self.vars.resolve(t);
        if depth > 16 || self.is_view_type(&t) {
            return depth <= 16;
        }
        match &t {
            Ty::List(x) | Ty::Opt(x) => self.holds_calls(x, depth + 1),
            Ty::Inline(fs) => fs.iter().any(|(_, x)| self.holds_calls(x, depth + 1)),
            Ty::Def(d, _) => match &self.defs[*d].kind {
                DefKind::Record { fields } => fields.iter().any(|f| self.holds_calls(&f.ty, depth + 1)),
                DefKind::Enum { cases } => cases.iter().any(|c| c.fields.iter().any(|f| self.holds_calls(&f.ty, depth + 1))),
                _ => false,
            },
            _ => false,
        }
    }

    pub(crate) fn hole_kind(&self, t: &Ty) -> Hole {
        match self.vars.resolve(t) {
            Ty::Rich => Hole::Rich,
            t if self.is_view_type(&t) => Hole::View,
            _ => Hole::Words,
        }
    }

    /// The piece of Markdown an extern widget says it is.
    pub(crate) fn piece_of(&self, d: DefId) -> Option<String> {
        let ast::DeclKind::ExternWidget(w) = self.decl_of(d) else { return None };
        let piece = w.body.iter().flat_map(|b| b.iter()).find_map(|m| match m {
            ast::WidgetMember::Markdown(m) => m.piece.as_ref().map(|p| p.name.clone()),
            _ => None,
        });
        piece
    }

    /// The same, as a value: a rich_text where a view is wanted.
    pub(crate) fn alone_value(&mut self, x: ir::Expr) -> ir::Expr {
        let mut args = BTreeMap::new();
        args.insert("value".to_string(), x.clone());
        let paragraph = ir::Expr::View(ir::ViewValue { widget: "core/paragraph".into(), args, slots: BTreeMap::new(), events: BTreeMap::new(), reads: Default::default() });
        ir::Expr::Op(Box::new(ir::ExprOp { name: ir::OpName::Pick, args: vec![ir::Expr::Sentence(Box::new(x.clone())), paragraph, x], ty: Some(ir::Type::named("core/view")), whole: Default::default() }))
    }

    /// What a block of prose gives a slot: for a rich_text slot, its one paragraph's sentence.
    pub(crate) fn prose_fill(&mut self, file: usize, d: DefId, slot: &SlotSym, nodes: Vec<ir::Node>, at: Span) -> Vec<ir::Node> {
        if !holds_rich(&slot.ir) {
            return nodes;
        }
        match nodes.as_slice() {
            [ir::Node::Extern(p)] if p.name == "core/paragraph" => match p.props.get("value") {
                Some(value) => return vec![ir::Node::Place(ir::NodePlace { sentence: true, site: p.site.clone(), view: value.clone(), reads: Default::default() })],
                None => {}
            },
            _ => {}
        }
        if !nodes.is_empty() && self.piece_of(d).as_deref().map_or(false, |p| pieces::takes_sentence(p, &slot.name)) {
            let name = self.defs[d].name.clone();
            self.error(file, codes::SENTENCE, at, format!("`{}` holds a sentence, and this is blocks: write one paragraph", name));
        }
        nodes
    }
}

/// A rich_text placed on its own: a sentence is a paragraph, and blocks are themselves.
pub(crate) fn alone(site: String, x: ir::Expr) -> Vec<ir::Node> {
    // the value once, under a name no author can write
    let name = format!("~{}", site);
    let get = || ir::Expr::Get(ir::Get::Local(name.clone()));
    let paragraph = extern_node("core/paragraph", format!("{}p", site), vec![("value", get())], BTreeMap::new());
    vec![
        ir::Node::Declare(ir::NodeDeclare { let_: ir::Local { name: name.clone(), ty: ir::Type::named("core/rich_text"), value: x }, reads: Default::default() }),
        ir::Node::If(ir::NodeIf {
            site: site.clone(),
            branches: vec![ir::NodeIfBranches { when: ir::Cond::Cond(ir::Expr::Sentence(Box::new(get()))), then: vec![paragraph], reads: Default::default() }],
            else_: vec![ir::Node::Place(ir::NodePlace { sentence: false, site: format!("{}b", site), view: get(), reads: Default::default() })],
        }),
    ]
}

/// A literal's blocks, or a paragraph's words, as calls.
pub(crate) struct Rich<'a> {
    pub holes: &'a [ir::Expr],
    pub kinds: &'a [Hole],
    next: usize,
}

impl Rich<'_> {
    pub fn new<'a>(holes: &'a [ir::Expr], kinds: &'a [Hole]) -> Rich<'a> {
        Rich { holes, kinds, next: 0 }
    }

    fn site(&mut self) -> String {
        self.next += 1;
        format!("t{}", self.next - 1)
    }

    fn call(&mut self, name: &str, props: Vec<(&str, ir::Expr)>, slots: BTreeMap<String, Vec<ir::Node>>) -> ir::Node {
        let site = self.site();
        extern_node(name, site, props, slots)
    }

    /// `core/md_read`'s blocks: exactly one paragraph is a sentence.
    pub fn literal(&mut self, blocks: &Value) -> ir::Expr {
        let blocks = items(Some(blocks));
        if let [Value::Case(c)] = blocks.as_slice() {
            if &*c.name == "PARAGRAPH" {
                return self.sentence(get(&blocks[0], "words"));
            }
        }
        if blocks.is_empty() {
            return ir::Expr::Rich(ir::Rich { blocks: false, nodes: Arc::new(Vec::new()) });
        }
        ir::Expr::Rich(ir::Rich { blocks: true, nodes: Arc::new(self.blocks(&blocks)) })
    }

    /// `core/md_inline`'s segments.
    pub fn sentence(&mut self, words: Option<&Value>) -> ir::Expr {
        let nodes = self.words(&items(words), 0);
        ir::Expr::Rich(ir::Rich { blocks: false, nodes: Arc::new(nodes) })
    }

    /// Segments with the marks from `level` on: neighbours with the same one share its call.
    fn words(&mut self, segs: &[Value], level: usize) -> Vec<ir::Node> {
        const MARKS: [&str; 4] = ["link", "bold", "italic", "strike"];
        let mut out = Vec::new();
        let Some(mark) = MARKS.get(level) else {
            for s in segs {
                out.extend(get(s, "part").and_then(|p| self.part(p)));
            }
            return out;
        };
        let of = |s: &Value| match get(s, "marks").and_then(|m| get(m, mark)) {
            Some(Value::Bool(true)) => Some(String::new()),
            Some(Value::Opt(Some(l))) => Some(l.as_text().unwrap_or("").to_string()),
            _ => None,
        };
        let mut k = 0;
        while k < segs.len() {
            let here = of(&segs[k]);
            let mut end = k + 1;
            while end < segs.len() && of(&segs[end]) == here {
                end += 1;
            }
            match here {
                None => out.extend(self.words(&segs[k..end], level + 1)),
                Some(to) => {
                    let site = self.site();
                    let mut slots = BTreeMap::new();
                    slots.insert("body".to_string(), self.words(&segs[k..end], level + 1));
                    let props = if *mark == "link" { vec![("to", ir::Expr::Text(to))] } else { Vec::new() };
                    out.push(extern_node(&format!("core/{}", mark), site, props, slots));
                }
            }
            k = end;
        }
        out
    }

    fn part(&mut self, p: &Value) -> Option<ir::Node> {
        let Value::Case(c) = p else { return None };
        let x = |n: &str| get(p, n).map(|v| put_holes(v.to_ir(), self.holes)).unwrap_or(ir::Expr::Text(String::new()));
        Some(match &*c.name {
            "TEXT" => match get(p, "value").and_then(|v| v.as_text()).and_then(hole_of) {
                Some(k) => {
                    let value = self.holes.get(k).cloned().unwrap_or(ir::Expr::Text(String::new()));
                    match self.kinds.get(k) {
                        Some(Hole::View) => ir::Node::Place(ir::NodePlace { sentence: false, site: self.site(), view: value, reads: Default::default() }),
                        Some(Hole::Rich) => ir::Node::Place(ir::NodePlace { sentence: true, site: self.site(), view: value, reads: Default::default() }),
                        _ => self.call("core/words", vec![("value", value)], BTreeMap::new()),
                    }
                }
                None => {
                    let value = x("value");
                    self.call("core/words", vec![("value", value)], BTreeMap::new())
                }
            },
            "CODE" => {
                let src = x("value");
                self.call("core/code_span", vec![("src", src)], BTreeMap::new())
            }
            "MATH" => {
                let tex = x("tex");
                self.call("core/inline_math", vec![("tex", tex)], BTreeMap::new())
            }
            "IMAGE" => {
                let (src, alt) = (x("src"), x("alt"));
                self.call("core/inline_image", vec![("src", src), ("alt", alt)], BTreeMap::new())
            }
            "NOTE" => {
                let mark = x("mark");
                self.call("core/note_mark", vec![("mark", mark)], BTreeMap::new())
            }
            _ => self.call("core/line_break", vec![], BTreeMap::new()),
        })
    }

    fn blocks(&mut self, blocks: &[Value]) -> Vec<ir::Node> {
        let mut out = Vec::new();
        for b in blocks {
            let Value::Case(c) = b else { continue };
            let slot = |name: &str, nodes: Vec<ir::Node>| {
                let mut slots = BTreeMap::new();
                slots.insert(name.to_string(), nodes);
                slots
            };
            let field = |n: &str| get(b, n).map(|v| v.to_ir()).unwrap_or(ir::Expr::Text(String::new()));
            match &*c.name {
                "PARAGRAPH" => {
                    // a paragraph that is only a hole holding a view places that view
                    if let Some(node) = self.only_hole(get(b, "words")) {
                        out.extend(node);
                        continue;
                    }
                    let site = self.site();
                    let value = self.sentence(get(b, "words"));
                    out.push(extern_node("core/paragraph", site, vec![("value", value)], BTreeMap::new()));
                }
                "HEADING" => {
                    let site = self.site();
                    let value = self.sentence(get(b, "words"));
                    out.push(extern_node("core/heading", site, vec![("level", field("level")), ("value", value)], BTreeMap::new()));
                }
                "BULLETS" => {
                    let site = self.site();
                    let mut kids = Vec::new();
                    for it in items(get(b, "items")) {
                        let at = self.site();
                        let body = self.blocks(&items(get(&it, "body")));
                        kids.push(extern_node("core/item", at, vec![("done", get(&it, "done").map(|d| d.to_ir()).unwrap_or(ir::Expr::None(ir::Empty {})))], slot("body", body)));
                    }
                    out.push(extern_node("core/bullets", site, vec![("ordered", field("ordered")), ("start", field("start"))], slot("items", kids)));
                }
                "QUOTE" => {
                    let site = self.site();
                    let body = self.blocks(&items(get(b, "body")));
                    out.push(extern_node("core/quote", site, vec![], slot("body", body)));
                }
                "CODE" => {
                    let (src, lang) = (field("src"), field("lang"));
                    out.push(self.call("core/code", vec![("src", src), ("lang", lang)], BTreeMap::new()));
                }
                "MATH" => {
                    let tex = put_holes(field("tex"), self.holes);
                    out.push(self.call("core/math", vec![("tex", tex)], BTreeMap::new()));
                }
                "TABLE" => {
                    let site = self.site();
                    let head: Vec<ir::Expr> = items(get(b, "head")).iter().map(|c| self.sentence(Some(c))).collect();
                    let rows: Vec<ir::Expr> = items(get(b, "rows")).iter().map(|r| ir::Expr::List(items(Some(r)).iter().map(|c| self.sentence(Some(c))).collect())).collect();
                    out.push(extern_node("core/table", site, vec![("head", ir::Expr::List(head)), ("rows", ir::Expr::List(rows)), ("align", field("align"))], BTreeMap::new()));
                }
                "FOOTNOTE" => {
                    let site = self.site();
                    let body = self.blocks(&items(get(b, "body")));
                    out.push(extern_node("core/footnote", site, vec![("mark", field("mark"))], slot("body", body)));
                }
                _ => out.push(self.call("core/divider", vec![], BTreeMap::new())),
            }
        }
        out
    }

    fn only_hole(&mut self, words: Option<&Value>) -> Option<Vec<ir::Node>> {
        let segs = items(words);
        let [seg] = segs.as_slice() else { return None };
        let k = get(seg, "part").and_then(|p| get(p, "value")).and_then(|v| v.as_text()).and_then(hole_of)?;
        let x = self.holes.get(k).cloned()?;
        let site = self.site();
        match self.kinds.get(k)? {
            Hole::View => Some(vec![ir::Node::Place(ir::NodePlace { sentence: false, site, view: x, reads: Default::default() })]),
            Hole::Rich => Some(alone(site, x)),
            Hole::Words => None,
        }
    }
}

fn get<'v>(v: &'v Value, field: &str) -> Option<&'v Value> {
    match v {
        Value::Record(r) => r.get(field),
        Value::Case(c) => c.values.iter().find(|(k, _)| &**k == field).map(|(_, x)| x),
        _ => None,
    }
}

fn items(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::List(l)) => l.items.iter().map(|i| i.value.clone()).collect(),
        _ => Vec::new(),
    }
}
