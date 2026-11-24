//! Markdown out (page 1, plan tab 13): the page written as Markdown with the values it has now,
//! in one spelling for each block and mark. A port of @extended-mark/runtime's `markdown.ts`, reading the
//! live page: `$state`, the page's values and each widget's own variables as they are. The
//! blocks are written by xtmd-markdown, as the compiler writes a page's starting Markdown.

use std::collections::BTreeMap;
use std::rc::Rc;

use xtmd_eval::interp::R;
use xtmd_eval::value::ContentOf;
use xtmd_eval::{Binding, FailKind, Failure, Machine, Value};
use xtmd_ir::ir::{Expr, Node};
use xtmd_markdown::{join_runs, Align, Block, Inline, Item};

use crate::page::{check_sentence, join, Args, Env, Rt};

enum Piece {
    Block(Block),
    /// A part of a sentence.
    Run(Inline),
    Item(Item),
}

fn field<'a>(v: Option<&'a Value>, name: &str) -> Option<&'a Value> {
    match v? {
        Value::Record(r) => r.get(name),
        Value::Case(c) => c.values.iter().find(|(k, _)| &**k == name).map(|(_, x)| x),
        _ => None,
    }
}

fn text_of(v: Option<&Value>) -> String {
    match v {
        Some(Value::Text(t)) => t.to_string(),
        Some(Value::Num(n)) => n.show(),
        Some(Value::Opt(Some(x))) => text_of(Some(x)),
        _ => String::new(),
    }
}

fn items_of(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::List(l)) => l.items.iter().map(|i| i.value.clone()).collect(),
        _ => Vec::new(),
    }
}

/// A number as JavaScript's `Number()` reads the text it shows: how the web writes a heading's
/// level and a list's start.
fn number(v: Option<&Value>) -> Option<f64> {
    match v {
        Some(Value::Num(n)) => n.show().parse::<f64>().ok(),
        _ => None,
    }
}

/// A template's text: its pieces with each hole's shown text between them.
fn template_text(v: Option<&Value>) -> String {
    let pieces = items_of(field(v, "pieces"));
    let holes = items_of(field(v, "holes"));
    let mut out = String::new();
    for (k, p) in pieces.iter().enumerate() {
        out.push_str(&text_of(Some(p)));
        if let Some(h) = holes.get(k) {
            out.push_str(&text_of(field(Some(h), "value")));
        }
    }
    out
}

/// List items that follow one another make one list, across a loop's turns too (page 3).
fn blocks_of(pieces: Vec<Piece>) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut in_sentence = false;
    for p in pieces {
        let b = match p {
            Piece::Block(b) => b,
            // a sentence among blocks is a paragraph
            Piece::Run(r) => {
                if let (Some(Block::Paragraph(runs)), true) = (out.last_mut(), in_sentence) {
                    runs.push(r);
                } else {
                    out.push(Block::Paragraph(vec![r]));
                }
                in_sentence = true;
                continue;
            }
            Piece::Item(i) => Block::Bullets { ordered: false, start: 1, items: vec![i] },
        };
        in_sentence = false;
        if let (Some(Block::Bullets { ordered: o1, items: i1, .. }), Block::Bullets { ordered: o2, items: i2, .. }) = (out.last_mut(), &b) {
            if *o1 == *o2 {
                i1.extend(i2.iter().cloned());
                continue;
            }
        }
        out.push(b);
    }
    out
}

/// A block's words, for a view placed in a sentence.
fn plain(b: &Block) -> String {
    let runs = |r: &[Inline]| {
        r.iter()
            .map(|i| match i {
                Inline::Text { value, .. } | Inline::Code(value) | Inline::Math(value) => value.clone(),
                Inline::Image { alt, .. } => alt.clone(),
                _ => String::new(),
            })
            .collect::<String>()
    };
    match b {
        Block::Paragraph(r) | Block::Heading { inline: r, .. } => runs(r),
        Block::Quote(bs) => bs.iter().map(plain).collect::<Vec<_>>().join(" "),
        _ => String::new(),
    }
}

struct Writer<'r> {
    rt: &'r Rt,
    depth: u32,
}

impl Writer<'_> {
    fn nodes(&mut self, m: &mut Machine, nodes: &[Node], env0: &Env) -> R<Vec<Piece>> {
        let mut env = env0.clone();
        let mut out = Vec::new();
        for n in nodes {
            match n {
                Node::Declare(d) => {
                    let v = self.rt.eval_in(m, &env, &d.let_.value)?;
                    env.bindings.push((Rc::from(d.let_.name.as_str()), Binding::Value(v)));
                }
                Node::For(f) => {
                    // rows by position for paths, and by item for identity, as drawing walks them
                    for (k, row) in self.rt.keyed_rows(m, f, &env)?.into_iter().enumerate() {
                        let mut bindings = env.bindings.clone();
                        bindings.extend(row.bindings);
                        let key = env.key.clone().unwrap_or_else(|| env.path.clone());
                        let inner = Env { owner: env.owner, bindings, path: join(&env.path, &format!("{}[{}]", f.site, k)), key: Some(join(&key, &format!("{}[{}]", f.site, row.key))), keep: env.keep.clone() };
                        out.extend(self.nodes(m, &f.body, &inner)?);
                    }
                }
                Node::If(i) => {
                    let (nodes, inner) = self.rt.pick_branch(m, i, &env)?;
                    out.extend(self.nodes(m, nodes, &inner)?);
                }
                Node::Match(mt) => {
                    let v = self.rt.eval_in(m, &env, &mt.value)?;
                    let mut done = false;
                    for c in &mt.cases {
                        if let Some(binds) = self.rt.matches(m, &v, &c.when)? {
                            let mut inner = env.clone();
                            inner.bindings.extend(binds);
                            out.extend(self.nodes(m, &c.then, &inner)?);
                            done = true;
                            break;
                        }
                    }
                    if !done {
                        out.extend(self.nodes(m, &mt.else_, &env)?);
                    }
                }
                Node::Place(p) => {
                    let v = self.rt.eval_in(m, &env, &p.view)?;
                    if p.sentence {
                        check_sentence(&v)?;
                    }
                    let key = join(env.key.as_deref().unwrap_or(&env.path), &p.site);
                    out.extend(self.view(m, &v, &join(&env.path, &p.site), &key)?);
                }
                Node::Extern(x) => {
                    let mut props = BTreeMap::new();
                    for (k, e) in &x.props {
                        props.insert(k.clone(), self.rt.eval_in(m, &env, e)?);
                    }
                    for (k, p) in &x.writes {
                        props.insert(k.clone(), m.eval_with(&env.bindings, env.owner, &Expr::Place(p.clone()))?);
                    }
                    let path = join(&env.path, &x.site);
                    let key = join(env.key.as_deref().unwrap_or(&env.path), &x.site);
                    out.extend(self.widget(m, &x.name, &path, props, Args::Exprs(&BTreeMap::new()), &x.slots, &env, &key, &BTreeMap::new())?);
                }
                Node::Use(u) => {
                    let key = join(env.key.as_deref().unwrap_or(&env.path), &u.site);
                    out.extend(self.widget(m, &u.name, &join(&env.path, &u.site), BTreeMap::new(), Args::Exprs(&u.args), &u.slots, &env, &key, &u.events)?);
                }
            }
        }
        Ok(out)
    }

    fn view(&mut self, m: &mut Machine, v: &Value, path: &str, key: &str) -> R<Vec<Piece>> {
        match v {
            Value::Opt(Some(x)) => self.view(m, x, path, key),
            Value::List(l) => {
                let mut out = Vec::new();
                for (k, i) in l.items.iter().enumerate() {
                    out.extend(self.view(m, &i.value, &format!("{}[{}]", path, k), &format!("{}[{}]", key, i.id))?);
                }
                Ok(out)
            }
            Value::Content(c) => match &c.of {
                ContentOf::Nodes { nodes, captured, owner, at } => {
                    let (path, key) = match at {
                        Some((p, k)) => (p.to_string(), k.to_string()),
                        None => (path.to_string(), key.to_string()),
                    };
                    self.nodes(m, nodes, &Env { owner: *owner, bindings: captured.clone(), path, key: Some(key), keep: None })
                }
                ContentOf::Parts(parts) => {
                    let mut out = Vec::new();
                    for (k, p) in parts.iter().enumerate() {
                        let site = format!("j{}", k);
                        out.extend(self.view(m, p, &join(path, &site), &join(key, &site))?);
                    }
                    Ok(out)
                }
                ContentOf::Drawn(_) => Ok(Vec::new()),
            },
            Value::View(view) => {
                let who = view.at.as_deref().unwrap_or(key);
                let made = Env { owner: view.owner, bindings: view.captured.clone(), path: path.to_string(), key: Some(who.to_string()), keep: None };
                self.widget(m, &view.widget, path, view.args.clone(), Args::Values(&view.args), &view.slots, &made, who, &view.events)
            }
            _ => Ok(Vec::new()),
        }
    }

    /// A sentence's calls as runs.
    fn sentence(&mut self, m: &mut Machine, v: Option<&Value>, path: &str, key: &str) -> R<Vec<Inline>> {
        let Some(v) = v else { return Ok(Vec::new()) };
        check_sentence(v)?;
        Ok(join_runs(runs_of(self.view(m, v, path, key)?)))
    }

    fn slot(&mut self, m: &mut Machine, slots: &BTreeMap<String, Vec<Node>>, name: &str, env: &Env) -> R<Vec<Piece>> {
        match slots.get(name) {
            Some(nodes) => self.nodes(m, nodes, env),
            None => Ok(Vec::new()),
        }
    }

    /// A widget: one that is a piece of Markdown is written as it; any other writes its
    /// markdown. `key` and `events` are the call's, as drawing gives them: entering a widget to
    /// write its Markdown finds the instance the page drew.
    #[allow(clippy::too_many_arguments)]
    fn widget(&mut self, m: &mut Machine, name: &str, path: &str, props: BTreeMap<String, Value>, args: Args, slots: &BTreeMap<String, Vec<Node>>, env: &Env, key: &str, events: &BTreeMap<String, String>) -> R<Vec<Piece>> {
        self.depth += 1;
        if self.depth > 64 {
            self.depth -= 1;
            return Err(Failure::new(FailKind::Limit(xtmd_eval::interp::Limit::Depth), format!("{} places itself more than 64 deep", name)));
        }
        let out = self.widget_inner(m, name, path, props, args, slots, env, key, events);
        self.depth -= 1;
        match out {
            Ok(out) => Ok(out),
            Err(f) => {
                if self.rt.s.extern_widgets.get(name).map_or(false, |w| w.piece.is_some()) {
                    return Err(f);
                }
                // a widget that fails is a notice in its place, a quote in Markdown (page 6)
                let short = name.rsplit('/').next().unwrap_or(name);
                let notice = Inline::Text { value: format!("{} failed: {}", short, f.message), bold: false, italic: false, strike: false, link: None };
                Ok(vec![Piece::Block(Block::Quote(vec![Block::Paragraph(vec![notice])]))])
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn widget_inner(&mut self, m: &mut Machine, name: &str, path: &str, props: BTreeMap<String, Value>, args: Args, slots: &BTreeMap<String, Vec<Node>>, env: &Env, key: &str, events: &BTreeMap<String, String>) -> R<Vec<Piece>> {
        let p = |k: &str| props.get(k);
        let block = |b: Block| Ok(vec![Piece::Block(b)]);
        let piece = self.rt.s.extern_widgets.get(name).and_then(|w| w.piece.clone());
        match piece.as_deref().unwrap_or("") {
            "paragraph" => {
                let runs = self.sentence(m, p("value"), &join(path, "value"), &join(key, "value"))?;
                return if runs.is_empty() { Ok(Vec::new()) } else { block(Block::Paragraph(runs)) };
            }
            "heading" => {
                let level = number(p("level")).map_or(1.0, |l| l.clamp(1.0, 6.0)).floor() as u8;
                let inline = self.sentence(m, p("value"), &join(path, "value"), &join(key, "value"))?;
                return block(Block::Heading { level, inline });
            }
            "bullets" => {
                let ordered = matches!(p("ordered"), Some(Value::Bool(true)));
                let start = number(p("start")).map_or(1, |s| s as i64);
                let items: Vec<Item> = self
                    .slot(m, slots, "items", env)?
                    .into_iter()
                    .map(|x| match x {
                        Piece::Item(i) => i,
                        other => Item { done: None, blocks: blocks_of(vec![other]) },
                    })
                    .collect();
                return if items.is_empty() { Ok(Vec::new()) } else { block(Block::Bullets { ordered, start, items }) };
            }
            "item" => {
                let done = match p("done") {
                    Some(Value::Opt(Some(d))) => d.as_bool(),
                    Some(Value::Bool(b)) => Some(*b),
                    _ => None,
                };
                let blocks = blocks_of(self.slot(m, slots, "body", env)?);
                return Ok(vec![Piece::Item(Item { done, blocks })]);
            }
            "quote" => {
                let blocks = blocks_of(self.slot(m, slots, "body", env)?);
                return block(Block::Quote(blocks));
            }
            "code" => return block(Block::Code { lang: text_of(p("lang")), src: text_of(p("src")) }),
            "math" => return block(Block::Math(template_text(field(p("tex"), "tex")))),
            "divider" => return block(Block::Divider),
            "text" => return Ok(vec![Piece::Run(Inline::words(text_of(p("value"))))]),
            "bold" | "italic" | "strike" | "link" => {
                let body = runs_of(self.slot(m, slots, "body", env)?);
                let to = text_of(p("to"));
                let marked = body.into_iter().map(|r| match r {
                    Inline::Text { value, mut bold, mut italic, mut strike, mut link } => {
                        match piece.as_deref() {
                            Some("bold") => bold = true,
                            Some("italic") => italic = true,
                            Some("strike") => strike = true,
                            _ => link = Some(to.clone()),
                        }
                        Inline::Text { value, bold, italic, strike, link }
                    }
                    other => other,
                });
                return Ok(marked.map(Piece::Run).collect());
            }
            "code_span" => return Ok(vec![Piece::Run(Inline::Code(text_of(p("src"))))]),
            "inline_math" => return Ok(vec![Piece::Run(Inline::Math(template_text(field(p("tex"), "tex"))))]),
            "inline_image" => return Ok(vec![Piece::Run(Inline::Image { src: text_of(p("src")), alt: text_of(p("alt")) })]),
            "note_mark" => return Ok(vec![Piece::Run(Inline::Note(text_of(p("mark"))))]),
            "line_break" => return Ok(vec![Piece::Run(Inline::Break)]),
            "footnote" => {
                let blocks = blocks_of(self.slot(m, slots, "body", env)?);
                return block(Block::Footnote { mark: text_of(p("mark")), blocks });
            }
            "table" => {
                let mut head = Vec::new();
                for (k, c) in items_of(p("head")).iter().enumerate() {
                    head.push(self.sentence(m, Some(c), &format!("{}[{}]", join(path, "head"), k), &format!("{}[{}]", join(key, "head"), k))?);
                }
                let mut rows = Vec::new();
                for (j, r) in items_of(p("rows")).iter().enumerate() {
                    let mut row = Vec::new();
                    for (k, c) in items_of(Some(r)).iter().enumerate() {
                        row.push(self.sentence(m, Some(c), &format!("{}[{}][{}]", join(path, "rows"), j, k), &format!("{}[{}][{}]", join(key, "rows"), j, k))?);
                    }
                    rows.push(row);
                }
                let align = items_of(p("align"))
                    .iter()
                    .map(|a| match a {
                        Value::Case(c) if &*c.name == "CENTER" => Align::Center,
                        Value::Case(c) if &*c.name == "END" => Align::End,
                        _ => Align::Start,
                    })
                    .collect();
                return block(Block::Table { head, rows, align });
            }
            _ => {}
        }
        if let Some(t) = self.rt.s.templates.get(name).cloned() {
            // a widget written in Extended Mark: its markdown, in its instance
            let own = self.rt.enter_use(m, &t, path, args, slots, events, env, key)?;
            return self.nodes(m, t.markdown(), &own);
        }
        if let Some(w) = self.rt.s.extern_widgets.get(name).cloned() {
            let own = self.rt.enter_extern(m, &w, path, &props, slots, env)?;
            let out = self.nodes(m, &w.markdown, &own);
            self.rt.leave(&own);
            return out;
        }
        Err(Failure::ir(format!("there is no widget {} to write", name)))
    }
}

/// What some calls write in a sentence: their runs, and the words of any blocks among them.
fn runs_of(pieces: Vec<Piece>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    let mut blocks: Vec<Piece> = Vec::new();
    let flush = |blocks: &mut Vec<Piece>, out: &mut Vec<Inline>| {
        let mut bs = blocks_of(std::mem::take(blocks));
        match bs.as_mut_slice() {
            [] => {}
            // a widget that is one paragraph writes its sentence
            [Block::Paragraph(runs)] => out.append(runs),
            bs => {
                let words: Vec<String> = bs.iter().map(plain).filter(|s| !s.is_empty()).collect();
                out.push(Inline::words(words.join(" ")));
            }
        }
    };
    for p in pieces {
        match p {
            Piece::Run(r) => {
                flush(&mut blocks, &mut out);
                out.push(r);
            }
            other => blocks.push(other),
        }
    }
    flush(&mut blocks, &mut out);
    out
}

/// The page's blocks, with the values it has now.
pub(crate) fn page_blocks(rt: &Rt, m: &mut Machine) -> R<Vec<Block>> {
    let Some(doc) = rt.s.templates.get("doc").cloned() else { return Ok(Vec::new()) };
    let mut w = Writer { rt, depth: 0 };
    Ok(blocks_of(w.nodes(m, doc.markdown(), &Env::default())?))
}
