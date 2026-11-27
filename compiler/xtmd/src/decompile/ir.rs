//! A compact IR (schemas/ir.schema.json) in the resolved IR's shape, for the printer to write back: what
//! `xtmd decompile page.ir.json` prints.
//!
//! The compact IR is what a runtime runs, so it has lost what only a reader needs: comments, how the
//! Markdown was spelled, the declarations the page never reaches, which names were imported as what,
//! the source's order of a call's arguments. What comes back compiles to the same IR; each kind of
//! loss this file met is said once, as a warning.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The resolved IR of one file made from an IR, and what was lost on the way.
pub struct FromIr {
    pub rir: Value,
    pub losses: Vec<String>,
}

/// `ir` is `{ "ir": … }` as `xtmd compile` writes it; `path` is where the file it was made from is to be written.
pub fn from_ir(ir: &Value, path: &str) -> Result<FromIr, String> {
    let m = &ir["ir"];
    let Some(module) = m["module"].as_str() else {
        return Err("it isn't an IR: `xtmd compile` writes { \"ir\": { \"module\": … } }".into());
    };
    let mut c = Converter::new(m, module);
    let file = c.file(path);
    if !c.errors.is_empty() {
        return Err(c.errors.join("\n"));
    }
    Ok(FromIr { rir: json!({ "rir": crate::decompile::RIR_SHAPE, "files": [file] }), losses: c.losses.into_iter().collect() })
}

struct Converter<'a> {
    ir: &'a Value,
    module: &'a str,
    /// Every declaration, by its full name.
    decls: BTreeMap<String, (&'a str, &'a Value)>,
    /// Their names in the IR's order, which is the order they were written.
    order: Vec<String>,
    templates: BTreeMap<String, &'a Value>,
    handlers: BTreeMap<String, &'a Value>,
    /// The packs the file's own declarations name, other than core and its own.
    packs: BTreeSet<String>,
    losses: BTreeSet<String>,
    errors: Vec<String>,
    /// The locals a worked-out call left in a rich text, by name: what a hole that reads one shows.
    held: BTreeMap<String, Value>,
}

const COMMENTS: &str = "comments: an IR keeps none";
const MARKDOWN: &str = "the Markdown's spelling: the prose is written again from its blocks, in one spelling of each";
const UNREACHED: &str = "what the page doesn't reach: an IR keeps only the declarations it runs";
const ORDER: &str = "the order of a call's arguments: they are written in the order the widget declares them";
const TOKENS: &str = "the names of tokens blocks: each token's values are written in a block named after it";
const FOLDED: &str = "a call worked out when the page was compiled: its value is written, not the call";
const IMPORTS: &str = "imports' other names: every name is written as its pack declares it";

impl<'a> Converter<'a> {
    fn new(ir: &'a Value, module: &'a str) -> Self {
        let mut decls = BTreeMap::new();
        let mut order = Vec::new();
        for d in ir["declarations"].as_array().into_iter().flatten() {
            let Some((kind, v)) = d.as_object().and_then(|o| o.iter().next()) else { continue };
            // `declare: { extern: { widget: {...} } }` and `declare: { record: {...} }`
            let (kind, v) = match (kind.as_str(), v) {
                ("declare", v) => match v.as_object().and_then(|o| o.iter().next()) {
                    Some((k, x)) if k == "extern" => match x.as_object().and_then(|o| o.iter().next()) {
                        Some((k2, y)) => (extern_kind(k2), y),
                        None => continue,
                    },
                    Some((k, x)) => (decl_kind(k), x),
                    None => continue,
                },
                _ => continue,
            };
            let name = v["name"].as_str().unwrap_or(if kind == "state" { "$state" } else { "" }).to_string();
            let key = if kind == "state" { "$state".to_string() } else { name };
            order.push(key.clone());
            decls.insert(key, (kind, v));
        }
        let templates = ir["templates"].as_array().into_iter().flatten().filter_map(|t| Some((t["name"].as_str()?.to_string(), t))).collect();
        let handlers = ir["handlers"].as_array().into_iter().flatten().filter_map(|h| Some((h["id"].as_str()?.to_string(), h))).collect();
        let mut losses = BTreeSet::new();
        losses.insert(COMMENTS.to_string());
        losses.insert(UNREACHED.to_string());
        Converter { ir, module, decls, order, templates, handlers, packs: BTreeSet::new(), losses, errors: Vec::new(), held: BTreeMap::new() }
    }

    fn own(&self, name: &str) -> bool {
        name.split('/').next() == Some(self.module) && name.contains('/')
    }

    fn lost(&mut self, what: &str) {
        self.losses.insert(what.to_string());
    }

    /// A name from another pack than core or the file's own is that pack's, imported.
    fn named(&mut self, id: &str) -> String {
        if let Some((pack, _)) = id.split_once('/') {
            if pack != "core" && pack != self.module {
                self.packs.insert(pack.to_string());
            }
        }
        id.to_string()
    }

    // ---- the file

    fn file(&mut self, path: &str) -> Value {
        let document = self.module == "doc";
        let mut o = Map::new();
        o.insert("path".into(), json!(path));
        o.insert("kind".into(), json!(if document { "document" } else { "pack" }));
        if !document {
            o.insert("package".into(), json!(self.module));
        }
        let mut decls = Vec::new();
        let own: Vec<(String, (&str, &Value))> = self.order.iter().filter(|n| self.own(n) || *n == "$state").map(|n| (n.clone(), self.decls[n])).collect();
        for (name, (kind, v)) in own {
            match kind {
                "state" => {
                    o.insert("state".into(), self.state(v));
                }
                "expose" => {}
                _ => {
                    match self.decl(kind, &name, v) {
                        Some(Value::Array(both)) => decls.extend(both),
                        Some(d) => decls.push(d),
                        None => {}
                    }
                }
            }
        }
        for e in self.ir["declarations"].as_array().into_iter().flatten().filter_map(|d| d["declare"].get("expose")) {
            let fns: Vec<Value> = e["fns"].as_array().into_iter().flatten().map(|f| json!({ "name": f["name"], "params": self.params(&f["params"]), "body": self.stmts(&f["body"]) })).collect();
            o.entry("expose").or_insert_with(|| json!([])).as_array_mut().unwrap().push(json!({ "channel": e["channel"], "fns": fns }));
        }
        // in the IR's order, which is the order they were written: their handlers are numbered by it
        let widgets: Vec<String> = self.ir["templates"].as_array().into_iter().flatten().filter_map(|t| t["name"].as_str()).filter(|n| self.own(n)).map(String::from).collect();
        for name in widgets {
            let t = self.templates[&name];
            decls.push(json!({ "widget": self.widget(t) }));
        }
        o.insert("declarations".into(), json!(decls));
        if document {
            if let Some(page) = self.templates.get(self.module).copied() {
                let prose = self.prose(&page["view"]);
                o.insert("prose".into(), json!(prose));
            }
            if let Some(fm) = self.ir["front_matter"].as_str() {
                o.insert("front_matter".into(), json!({ "text": fm }));
            }
        }
        if !self.packs.is_empty() {
            self.lost(IMPORTS);
        }
        let imports: Vec<Value> = self.packs.iter().map(|p| json!({ "pack": p })).collect();
        o.insert("imports".into(), json!(imports));
        Value::Object(o)
    }

    // ---- declarations

    fn decl(&mut self, kind: &str, name: &str, v: &Value) -> Option<Value> {
        let mut o = Map::new();
        o.insert("id".into(), json!(name));
        if let Some(g) = v["params"].as_array().filter(|_| matches!(kind, "record" | "enum" | "interface" | "extern_type")) {
            o.insert("generics".into(), json!(g.iter().map(|p| self.generic(p)).collect::<Vec<_>>()));
        }
        let key = match kind {
            "record" => {
                o.insert("fields".into(), json!(self.fields(name, &v["fields"])));
                o.insert("requires".into(), json!(self.requires(&v["requires"])));
                "record"
            }
            "enum" => {
                o.insert("cases".into(), json!(self.cases(&v["cases"])));
                "enum"
            }
            "token" => {
                o.insert("value".into(), v["value"].clone());
                o.insert("cases".into(), json!(self.cases(&v["cases"])));
                "token"
            }
            "type" => {
                o.insert("is".into(), v["is"].clone());
                "type"
            }
            "interface" => {
                if v["widget"] == true {
                    o.insert("widget".into(), json!(true));
                }
                o.insert("fields".into(), json!(self.fields(name, &v["fields"])));
                // an interface's fns are names in an IR: their signatures are the first type's that implements it
                let fns: Vec<Value> = v["fns"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|f| self.interface_fn(name, f)).collect();
                o.insert("fns".into(), json!(fns));
                o.insert("events".into(), json!(v["events"].as_array().into_iter().flatten().map(|e| json!({ "name": e["name"], "params": self.params(&e["params"]) })).collect::<Vec<_>>()));
                "interface"
            }
            "fn" => {
                self.fn_parts(name, v, &mut o);
                o.insert("body".into(), json!(self.stmts(&v["body"])));
                "fn"
            }
            "extern_fn" => {
                self.fn_parts(name, v, &mut o);
                if let Some(b) = v["default"].as_array() {
                    o.insert("body".into(), json!(self.stmts(&Value::Array(b.clone()))));
                }
                "extern_fn"
            }
            "extern_type" => "extern_type",
            "extern_widget" => {
                let w = self.extern_widget(v);
                return Some(json!({ "extern_widget": w }));
            }
            "channel" => {
                o.insert("values".into(), json!(self.fields(name, &v["values"])));
                let fns: Vec<Value> = v["fns"].as_array().into_iter().flatten().map(|f| {
                    let mut s = Map::new();
                    s.insert("name".into(), f["name"].clone());
                    s.insert("id".into(), json!(format!("{}.{}", name, f["name"].as_str().unwrap_or(""))));
                    s.insert("params".into(), json!(self.params(&f["params"])));
                    if !f["returns"].is_null() {
                        s.insert("returns".into(), f["returns"].clone());
                    }
                    Value::Object(s)
                }).collect();
                o.insert("fns".into(), json!(fns));
                "extern"
            }
            "let" => {
                o.insert("type".into(), v["type"].clone());
                o.insert("value".into(), self.expr(&v["value"]));
                "let"
            }
            other => {
                self.errors.push(format!("a declaration of a kind an IR has that this xtmd can't write back: {}", other));
                return None;
            }
        };
        if kind == "token" {
            // its values, from the tokens block that gave them
            if let Some(values) = v["values"].as_object().filter(|m| !m.is_empty()) {
                self.lost(TOKENS);
                let lines: Vec<Value> = values.iter().map(|(case, x)| json!({ "token": name, "case": case, "value": self.expr(x) })).collect();
                let token = Value::Object(o);
                return Some(json!([{ "token": token }, { "tokens": { "id": format!("{}_values", name), "lines": lines } }]));
            }
        }
        Some(json!({ key: Value::Object(o) }))
    }

    fn generic(&self, p: &Value) -> Value {
        json!({ "name": p["name"], "bounds": p["bounds"].as_array().into_iter().flatten().map(|b| json!({ "name": b })).collect::<Vec<_>>() })
    }

    fn interface_fn(&mut self, iface: &str, f: &str) -> Value {
        // the fn as a type that implements the interface declares it, with self of the interface's own
        let found = self.decls.iter().find(|(n, (k, _))| matches!(*k, "fn" | "extern_fn") && n.ends_with(&format!(".{}", f))).map(|(_, (_, v))| *v);
        let mut s = Map::new();
        s.insert("name".into(), json!(f));
        s.insert("id".into(), json!(format!("{}.{}", iface, f)));
        match found {
            Some(v) => {
                s.insert("params".into(), json!(self.params(&v["params"])));
                if !v["returns"].is_null() {
                    s.insert("returns".into(), v["returns"].clone());
                }
            }
            None => {
                self.errors.push(format!("{}: the IR names its fn {} and no type's fn says what it takes", iface, f));
            }
        }
        Value::Object(s)
    }

    fn fn_parts(&mut self, name: &str, v: &Value, o: &mut Map<String, Value>) {
        let rest = name.split_once('/').map_or(name, |(_, r)| r);
        let pack = name.split('/').next().unwrap_or("");
        if let Some((ty, f)) = rest.rsplit_once('.') {
            o.insert("on".into(), json!(self.type_id(pack, ty)));
            o.insert("name".into(), json!(f));
        } else {
            o.insert("name".into(), json!(rest));
        }
        if let Some(g) = v["generics"].as_array().filter(|g| !g.is_empty()) {
            o.insert("generics".into(), json!(g.iter().map(|p| self.generic(p)).collect::<Vec<_>>()));
        }
        o.insert("params".into(), json!(self.params(&v["params"])));
        if !v["returns"].is_null() {
            o.insert("returns".into(), v["returns"].clone());
        }
    }

    /// A type's id as a fn on it is written: `core/list` for `list`, or the pack's own.
    fn type_id(&self, pack: &str, ty: &str) -> String {
        if self.decls.contains_key(&format!("{}/{}", pack, ty)) {
            format!("{}/{}", pack, ty)
        } else {
            format!("core/{}", ty)
        }
    }

    fn params(&self, ps: &Value) -> Vec<Value> {
        ps.as_array()
            .into_iter()
            .flatten()
            .map(|p| {
                let mut o = Map::new();
                o.insert("name".into(), p["name"].clone());
                if p["name"] == "self" {
                    o.insert("self".into(), json!(true));
                } else if !p["type"].is_null() {
                    o.insert("type".into(), p["type"].clone());
                }
                if p["mut"] == true {
                    o.insert("mutable".into(), json!(true));
                }
                Value::Object(o)
            })
            .collect()
    }

    fn fields(&mut self, owner: &str, fs: &Value) -> Vec<Value> {
        fs.as_array()
            .into_iter()
            .flatten()
            .map(|f| {
                let mut o = Map::new();
                o.insert("name".into(), f["name"].clone());
                o.insert("id".into(), json!(format!("{}.{}", owner, f["name"].as_str().unwrap_or(""))));
                o.insert("type".into(), f["type"].clone());
                if !f["default"].is_null() {
                    o.insert("default".into(), self.expr(&f["default"]));
                }
                if f["mut"] == true {
                    o.insert("mutable".into(), json!(true));
                }
                Value::Object(o)
            })
            .collect()
    }

    fn cases(&mut self, cs: &Value) -> Vec<Value> {
        cs.as_array()
            .into_iter()
            .flatten()
            .map(|c| {
                let mut o = Map::new();
                o.insert("name".into(), c["name"].clone());
                if let Some(f) = c["fields"].as_array() {
                    let fields: Vec<Value> = f.iter().map(|x| json!({ "name": x["name"], "type": x["type"] })).collect();
                    o.insert("fields".into(), json!(fields));
                }
                Value::Object(o)
            })
            .collect()
    }

    fn requires(&mut self, rs: &Value) -> Vec<Value> {
        rs.as_array()
            .into_iter()
            .flatten()
            .map(|r| {
                let mut o = Map::new();
                o.insert("message".into(), r["message"].clone());
                o.insert("body".into(), json!(self.stmts(&r["body"])));
                if r["warn"] == true {
                    o.insert("warn".into(), json!(true));
                }
                Value::Object(o)
            })
            .collect()
    }

    fn state(&mut self, s: &Value) -> Value {
        let fields: Vec<Value> = s["fields"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|f| {
                let mut o = Map::new();
                o.insert("name".into(), f["name"].clone());
                o.insert("id".into(), json!(format!("$state.{}", f["name"].as_str().unwrap_or(""))));
                o.insert("type".into(), f["type"].clone());
                if !f["default"].is_null() {
                    o.insert("default".into(), self.expr(&f["default"]));
                }
                o.insert("mutable".into(), json!(true));
                Value::Object(o)
            })
            .collect();
        json!({ "fields": fields, "requires": self.requires(&s["requires"]) })
    }

    fn widget(&mut self, t: &Value) -> Value {
        let name = t["name"].as_str().unwrap_or("");
        let mut o = Map::new();
        o.insert("id".into(), json!(name));
        if let Some(g) = t["generics"].as_array().filter(|g| !g.is_empty()) {
            o.insert("generics".into(), json!(g.iter().map(|p| self.generic(p)).collect::<Vec<_>>()));
        }
        o.insert("inputs".into(), json!(self.fields(name, &t["inputs"])));
        o.insert("slots".into(), json!(t["slots"].as_array().into_iter().flatten().map(|s| json!({ "name": s["name"], "type": s["type"] })).collect::<Vec<_>>()));
        o.insert("events".into(), json!(t["events"].as_array().into_iter().flatten().map(|e| json!({ "name": e["name"], "params": self.params(&e["params"]) })).collect::<Vec<_>>()));
        o.insert("requires".into(), json!(self.requires(&t["requires"])));
        let own: Vec<Value> = t["cells"].as_array().into_iter().flatten().map(|c| json!({ "name": c["name"], "type": c["type"], "value": self.expr(&c["default"]) })).collect();
        o.insert("own".into(), json!(own));
        let fns: Vec<Value> = t["fns"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|f| {
                let mut fo = Map::new();
                let n = f["name"].as_str().unwrap_or("");
                self.fn_parts(n, f, &mut fo);
                // a widget's own fn, `doc/counter.add`, is on no type
                fo.remove("on");
                fo.insert("body".into(), json!(self.stmts(&f["body"])));
                Value::Object(fo)
            })
            .collect();
        o.insert("fns".into(), json!(fns));
        o.insert("view".into(), json!(self.view(&t["view"])));
        if let Some(md) = t["markdown"].as_array() {
            o.insert("markdown".into(), json!(self.prose(&Value::Array(md.clone()))));
        }
        Value::Object(o)
    }

    fn extern_widget(&mut self, v: &Value) -> Value {
        let name = v["name"].as_str().unwrap_or("");
        let mut o = Map::new();
        o.insert("id".into(), json!(name));
        o.insert("inputs".into(), json!(self.fields(name, &v["inputs"])));
        o.insert("slots".into(), json!(v["slots"].as_array().into_iter().flatten().map(|s| json!({ "name": s["name"], "type": s["type"] })).collect::<Vec<_>>()));
        o.insert("events".into(), json!(v["events"].as_array().into_iter().flatten().map(|e| json!({ "name": e["name"], "params": self.params(&e["params"]) })).collect::<Vec<_>>()));
        if let Some(t) = v["tokens"].as_array() {
            o.insert("tokens".into(), json!(t));
        }
        if let Some(p) = v["piece"].as_str() {
            o.insert("piece".into(), json!(p));
        } else {
            self.lost(MARKDOWN);
            o.insert("markdown".into(), json!(self.prose(&v["markdown"])));
        }
        Value::Object(o)
    }

    // ---- statements and expressions

    fn stmts(&mut self, b: &Value) -> Vec<Value> {
        b.as_array().into_iter().flatten().map(|s| self.stmt(s)).collect()
    }

    fn stmt(&mut self, s: &Value) -> Value {
        let Some((kind, v)) = s.as_object().and_then(|o| o.iter().next()) else { return json!({}) };
        match kind.as_str() {
            "declare" => self.declare(v),
            "set" => json!({ "set": { "place": self.place(&v["place"]), "value": self.expr(&v["value"]) } }),
            "return" => match v.get("value") {
                Some(x) if !x.is_null() => json!({ "return": { "value": self.expr(x) } }),
                _ => json!({ "return": {} }),
            },
            "if" => {
                let branches: Vec<Value> = v["branches"].as_array().into_iter().flatten().map(|b| self.branch(b, |c, t| json!(c.stmts(t)), "body")).collect();
                let mut o = Map::new();
                o.insert("branches".into(), json!(branches));
                if let Some(e) = v.get("else").filter(|e| !e.is_null()) {
                    o.insert("else".into(), json!(self.stmts(e)));
                }
                json!({ "if": o })
            }
            "for" => {
                let mut o = self.walk(v);
                o.insert("body".into(), json!(self.stmts(&v["body"])));
                json!({ "for": o })
            }
            "match" => {
                let mut cases: Vec<Value> = v["cases"].as_array().into_iter().flatten().map(|c| json!({ "patterns": self.patterns(&c["when"]), "body": self.stmts(&c["then"]) })).collect();
                if let Some(e) = v.get("else").filter(|e| !e.is_null()) {
                    cases.push(json!({ "patterns": [], "body": self.stmts(e) }));
                }
                json!({ "match": { "subject": self.expr(&v["value"]), "cases": cases } })
            }
            "invoke" => json!({ "invoke": { "event": v["event"], "args": v["args"].as_array().into_iter().flatten().map(|a| self.expr(a)).collect::<Vec<_>>() } }),
            "call" | "apply" => json!({ "expr": self.expr(s) }),
            other => {
                self.errors.push(format!("a statement of a kind an IR has that this xtmd can't write back: {}", other));
                json!({})
            }
        }
    }

    fn declare(&mut self, v: &Value) -> Value {
        let (kind, d) = match v.as_object().and_then(|o| o.iter().next()) {
            Some((k, d)) => (k.as_str(), d),
            None => return json!({}),
        };
        let word = if kind == "mut" { "mut" } else { "let" };
        json!({ word: { "name": d["name"], "type": d["type"], "value": self.expr(&d["value"]) } })
    }

    fn branch(&mut self, b: &Value, body: impl Fn(&mut Self, &Value) -> Value, key: &str) -> Value {
        let mut o = Map::new();
        let when = &b["when"];
        if let Some(open) = when.get("open") {
            o.insert("opens".into(), json!({ "name": open["name"], "type": open["type"], "value": self.expr(&open["opt"]), "mutable": open["mut"] == true }));
        } else {
            o.insert("cond".into(), self.expr(when.get("cond").unwrap_or(when)));
        }
        o.insert(key.into(), body(self, &b["then"]));
        Value::Object(o)
    }

    fn walk(&mut self, v: &Value) -> Map<String, Value> {
        let mut o = Map::new();
        o.insert("name".into(), v["name"].clone());
        if let Some(k) = v["position"].as_str() {
            o.insert("key".into(), json!(k));
        }
        if v["mut"] == true {
            o.insert("mutable".into(), json!(true));
        }
        let over = &v["over"];
        o.insert("over".into(), if over.get("place").is_some() { self.place(&over["place"]) } else { self.expr(over) });
        o
    }

    fn patterns(&mut self, ps: &Value) -> Vec<Value> {
        ps.as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                if let Some(c) = p.get("case") {
                    let mut x = json!({ "case": { "enum": self.named(c["enum"].as_str().unwrap_or("")), "name": c["name"] } });
                    if let Some(b) = c["binds"].as_array() {
                        x["case"]["binds"] = json!(b);
                    }
                    Some(x)
                } else if p.get("else").is_some() {
                    None
                } else {
                    Some(p.clone())
                }
            })
            .collect()
    }

    /// A place written to, as the expression that names it: `$state.items[0].name`.
    fn place(&mut self, p: &Value) -> Value {
        let root = &p["root"];
        let mut e = self.get(root);
        for step in p["path"].as_array().into_iter().flatten() {
            e = if let Some(f) = step["field"]["name"].as_str() {
                json!({ "field": { "of": e, "field": f } })
            } else {
                json!({ "index": { "of": e, "index": self.expr(&step["item"]["at"]) } })
            };
        }
        e
    }

    fn get(&mut self, g: &Value) -> Value {
        let Some((k, v)) = g.as_object().and_then(|o| o.iter().next()) else { return json!({}) };
        if k == "local" {
            if let Some(x) = v.as_str().and_then(|n| self.held.get(n)).cloned() {
                self.lost(FOLDED);
                return self.expr(&x);
            }
        }
        match k.as_str() {
            "$state" => json!({ "get": { "state": v } }),
            "cell" => json!({ "get": { "own": v } }),
            "let" => json!({ "get": { "let": self.named(v.as_str().unwrap_or("")) } }),
            "extern" => json!({ "get": { "extern": self.named(v.as_str().unwrap_or("")) } }),
            "token" => self.token(v.as_str().unwrap_or("")),
            other => json!({ "get": { other: v } }),
        }
    }

    fn token(&mut self, t: &str) -> Value {
        let (ty, case) = t.rsplit_once('.').unwrap_or((t, ""));
        json!({ "case": { "of": self.named(ty), "case": case } })
    }

    fn expr(&mut self, e: &Value) -> Value {
        let Some((kind, v)) = e.as_object().and_then(|o| o.iter().find(|(k, _)| k.as_str() != "type")) else {
            self.errors.push(format!("an expression an IR holds with nothing in it: {}", e));
            return json!({});
        };
        match kind.as_str() {
            "num" | "text" | "bool" | "duration" => json!({ kind.as_str(): v }),
            "none" => json!({ "none": {} }),
            "some" => json!({ "some": self.expr(v) }),
            "list" => json!({ "list": v.as_array().into_iter().flatten().map(|x| self.expr(x)).collect::<Vec<_>>() }),
            "record" => {
                let ty = v["type"].as_str().unwrap_or("record");
                let order = self.field_order(ty);
                let mut fields: Vec<(String, &Value)> = v["fields"].as_object().into_iter().flatten().map(|(k, x)| (k.clone(), x)).collect();
                fields.sort_by_key(|(k, _)| order.iter().position(|n| n == k).unwrap_or(usize::MAX));
                let fields: Vec<Value> = fields.into_iter().map(|(k, x)| json!({ "name": k, "value": self.expr(x) })).collect();
                let mut o = json!({ "fields": fields });
                if ty != "record" {
                    o["of"] = json!(self.named(ty));
                }
                json!({ "record": o })
            }
            "case" => {
                let en = v["enum"].as_str().unwrap_or("");
                let order = self.case_order(en, v["name"].as_str().unwrap_or(""));
                let mut o = json!({ "of": self.named(en), "case": v["name"] });
                if let Some(values) = v["values"].as_object().filter(|m| !m.is_empty()) {
                    let mut vs: Vec<(&String, &Value)> = values.iter().collect();
                    vs.sort_by_key(|(k, _)| order.iter().position(|n| n == *k).unwrap_or(usize::MAX));
                    o["args"] = json!(vs.into_iter().map(|(_, x)| self.expr(x)).collect::<Vec<_>>());
                }
                json!({ "case": o })
            }
            "range" => {
                let mut o = Map::new();
                for k in ["low", "high", "step"] {
                    if let Some(x) = v.get(k) {
                        o.insert(k.into(), self.expr(x));
                    }
                }
                json!({ "range": o })
            }
            "token" => self.token(v.as_str().unwrap_or("")),
            "tokval" => {
                let ty = v["type"].as_str().unwrap_or("");
                json!({ "case": { "of": self.named(ty), "case": v["name"], "args": [self.expr(&v["value"])] } })
            }
            "template" => {
                // a hole shows its value as text: the hole is the value
                let holes: Vec<Value> = v["holes"].as_array().into_iter().flatten().map(|h| self.expr(h.get("show").map(|x| &x["value"]).unwrap_or(h))).collect();
                json!({ "template": { "pieces": v["pieces"], "holes": holes } })
            }
            "show" => {
                // a value shown as text outside a template: a template of one hole
                let inner = self.expr(&v["value"]);
                json!({ "template": { "pieces": ["", ""], "holes": [inner] } })
            }
            "rich" | "sentence" => {
                self.lost(MARKDOWN);
                let (text, holes) = self.rich_text(v);
                let pieces: Vec<&str> = text.split('\u{fffc}').collect();
                json!({ "template": { "pieces": pieces, "holes": holes } })
            }
            "get" => self.get(v),
            "place" => self.place(v),
            "field" => json!({ "field": { "of": self.expr(&v["value"]), "field": v["name"] } }),
            "call" => self.call(v),
            "apply" => json!({ "apply": { "fn": self.expr(&v["fn"]), "args": v["args"].as_array().into_iter().flatten().map(|a| self.expr(a)).collect::<Vec<_>>() } }),
            "op" => {
                let args: Vec<Value> = v["args"].as_array().into_iter().flatten().map(|a| self.expr(a)).collect();
                match v["name"].as_str().unwrap_or("") {
                    "?" if args.len() == 3 => json!({ "pick": { "cond": args[0], "then": args[1], "else": args[2] } }),
                    op => json!({ "op": { "op": op, "args": args } }),
                }
            }
            "fn" => json!({ "lambda": self.lambda(v) }),
            "try" => json!({ "try": self.expr(v) }),
            "with" => {
                let fields: Vec<Value> = v["fields"].as_object().into_iter().flatten().map(|(k, x)| json!({ "name": k, "value": self.expr(x) })).collect();
                json!({ "with": { "of": self.expr(&v["value"]), "fields": fields } })
            }
            "view" => {
                let u = self.view_value(v);
                json!({ "use": u })
            }
            "pick" => json!({ "pick": { "cond": self.expr(&v["cond"]), "then": self.expr(&v["then"]), "else": self.expr(&v["else"]) } }),
            "index" => json!({ "index": { "of": self.expr(&v["value"]), "index": self.expr(&v["index"]) } }),
            other => {
                self.errors.push(format!("an expression of a kind an IR has that this xtmd can't write back: {}", other));
                json!({})
            }
        }
    }

    fn lambda(&mut self, v: &Value) -> Value {
        let mut o = Map::new();
        o.insert("params".into(), json!(self.params(&v["params"])));
        if !v["returns"].is_null() {
            o.insert("returns".into(), v["returns"].clone());
        }
        o.insert("body".into(), json!(self.stmts(&v["body"])));
        Value::Object(o)
    }

    /// A fn called: on its first argument when the fn takes `self`, or by name.
    fn call(&mut self, v: &Value) -> Value {
        let name = v["name"].as_str().unwrap_or("").to_string();
        let args: Vec<Value> = v["args"].as_array().into_iter().flatten().cloned().collect();
        let takes_self = self.decls.get(&name).map_or(false, |(_, d)| d["params"][0]["name"] == "self");
        // a widget's own fn is called by its name alone
        let local = name.rsplit_once('.').filter(|(w, _)| self.templates.contains_key(*w)).map(|(_, f)| f.to_string());
        let mut o = json!({ "fn": local.unwrap_or_else(|| self.named(&name)) });
        let mut rest = &args[..];
        if takes_self && !args.is_empty() {
            o["self"] = self.expr(&args[0]);
            rest = &args[1..];
        }
        o["args"] = json!(rest.iter().map(|a| self.expr(a)).collect::<Vec<_>>());
        json!({ "call": o })
    }

    fn field_order(&self, ty: &str) -> Vec<String> {
        self.decls.get(ty).map(|(_, d)| d["fields"].as_array().into_iter().flatten().filter_map(|f| f["name"].as_str().map(String::from)).collect()).unwrap_or_default()
    }

    fn case_order(&self, en: &str, case: &str) -> Vec<String> {
        self.decls
            .get(en)
            .and_then(|(_, d)| d["cases"].as_array()?.iter().find(|c| c["name"] == case).cloned())
            .map(|c| c["fields"].as_array().into_iter().flatten().filter_map(|f| f["name"].as_str().map(String::from)).collect())
            .unwrap_or_default()
    }

    // ---- views

    fn view(&mut self, nodes: &Value) -> Vec<Value> {
        let nodes: Vec<Value> = nodes.as_array().cloned().unwrap_or_default();
        let mut out = Vec::new();
        let mut k = 0;
        while k < nodes.len() {
            let n = &nodes[k];
            k += 1;
            // `@title;` for a rich_text: its value held, then a sentence or blocks
            if let Some(held) = n["declare"]["let"]["name"].as_str().filter(|x| x.starts_with('~')) {
                let next = nodes.get(k).and_then(|x| x.get("if"));
                if next.map_or(false, |i| i["branches"][0]["when"]["cond"]["sentence"]["get"]["local"] == held) {
                    k += 1;
                    out.push(self.placed(&n["declare"]["let"]["value"]));
                    continue;
                }
            }
            out.push(self.view_node(n));
        }
        out
    }

    fn view_node(&mut self, n: &Value) -> Value {
        let Some((kind, v)) = n.as_object().and_then(|o| o.iter().next()) else { return json!({}) };
        match kind.as_str() {
            "extern" | "use" => json!({ "use": self.call_of(v, false) }),
            "for" => {
                let mut o = self.walk(v);
                o.insert("body".into(), json!(self.view(&v["body"])));
                json!({ "for": o })
            }
            "if" => {
                let branches: Vec<Value> = v["branches"].as_array().into_iter().flatten().map(|b| self.branch(b, |c, t| json!(c.view(t)), "body")).collect();
                let mut o = Map::new();
                o.insert("branches".into(), json!(branches));
                if let Some(e) = v.get("else").filter(|e| !e.is_null()) {
                    o.insert("else".into(), json!(self.view(e)));
                }
                json!({ "if": o })
            }
            "match" => {
                let mut cases: Vec<Value> = v["cases"].as_array().into_iter().flatten().map(|c| json!({ "patterns": self.patterns(&c["when"]), "body": self.view(&c["then"]) })).collect();
                if let Some(e) = v.get("else").filter(|e| !e.is_null()) {
                    cases.push(json!({ "patterns": [], "body": self.view(e) }));
                }
                json!({ "match": { "subject": self.expr(&v["value"]), "cases": cases } })
            }
            "declare" => self.declare(v),
            "place" => self.placed(&v["view"]),
            other => {
                self.errors.push(format!("a node of a kind an IR has that this xtmd can't write back: {}", other));
                json!({})
            }
        }
    }

    /// A view value placed in a view: `@body;` for a slot or a variable that holds one.
    fn placed(&mut self, view: &Value) -> Value {
        let name = ["slot", "local", "input", "cell"].iter().find_map(|k| view["get"][*k].as_str()).map(String::from).or_else(|| view["get"]["let"].as_str().map(|l| l.rsplit('/').next().unwrap_or(l).to_string()));
        match name {
            Some(n) => json!({ "use": { "widget": n, "args": [] } }),
            None => {
                self.errors.push(format!("a view placed from a value this xtmd can't write back: {}", view));
                json!({})
            }
        }
    }

    fn view_value(&mut self, v: &Value) -> Value {
        let mut x = v.clone();
        x["name"] = v["widget"].clone();
        x["props"] = v["args"].clone();
        self.call_of(&x, false)
    }

    /// A widget called: its arguments in the order it declares its inputs, a `mut` input's place, each
    /// event's handler as a fn, and its slots, the one it has as the call's block.
    fn call_of(&mut self, v: &Value, prose: bool) -> Value {
        let name = v["name"].as_str().unwrap_or("").to_string();
        let order: Vec<String> = self.inputs_of(&name);
        let mut args: Vec<(usize, Value)> = Vec::new();
        let props = v["props"].as_object().or_else(|| v["args"].as_object());
        for (k, x) in props.into_iter().flatten() {
            let at = order.iter().position(|n| n == k).unwrap_or(usize::MAX);
            args.push((at, json!({ "name": k, "value": self.expr(x) })));
        }
        for (k, p) in v["writes"].as_object().into_iter().flatten() {
            let at = order.iter().position(|n| n == k).unwrap_or(usize::MAX);
            args.push((at, json!({ "name": k, "value": self.place(p) })));
        }
        for (k, h) in v["events"].as_object().into_iter().flatten() {
            let lambda = match self.handlers.get(h.as_str().unwrap_or("")).copied() {
                Some(h) => json!({ "lambda": { "params": self.params(&h["params"]), "body": self.stmts(&h["body"]) } }),
                None => {
                    self.errors.push(format!("{}: its event {} names a handler the IR doesn't hold", name, k));
                    json!({})
                }
            };
            // handlers are numbered as they are written: that is the events' order
            let at = h.as_str().and_then(|h| h.trim_start_matches('h').parse::<usize>().ok()).unwrap_or(0);
            args.push((usize::MAX / 2 + at, json!({ "name": k, "value": lambda })));
        }
        if args.len() > 1 {
            self.lost(ORDER);
        }
        args.sort_by_key(|(at, _)| *at);
        let mut o = Map::new();
        o.insert("widget".into(), json!(self.named(&name)));
        o.insert("args".into(), json!(args.into_iter().map(|(_, a)| a).collect::<Vec<_>>()));
        let slots_declared = self.slots_of(&name);
        let mut slots = Vec::new();
        for (s, nodes) in v["slots"].as_object().into_iter().flatten() {
            let children = slots_declared.len() == 1;
            let mut so = Map::new();
            so.insert("slot".into(), json!(format!("{}.{}", name, s)));
            if children {
                so.insert("children".into(), json!(true));
            }
            if prose {
                so.insert("prose".into(), json!(self.prose(nodes)));
            } else {
                so.insert("view".into(), json!(self.view(nodes)));
            }
            slots.push(Value::Object(so));
        }
        if !slots.is_empty() {
            o.insert("slots".into(), json!(slots));
        }
        Value::Object(o)
    }

    fn inputs_of(&self, widget: &str) -> Vec<String> {
        let from = |v: &Value| v["inputs"].as_array().into_iter().flatten().filter_map(|i| i["name"].as_str().map(String::from)).collect::<Vec<_>>();
        if let Some(t) = self.templates.get(widget) {
            return from(t);
        }
        self.decls.get(widget).map(|(_, d)| from(d)).unwrap_or_default()
    }

    fn slots_of(&self, widget: &str) -> Vec<String> {
        let from = |v: &Value| v["slots"].as_array().into_iter().flatten().filter_map(|i| i["name"].as_str().map(String::from)).collect::<Vec<_>>();
        if let Some(t) = self.templates.get(widget) {
            return from(t);
        }
        self.decls.get(widget).map(|(_, d)| from(d)).unwrap_or_default()
    }

    fn piece_of(&self, widget: &str) -> Option<&str> {
        self.decls.get(widget).and_then(|(_, d)| d["piece"].as_str())
    }

    // ---- prose

    /// Lowered prose back as prose: Markdown's pieces as Markdown, every other call as a call line.
    fn prose(&mut self, nodes: &Value) -> Vec<Value> {
        let mut out: Vec<Value> = Vec::new();
        let nodes: Vec<Value> = nodes.as_array().cloned().unwrap_or_default();
        let mut k = 0;
        while k < nodes.len() {
            let n = &nodes[k];
            k += 1;
            let Some((kind, v)) = n.as_object().and_then(|o| o.iter().next()) else { continue };
            // a hole alone in its paragraph, `\{note}`: its value held, then a sentence or blocks
            if kind == "declare" && v["let"]["name"].as_str().map_or(false, |x| x.starts_with('~')) {
                let held = v["let"]["name"].as_str().unwrap_or("");
                let next = nodes.get(k).and_then(|x| x.get("if"));
                if next.map_or(false, |i| i["branches"][0]["when"]["cond"]["sentence"]["get"]["local"] == held) {
                    k += 1;
                    let value = &v["let"]["value"];
                    let hole = if value.get("show").is_some() { self.expr(&value["show"]["value"]) } else { self.expr(value) };
                    out.push(json!({ "markdown": { "markdown": "\u{fffc}", "holes": [hole] } }));
                    continue;
                }
            }
            // prose in a call's block, `@bold { plain, and *leaning* }`: its words, as written
            if kind == "place" && v["view"].get("rich").is_some() {
                let (text, holes) = self.rich_text(&v["view"]["rich"]);
                self.lost(MARKDOWN);
                out.push(json!({ "markdown": { "markdown": text, "holes": holes } }));
                continue;
            }
            if kind == "place" {
                let hole = self.expr(&v["view"]);
                out.push(json!({ "markdown": { "markdown": "\u{fffc}", "holes": [hole] } }));
                continue;
            }
            match kind.as_str() {
                "extern" | "use" => {
                    let name = v["name"].as_str().unwrap_or("");
                    match self.block_markdown(name, v) {
                        Some((text, holes)) => {
                            self.lost(MARKDOWN);
                            out.push(json!({ "markdown": { "markdown": text, "holes": holes } }));
                        }
                        None => out.push(json!({ "call": self.call_of(v, true) })),
                    }
                }
                "for" => {
                    let mut o = self.walk(v);
                    o.insert("prose".into(), json!(self.prose(&v["body"])));
                    out.push(json!({ "for": o }));
                }
                "if" => {
                    let branches: Vec<Value> = v["branches"].as_array().into_iter().flatten().map(|b| self.branch(b, |c, t| json!(c.prose(t)), "prose")).collect();
                    let mut o = Map::new();
                    o.insert("branches".into(), json!(branches));
                    if let Some(e) = v.get("else").filter(|e| !e.is_null()) {
                        o.insert("else".into(), json!(self.prose(e)));
                    }
                    out.push(json!({ "if": o }));
                }
                "match" => {
                    let mut cases: Vec<Value> = v["cases"].as_array().into_iter().flatten().map(|c| json!({ "patterns": self.patterns(&c["when"]), "prose": self.prose(&c["then"]) })).collect();
                    if let Some(e) = v.get("else").filter(|e| !e.is_null()) {
                        cases.push(json!({ "patterns": [], "prose": self.prose(e) }));
                    }
                    out.push(json!({ "match": { "subject": self.expr(&v["value"]), "cases": cases } }));
                }
                other => self.errors.push(format!("prose of a kind an IR has that this xtmd can't write back: {}", other)),
            }
        }
        out
    }

    /// A block of Markdown's own, as its Markdown: a paragraph, a heading, a list, a quote, code, maths, a
    /// table, a footnote, a divider. None for any other call.
    fn block_markdown(&mut self, name: &str, v: &Value) -> Option<(String, Vec<Value>)> {
        let piece = self.piece_of(name)?.to_string();
        let props = &v["props"];
        let mut holes = Vec::new();
        let text = match piece.as_str() {
            "paragraph" => self.inline_of(&props["value"], &mut holes),
            "heading" => {
                let level = props["level"]["num"].as_str().and_then(|n| n.parse::<usize>().ok()).unwrap_or(1);
                format!("{} {}", "#".repeat(level), self.inline_of(&props["value"], &mut holes))
            }
            "divider" => "---".into(),
            "code" => {
                let src = props["src"]["text"].as_str()?;
                let lang = props["lang"]["text"].as_str().unwrap_or("");
                format!("```{}\n{}```", lang, src)
            }
            "math" => format!("$$\n{}\n$$", self.latex(&props["tex"], &mut holes)),
            "bullets" => {
                let ordered = props["ordered"]["bool"] == true;
                let start = props["start"]["num"].as_str().and_then(|n| n.parse::<u64>().ok()).unwrap_or(1);
                let mut lines = Vec::new();
                for (k, item) in v["slots"]["items"].as_array().into_iter().flatten().enumerate() {
                    let it = item.get("extern").or_else(|| item.get("use"))?;
                    let mark = if ordered { format!("{}. ", start + k as u64) } else { "- ".into() };
                    let task = match &it["props"]["done"] {
                        d if d.get("some").is_some() => if d["some"]["bool"] == true { "[x] " } else { "[ ] " },
                        _ => "",
                    };
                    let body = self.blocks_text(&it["slots"]["body"], &mut holes)?;
                    let pad = " ".repeat(mark.len());
                    let body = body.replace('\n', &format!("\n{}", pad)).replace(&format!("\n{}\n", pad), "\n\n");
                    lines.push(format!("{}{}{}", mark, task, body));
                }
                lines.join("\n")
            }
            "quote" => {
                let body = self.blocks_text(&v["slots"]["body"], &mut holes)?;
                body.lines().map(|l| if l.is_empty() { ">".to_string() } else { format!("> {}", l) }).collect::<Vec<_>>().join("\n")
            }
            "footnote" => {
                let body = self.blocks_text(&v["slots"]["body"], &mut holes)?;
                format!("[^{}]: {}", props["mark"]["text"].as_str().unwrap_or(""), body.replace('\n', "\n    "))
            }
            "table" => {
                let head: Vec<String> = props["head"]["list"].as_array()?.iter().map(|c| self.inline_of(c, &mut holes)).collect();
                // a column's alignment: START is what a colon on the left says, and nothing says it too
                let align: Vec<String> = props["align"]["list"].as_array().into_iter().flatten().map(|a| match a["case"]["name"].as_str() {
                    Some("START") => ":---".to_string(),
                    Some("CENTER") => ":---:".to_string(),
                    Some("END") => "---:".to_string(),
                    _ => "---".to_string(),
                }).collect();
                let mut lines = vec![format!("| {} |", head.join(" | "))];
                let marks: Vec<String> = (0..head.len()).map(|k| align.get(k).cloned().unwrap_or_else(|| "---".into())).collect();
                lines.push(format!("| {} |", marks.join(" | ")));
                for row in props["rows"]["list"].as_array().into_iter().flatten() {
                    let cells: Vec<String> = row["list"].as_array().into_iter().flatten().map(|c| self.inline_of(c, &mut holes)).collect();
                    lines.push(format!("| {} |", cells.join(" | ")));
                }
                lines.join("\n")
            }
            _ => return None,
        };
        Some((text, holes))
    }

    /// The blocks in a list item, a quote or a footnote, as Markdown with a blank line between; None when one
    /// isn't Markdown's own.
    fn blocks_text(&mut self, nodes: &Value, holes: &mut Vec<Value>) -> Option<String> {
        let mut parts = Vec::new();
        for n in nodes.as_array().into_iter().flatten() {
            // a hole alone where a block goes, `- \{r}`: its value placed
            if let Some(p) = n.get("place") {
                holes.push(self.expr(&p["view"]));
                parts.push("\u{fffc}".to_string());
                continue;
            }
            let v = n.get("extern").or_else(|| n.get("use"))?;
            let (text, hs) = self.block_markdown(v["name"].as_str().unwrap_or(""), v)?;
            holes.extend(hs);
            parts.push(text);
        }
        Some(parts.join("\n\n"))
    }

    /// A rich_text value as the Markdown of one sentence: its text, with a hole for each value shown.
    fn inline_of(&mut self, value: &Value, holes: &mut Vec<Value>) -> String {
        match value.as_object().and_then(|o| o.iter().find(|(k, _)| k.as_str() != "type")) {
            Some((k, v)) if k == "rich" || k == "sentence" => {
                let (text, hs) = self.rich_text(v);
                holes.extend(hs);
                text
            }
            Some((k, v)) if k == "text" => escape(v.as_str().unwrap_or("")),
            _ => {
                holes.push(self.expr(value));
                "\u{fffc}".into()
            }
        }
    }

    /// A `latex` value's TeX, its holes as holes: `{ tex: { pieces, holes } }` as the IR has it.
    fn latex(&mut self, v: &Value, holes: &mut Vec<Value>) -> String {
        if let Some(t) = v["text"].as_str() {
            return t.to_string();
        }
        let t = &v["record"]["fields"]["tex"]["record"]["fields"];
        let pieces: Vec<&str> = t["pieces"]["list"].as_array().into_iter().flatten().filter_map(|p| p["text"].as_str()).collect();
        let hs: Vec<&Value> = t["holes"]["list"].as_array().into_iter().flatten().collect();
        let mut s = String::new();
        for (k, p) in pieces.iter().enumerate() {
            s.push_str(p);
            if let Some(h) = hs.get(k) {
                // `hole.TEXT(value)`, its value shown
                let h = h.pointer("/case/values/value").unwrap_or(h);
                holes.push(self.expr(h.get("show").map(|x| &x["value"]).unwrap_or(h)));
                s.push('\u{fffc}');
            }
        }
        s
    }

    fn rich_text(&mut self, r: &Value) -> (String, Vec<Value>) {
        let mut holes = Vec::new();
        let mut s = String::new();
        if let Some(blocks) = r["blocks"].as_array() {
            let text = self.blocks_text(&Value::Array(blocks.clone()), &mut holes).unwrap_or_default();
            return (text, holes);
        }
        let mut after_text = false;
        let nodes: Vec<Value> = r["nodes"].as_array().cloned().unwrap_or_default();
        let block = |c: &Self, n: &Value| {
            n.get("extern").or_else(|| n.get("use")).and_then(|v| c.piece_of(v["name"].as_str().unwrap_or(""))).map_or(false, |p| {
                matches!(p, "paragraph" | "heading" | "bullets" | "quote" | "code" | "math" | "table" | "footnote" | "divider")
            })
        };
        if nodes.iter().any(|n| block(self, n)) {
            let text = self.blocks_text(&Value::Array(nodes), &mut holes).unwrap_or_default();
            return (text, holes);
        }
        // a call worked out at compile time leaves its locals before the words that read them
        for n in &nodes {
            if let Some(d) = n["declare"]["let"].as_object() {
                if let Some(name) = d.get("name").and_then(Value::as_str) {
                    self.held.insert(name.to_string(), d.get("value").cloned().unwrap_or_default());
                }
            }
        }
        for n in &nodes {
            if n.get("declare").is_some() {
                continue;
            }
            if let Some(p) = n.get("place") {
                holes.push(self.expr(&p["view"]));
                s.push('\u{fffc}');
                after_text = false;
                continue;
            }
            let v = n.get("extern").or_else(|| n.get("use"));
            let text = v.filter(|v| self.piece_of(v["name"].as_str().unwrap_or("")) == Some("text")).and_then(|v| v["props"]["value"]["text"].as_str());
            match text {
                // Markdown's text is one run: words after words were a hole, its value worked out
                Some(t) if after_text => {
                    holes.push(json!({ "text": t }));
                    s.push('\u{fffc}');
                }
                _ => self.inline(n, &mut s, &mut holes),
            }
            after_text = text.is_some();
        }
        (s, holes)
    }

    fn inline(&mut self, n: &Value, s: &mut String, holes: &mut Vec<Value>) {
        // a value shown in a sentence, in bold or italics too
        if let Some(p) = n.get("place") {
            holes.push(self.expr(&p["view"]));
            s.push('\u{fffc}');
            return;
        }
        let Some(v) = n.get("extern").or_else(|| n.get("use")) else { return };
        let name = v["name"].as_str().unwrap_or("");
        let props = &v["props"];
        let inner = |c: &mut Self, s: &mut String, holes: &mut Vec<Value>| {
            for x in v["slots"]["body"].as_array().into_iter().flatten() {
                c.inline(x, s, holes);
            }
        };
        match self.piece_of(name).unwrap_or("") {
            "text" => match props["value"].get("text").and_then(Value::as_str) {
                Some(t) => s.push_str(&escape(t)),
                None => {
                    let value = &props["value"];
                    let shown = value.get("show").map(|x| &x["value"]).unwrap_or(value);
                    holes.push(self.expr(shown));
                    s.push('\u{fffc}');
                }
            },
            "bold" => {
                s.push_str("**");
                inner(self, s, holes);
                s.push_str("**");
            }
            "italic" => {
                s.push('*');
                inner(self, s, holes);
                s.push('*');
            }
            "strike" => {
                s.push_str("~~");
                inner(self, s, holes);
                s.push_str("~~");
            }
            "link" => {
                s.push('[');
                inner(self, s, holes);
                s.push_str(&format!("]({})", props["to"]["text"].as_str().unwrap_or("")));
            }
            "code_span" => s.push_str(&format!("`{}`", props["src"]["text"].as_str().unwrap_or(""))),
            "inline_math" => {
                let tex = self.latex(&props["tex"], holes);
                s.push_str(&format!("${}$", tex));
            }
            "inline_image" => s.push_str(&format!("![{}]({})", props["alt"]["text"].as_str().unwrap_or(""), props["src"]["text"].as_str().unwrap_or(""))),
            "note_mark" => s.push_str(&format!("[^{}]", props["mark"]["text"].as_str().unwrap_or(""))),
            "line_break" => s.push_str("\\\n"),
            _ => {
                // a widget in a sentence: a hole that holds the call
                let u = self.call_of(v, false);
                holes.push(json!({ "use": u }));
                s.push('\u{fffc}');
            }
        }
    }
}

fn extern_kind(k: &str) -> &'static str {
    match k {
        "fn" => "extern_fn",
        "widget" => "extern_widget",
        "type" => "extern_type",
        "channel" => "channel",
        _ => "extern_other",
    }
}

fn decl_kind(k: &str) -> &'static str {
    match k {
        "record" => "record",
        "enum" => "enum",
        "token" => "token",
        "type" => "type",
        "interface" => "interface",
        "fn" => "fn",
        "let" => "let",
        "$state" => "state",
        "expose" => "expose",
        _ => "other",
    }
}

/// Text that was Markdown's, escaped so that it reads as the same text: what would start a piece of
/// Markdown, a hole or a call.
fn escape(t: &str) -> String {
    let mut s = String::new();
    for c in t.chars() {
        if matches!(c, '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '$' | '~' | '@' | '#' | '|') {
            s.push('\\');
        }
        s.push(c);
    }
    s
}
