//! The values a migration writes into a use (page 6): worked out where the use gives constants, by the
//! compiler's own evaluator, and otherwise the migration's code with the use's values in it. Either
//! way the compiler makes the resolved IR of what is written: a probe document, in the newer version's
//! world, holds the value as a `let`, and each value the use gave stands in as a call of an extern fn
//! of its type, which is then replaced by that value's own resolved IR. Nothing is put together as
//! text, so a value is never read in another order than it was written.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::check::{compile_on, compile_rir, Checker, Source};
use crate::eval::interp::{FailKind, Failure, Host, Machine, Outer, Program};
use crate::eval::value::Case;
use crate::eval::Value as Val;
use crate::formats::ir;
use crate::syntax::ast::{self, Expr, ExprKind};
use crate::syntax::printer::print_expr;

/// The name of the probe's own declarations, which no file of the tree is let to have.
pub const PROBE: &str = "xtmd_probe";

pub struct Probe<'a> {
    base: &'a Checker,
    /// The build as the newer version has it.
    world: Vec<Source>,
    /// The probe's lines before its value: the pack imported whole, the migration's file's imports.
    head: String,
    /// The migration's own fns and `let`s, as written, for working a value out.
    own: String,
    /// Their names, which a use can't be given.
    pub private: BTreeSet<String>,
    /// The migration's own fns, by name.
    pub fns: BTreeMap<String, ast::FnDecl>,
    /// The newer version's declarations, for the fields a record needn't write.
    new: Vec<Source>,
    made: RefCell<BTreeMap<String, Result<Value, String>>>,
}

/// Constants alone: a `let` the probe declares, and nothing a page supplies.
struct Closed;

impl Host for Closed {
    fn read(&self, m: &mut Machine, what: &Outer, _owner: Option<u64>) -> Result<Val, Failure> {
        match what {
            Outer::Let(n) if m.prog.lets.contains_key(&**n) => Ok(m.prog.lets[&**n].clone()),
            _ => Err(unknown()),
        }
    }

    fn write(&self, _m: &mut Machine, _what: &Outer, _owner: Option<u64>, _value: Val) -> Result<(), Failure> {
        Err(unknown())
    }

    fn call_extern(&self, _m: &mut Machine, _name: &str, _args: &[Val]) -> Result<Option<Option<Val>>, Failure> {
        Err(unknown())
    }

    fn token_value(&self, _m: &mut Machine, _case: &Case) -> Result<Val, Failure> {
        Err(unknown())
    }
}

fn unknown() -> Failure {
    Failure::new(FailKind::NotConstant, "only known when the page runs".to_string())
}

impl<'a> Probe<'a> {
    /// For a migration of `pack` written in `step_text`, into the build `world` and the pack's files
    /// `new` at the version it moves to.
    pub fn new(base: &'a Checker, world: Vec<Source>, pack: &str, step: &ast::MigrationDecl, step_text: &str, new: Vec<Source>) -> Probe<'a> {
        let mut head = format!("import {};\n", pack);
        let (mut own, mut private, mut fns) = (String::new(), BTreeSet::new(), BTreeMap::new());
        if let Some(f) = crate::syntax::parse(step_text, crate::syntax::Entry::Xtmd).file {
            for n in &f.items {
                match &n.item {
                    ast::Item::Import(i) if i.path[0].name != pack => head.push_str(&format!("{};\n", step_text[i.span.range()].trim_end_matches(';').trim_end().replace('\n', " "))),
                    ast::Item::Decl(d) => match &d.kind {
                        ast::DeclKind::Fn(x) if x.sig.on.is_none() => {
                            private.insert(x.sig.name.name.clone());
                            fns.insert(x.sig.name.name.clone(), x.clone());
                            own.push_str(&format!("{}\n\n", &step_text[d.span.range()]));
                        }
                        ast::DeclKind::Let(v) => {
                            private.insert(v.name.name.clone());
                            own.push_str(&format!("{}\n\n", &step_text[d.span.range()]));
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        for item in step.items.iter() {
            let ast::MigrationItem::Fn(x) = item else { continue };
            let Some(b) = &x.body else { continue };
            private.insert(x.sig.name.name.clone());
            fns.insert(x.sig.name.name.clone(), x.clone());
            own.push_str(&format!("{}\n\n", &step_text[x.sig.span.start as usize..b.span.end as usize]));
        }
        Probe { base, world, head, own, private, fns, new, made: RefCell::new(BTreeMap::new()) }
    }

    fn doc(&self, decls: &str, ty: &str, expr: &str) -> Source {
        Source { path: format!("{}.xtmd", PROBE), text: format!("{}\n{}{}\nlet {}: {} = {};\n@@@\n", self.head, self.own, decls, PROBE, ty, expr) }
    }

    /// As `doc`, the value returned by a fn: it may read what a constant can't, such as the values a
    /// use gives standing in as calls.
    fn fn_doc(&self, decls: &str, ty: &str, expr: &str) -> Source {
        Source { path: format!("{}.xtmd", PROBE), text: format!("{}\n{}{}\nfn {}(): {} {{\n    return {};\n}}\n@@@\n", self.head, self.own, decls, PROBE, ty, expr) }
    }

    /// The resolved IR of `expr`, a value of type `ty`, with `decls` declared before it.
    pub fn rir(&self, decls: &str, ty: &str, expr: &str) -> Result<Value, String> {
        let key = format!("{}\u{0}{}\u{0}{}", decls, ty, expr);
        if let Some(v) = self.made.borrow().get(&key) {
            return v.clone();
        }
        let doc = self.fn_doc(decls, ty, expr);
        let (c, r) = compile_rir(self.base, &[self.world.clone()], &[doc.clone()], "doc");
        let out = match r.filter(|_| c.errors().next().is_none()) {
            None => Err(c.errors().filter(|d| d.file == doc.path).map(|d| d.diagnostic.message.clone()).next().unwrap_or_else(|| "it doesn't compile in the newer version".into())),
            Some(r) => {
                let file = r["files"].as_array().into_iter().flatten().find(|f| f["path"] == doc.path.as_str()).cloned().unwrap_or_default();
                let id = format!("doc/{}", PROBE);
                file["declarations"].as_array().into_iter().flatten().find(|d| d["fn"]["id"] == id.as_str()).and_then(|d| d.pointer("/fn/body/0/return/value").cloned()).ok_or_else(|| "the probe's value is missing".to_string())
            }
        };
        self.made.borrow_mut().insert(key, out.clone());
        out
    }

    /// What `expr` comes to, when it is constant.
    pub fn eval(&self, decls: &str, ty: &str, expr: &str) -> Option<Val> {
        let doc = self.doc(decls, ty, expr);
        let c = compile_on(self.base, &[self.world.clone()], &[doc], "doc");
        let m = c.module?;
        let prog = Program::from_module(&m);
        let mut prog = prog;
        let mut left: Vec<&ir::LetDecl> = m.declarations.iter().filter_map(|d| if let ir::Decl::Let(l) = &d.declare { Some(l) } else { None }).collect();
        loop {
            let before = left.len();
            left.retain(|l| {
                let v = Machine::new(&prog, Box::new(Closed)).eval(&l.value);
                match v {
                    Ok(v) => {
                        prog.lets.insert(l.name.clone(), v);
                        false
                    }
                    Err(_) => true,
                }
            });
            if left.len() == before || left.is_empty() {
                break;
            }
        }
        prog.lets.get(&format!("doc/{}", PROBE)).cloned()
    }

    /// A value as Extended Mark writes it; `None` for one with no literal of its own.
    pub fn written(&self, v: &Val) -> Option<String> {
        Some(match v {
            Val::Num(n) => n.show(),
            Val::Text(t) => quoted(t),
            Val::Bool(b) => b.to_string(),
            Val::List(l) => format!("[{}]", l.items.iter().map(|i| self.written(&i.value)).collect::<Option<Vec<_>>>()?.join(", ")),
            Val::Case(c) => {
                let ty = c.ty.rsplit('/').next()?;
                let vals = c.values.iter().map(|(_, v)| self.written(v)).collect::<Option<Vec<_>>>()?;
                if vals.is_empty() {
                    format!("{}.{}", ty, c.name)
                } else {
                    format!("{}.{}({})", ty, c.name, vals.join(", "))
                }
            }
            Val::Opt(None) => "opt.NONE".into(),
            Val::Opt(Some(v)) => self.written(v)?,
            Val::Record(r) => {
                // a date, and the others that are made from a literal, are written as their own
                let (pack, ty) = r.ty.rsplit_once('/').unwrap_or(("", &r.ty));
                if pack == "core" && matches!(ty, "date" | "moment" | "template" | "map" | "set") {
                    return None;
                }
                let defaults: BTreeMap<String, String> = crate::migrate::packs::api::decl(&self.new, ty).map(|d| d.inputs.into_iter().filter_map(|m| Some((m.name, m.default?))).collect()).unwrap_or_default();
                let fields = r.fields.iter().map(|(n, v)| Some((n.to_string(), self.written(v)?))).collect::<Option<Vec<_>>>()?;
                let fields: Vec<String> = fields.into_iter().filter(|(n, v)| defaults.get(n) != Some(v)).map(|(n, v)| format!("{} = {}", n, v)).collect();
                let name = if pack.is_empty() { String::new() } else { format!("{} ", ty) };
                if fields.is_empty() {
                    format!("{}{{}}", name)
                } else {
                    format!("{}{{ {} }}", name, fields.join(", "))
                }
            }
            Val::Duration(_) | Val::Range(_) | Val::Fn(_) | Val::View(_) | Val::Content(_) => return None,
        })
    }

    /// The type of a declaration's member, as written in the newer version, as its resolved IR.
    pub fn rir_type(&self, ty: &str) -> Option<Value> {
        let doc = Source { path: format!("{}.xtmd", PROBE), text: format!("{}\nfn {}(x: {}) {{\n}}\n@@@\n", self.head, PROBE, ty) };
        let (_, r) = compile_rir(self.base, &[self.world.clone()], &[doc.clone()], "doc");
        let r = r?;
        let file = r["files"].as_array()?.iter().find(|f| f["path"] == doc.path.as_str())?.clone();
        file["declarations"].as_array()?.iter().find_map(|d| d["fn"]["params"].get(0).map(|p| p["type"].clone()))
    }
}

/// A type of the resolved IR as the probe writes it: core's and the pack's by their names, which the
/// probe imports; `None` for a type the probe can't name, such as one a document declares.
pub fn type_text(t: &Value, pack: &str) -> Option<String> {
    let name = t["name"].as_str()?;
    let (of, short) = name.split_once('/')?;
    if of != "core" && of != pack {
        return None;
    }
    let args: Vec<String> = t["args"].as_array().into_iter().flatten().map(|a| type_text(a, pack)).collect::<Option<_>>()?;
    Some(if args.is_empty() { short.to_string() } else { format!("{}<{}>", short, args.join(", ")) })
}

/// An expression of the resolved IR printed as source, by decompile, with a type to hold it.
pub fn printed(e: &Value, ty: &Value) -> Option<String> {
    let file = json!({
        "path": "printed.xtmd",
        "kind": "document",
        "imports": [],
        "prose": [],
        "declarations": [{ "let": { "id": format!("doc/{}", PROBE), "name": PROBE, "type": ty, "value": e, "span": [0, 0] } }],
    });
    let p = crate::decompile::file(&file).ok()?;
    let start = p.text.find(" = ")? + 3;
    let end = p.text.rfind(";\n")?;
    Some(p.text[start..end].to_string())
}

/// Whether an expression of the resolved IR is made of literals alone.
pub fn constant(e: &Value) -> bool {
    match e {
        Value::Object(o) => o.iter().all(|(k, v)| match k.as_str() {
            "get" | "call" | "apply" | "lambda" | "field" | "index" | "use" | "template" => false,
            "type" | "span" | "comments" => true,
            _ => constant(v),
        }),
        Value::Array(a) => a.iter().all(constant),
        _ => true,
    }
}

/// Each probe stand-in, `doc/<PROBE>_<n>()`, replaced by the value it stands for.
pub fn fill(e: &mut Value, given: &[Value]) {
    if let Some(n) = e["call"]["fn"].as_str().and_then(|f| f.strip_prefix(&format!("doc/{}_", PROBE))).and_then(|n| n.parse::<usize>().ok()) {
        if let Some(v) = given.get(n) {
            *e = v.clone();
            return;
        }
    }
    match e {
        Value::Object(o) => o.values_mut().for_each(|v| fill(v, given)),
        Value::Array(a) => a.iter_mut().for_each(|v| fill(v, given)),
        _ => {}
    }
}

/// Every name an expression reads, a `fn`'s name it calls included.
pub fn names_in(e: &Expr) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut e = e.clone();
    crate::migrate::packs::ops::walk_names(&mut e, &mut |n| {
        out.insert(n.to_string());
        None
    });
    calls_in(&e, &mut out);
    out
}

fn calls_in(e: &Expr, out: &mut BTreeSet<String>) {
    match &e.kind {
        ExprKind::Call { callee, args } => {
            if let ExprKind::Name(n) = &callee.kind {
                out.insert(n.name.clone());
            }
            calls_in(callee, out);
            args.iter().for_each(|a| calls_in(&a.value, out));
        }
        ExprKind::Field { target, .. } | ExprKind::Paren(target) | ExprKind::Try(target) | ExprKind::Unary { operand: target, .. } => calls_in(target, out),
        ExprKind::Binary { left, right, .. } => {
            calls_in(left, out);
            calls_in(right, out);
        }
        ExprKind::Ternary { cond, then, otherwise } => {
            calls_in(cond, out);
            calls_in(then, out);
            calls_in(otherwise, out);
        }
        ExprKind::List(xs) => xs.iter().for_each(|x| calls_in(x, out)),
        ExprKind::Record { inits, .. } => inits.iter().for_each(|i| calls_in(&i.value, out)),
        _ => {}
    }
}

/// A text's words, quoted as Extended Mark writes a text.
pub fn quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:X}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// An expression printed, for a probe.
pub fn source(e: &Expr) -> String {
    print_expr(e)
}
