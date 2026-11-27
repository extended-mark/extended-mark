//! `xtmd decompile`: a resolved IR (schemas/rir.schema.json) printed back as source, the opposite of
//! `xtmd compile --rir`.
//!
//! Each file of the resolved IR is written out as Extended Mark text, every name by the name its file
//! knows it by and every comment in its place, and then printed by `xtmd fmt`'s printer, so what comes
//! out is in the canonical layout. Nothing is read from the source the resolved IR was made from: a
//! migration's steps change the resolved IR, and its spans no longer point anywhere. A part the
//! resolved IR holds that this can't write is an error that names it, never a part left out.

use serde_json::{json, Value};

pub mod ir;

/// The resolved IR's version this prints from, and the shape `ir` turns an IR into.
pub const RIR_SHAPE: u64 = 2;

use crate::syntax::{format, Entry};

/// One file, printed back.
pub struct Printed {
    pub path: String,
    pub text: String,
}

/// Every file of a resolved IR, printed back; or what kept one from being printed.
pub fn decompile(rir: &Value) -> Result<Vec<Printed>, Vec<String>> {
    let Some(files) = rir["files"].as_array() else {
        return Err(vec!["it isn't a resolved IR: `xtmd compile --rir` writes { \"rir\": …, \"files\": [ … ] }".into()]);
    };
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for f in files {
        match file(f) {
            Ok(p) => out.push(p),
            Err(e) => errors.extend(e),
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// One file of a resolved IR, printed back.
pub fn file(f: &Value) -> Result<Printed, Vec<String>> {
    let path = f["path"].as_str().unwrap_or("").to_string();
    let mut w = Writer::new(f);
    w.file(f);
    if !w.errors.is_empty() {
        return Err(w.errors.into_iter().map(|e| format!("{}: {}", path, e)).collect());
    }
    let entry = if f["kind"] == "document" { Entry::Document } else { Entry::Xtmd };
    match format(&w.out, entry) {
        Ok(text) => Ok(Printed { path, text }),
        Err(diags) => Err(diags.iter().map(|d| format!("{}: what was written back doesn't read, {} at {}: {}\n{}", path, d.code, d.span.start, d.message, numbered(&w.out))).collect()),
    }
}

fn numbered(text: &str) -> String {
    text.lines().enumerate().map(|(k, l)| format!("{:4} {}", k + 1, l)).collect::<Vec<_>>().join("\n")
}

/// The width a list is kept on one line within, and fmt's indent.
const WIDTH: usize = 110;
const STEP: usize = 4;

/// Expressions, loosest first: an operand of an operator is put in brackets when it binds looser.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Level {
    Pick,
    Or,
    And,
    Equal,
    Compare,
    Sum,
    Term,
    Unary,
    Postfix,
}

fn level_of(op: &str, arity: usize) -> Level {
    match op {
        "||" => Level::Or,
        "&&" => Level::And,
        "==" | "!=" => Level::Equal,
        "<" | "<=" | ">" | ">=" | "in" => Level::Compare,
        "+" | "-" if arity == 2 => Level::Sum,
        "*" | "/" | "%" => Level::Term,
        _ => Level::Unary,
    }
}

/// The comments of the node being written: which bracket pair comes next, and the comments after
/// each pair's brackets.
#[derive(Default)]
struct Brackets {
    next: usize,
    starts: Vec<(usize, String)>,
    ends: Vec<(usize, String)>,
}

struct Writer<'a> {
    out: String,
    /// The file's own pack, whose names are written bare.
    imports: Vec<(&'a str, Option<&'a str>, Option<&'a str>)>,
    /// The bracket pairs of the nodes being written, innermost last.
    nodes: Vec<Brackets>,
    /// The indent of the prose being written: four spaces for each block it is in.
    indent: String,
    /// Inside a hole of a text or of prose, which is read on one line, so no list breaks.
    flat: usize,
    /// How many brackets are open where the writer is, for how far fmt will indent the line.
    depth: usize,
    /// Inside the `$state` block, where its fields are read by their names alone.
    in_state: bool,
    errors: Vec<String>,
}

impl<'a> Writer<'a> {
    fn new(f: &'a Value) -> Writer<'a> {
        let imports = f["imports"].as_array().into_iter().flatten().filter(|i| i["comment"].is_null()).map(|i| (i["pack"].as_str().unwrap_or(""), i["name"].as_str(), i["as"].as_str())).collect();
        Writer { out: String::new(), imports, nodes: Vec::new(), indent: String::new(), in_state: false, flat: 0, depth: 0, errors: Vec::new() }
    }

    fn w(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn nl(&mut self) {
        self.out.push('\n');
    }

    fn fail(&mut self, what: String) {
        self.errors.push(what);
    }

    // ---- names

    /// What a file calls a declaration by its id: its own name, or the name an import gave it.
    fn name(&self, id: &str) -> String {
        let (pack, rest) = id.split_once('/').unwrap_or(("", id));
        let pack = pack.split('@').next().unwrap_or(pack);
        let decl = rest.split('.').next().unwrap_or(rest);
        for (p, name, alias) in &self.imports {
            if *p == pack && *name == Some(decl) {
                if let Some(a) = alias {
                    return rest.replacen(decl, a, 1);
                }
            }
        }
        rest.to_string()
    }

    /// What a file calls a fn a pack adds to a type: its name, or the one `import b.slug as b_slug;` gave it.
    fn fn_name(&self, id: &str) -> String {
        let pack = id.split('/').next().unwrap_or("").split('@').next().unwrap_or("");
        let last = Self::last(id);
        let alias = self.imports.iter().find(|(p, n, _)| *p == pack && *n == Some(last)).and_then(|(_, _, a)| *a);
        alias.unwrap_or(last).to_string()
    }

    /// The last part of an id: a member's, a fn's or a declaration's own name.
    fn last(id: &str) -> &str {
        let rest = id.rsplit('/').next().unwrap_or(id);
        rest.rsplit('.').next().unwrap_or(rest)
    }

    // ---- comments

    fn enter(&mut self, c: &Value) {
        let mut b = Brackets::default();
        for (key, into) in [("block_start", &mut b.starts), ("block_end", &mut b.ends)] {
            for x in c[key].as_array().into_iter().flatten() {
                into.push((x["at"].as_u64().unwrap_or(0) as usize, x["text"].as_str().unwrap_or("").to_string()));
            }
        }
        self.nodes.push(b);
    }

    fn leave(&mut self) {
        self.nodes.pop();
    }

    /// A node's comments above it: its doc lines, then the others.
    fn above(&mut self, c: &Value) {
        if let Some(d) = c["doc"].as_object() {
            for key in ["about", "deprecated"] {
                if let Some(t) = d.get(key).and_then(Value::as_str) {
                    for l in t.split('\n') {
                        self.w(&format!("//@{} {}\n", key, l).replace(" \n", "\n"));
                    }
                }
            }
            for (p, t) in d.get("params").and_then(Value::as_object).into_iter().flatten() {
                for l in t.as_str().unwrap_or("").split('\n') {
                    self.w(&format!("//@param {} {}\n", p, l).replace(" \n", "\n"));
                }
            }
        }
        for l in c["leading"].as_array().into_iter().flatten() {
            self.w(l.as_str().unwrap_or(""));
            self.nl();
        }
    }

    /// The comment that ends a node's line, after its `;` or `,`.
    fn trailing(&mut self, c: &Value) {
        if let Some(t) = c["trailing"].as_str() {
            self.w(" ");
            self.w(t);
        }
    }

    /// Free comments, before what comes next: each on a line of its own.
    fn free(&mut self, c: &Value) {
        for l in c["free"].as_array().into_iter().flatten() {
            self.nl();
            self.w(l.as_str().unwrap_or(""));
            self.nl();
        }
    }

    /// A blank line, unless one was just written or nothing has been.
    fn blank(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with("\n\n") {
            if !self.out.ends_with('\n') {
                self.nl();
            }
            self.nl();
        }
    }

    /// A blank line inside a body, unless it has only just opened.
    fn body_blank(&mut self) {
        if !self.out.ends_with("{\n") {
            self.blank();
        }
    }

    /// The blank line between a body's members: around those with a block, a fn's or a rule's.
    fn gap(&mut self, last: &str, part: &str) {
        let blocky = |p: &str| matches!(p, "fn" | "require");
        if !last.is_empty() && (blocky(last) || blocky(part)) {
            self.body_blank();
        }
    }

    /// A comment item: its lines, with a blank line after them.
    fn item(&mut self, v: &Value) -> bool {
        let Some(lines) = v["comment"].as_array() else { return false };
        self.blank();
        for l in lines {
            self.w(l.as_str().unwrap_or(""));
            self.nl();
        }
        self.nl();
        true
    }

    /// An opening bracket of the node being written, with the comment after it.
    fn open(&mut self, bracket: &str) -> usize {
        self.w(bracket);
        self.depth += 1;
        let Some(b) = self.nodes.last_mut() else { return usize::MAX };
        let k = b.next;
        b.next += 1;
        let texts: Vec<String> = b.starts.iter().filter(|(at, _)| *at == k).map(|(_, t)| t.clone()).collect();
        for t in texts {
            self.w(" ");
            self.w(&t);
            self.nl();
        }
        k
    }

    fn close(&mut self, bracket: &str, k: usize) {
        self.w(bracket);
        self.depth = self.depth.saturating_sub(1);
        let texts: Vec<String> = self.nodes.last().map(|b| b.ends.iter().filter(|(at, _)| *at == k).map(|(_, t)| t.clone()).collect()).unwrap_or_default();
        for t in texts {
            self.w(" ");
            self.w(&t);
            self.nl();
        }
    }

    /// A list's entries between brackets, comma-separated, each a node with its comments; its comment
    /// items in their places.
    /// On one line when it fits in `WIDTH`, and one item a line when it doesn't.
    fn list(&mut self, open: &str, close: &str, items: &[Value], mut each: impl FnMut(&mut Self, &Value)) {
        let (at, next, errors) = (self.out.len(), self.nodes.last().map(|b| b.next), self.errors.len());
        let indent = STEP * self.depth;
        // on one line first, the lists inside it too; then broken, where they decide for themselves
        self.flat += 1;
        self.list_as(false, open, close, items, &mut each);
        self.flat -= 1;
        let line = &self.out[self.out[..at].rfind('\n').map_or(0, |k| k + 1)..];
        let written = self.out.len() - at;
        // a short list with one item gains nothing from being broken: what is before it makes the line long
        let long = (line.chars().count() + indent > WIDTH && (items.len() > 1 || written > 40)) || line.contains('\n');
        if long && self.flat == 0 && !items.is_empty() {
            self.out.truncate(at);
            self.errors.truncate(errors);
            if let (Some(b), Some(n)) = (self.nodes.last_mut(), next) {
                b.next = n;
            }
            self.list_as(true, open.trim_end(), close.trim_start(), items, &mut each);
        }
    }

    fn list_as(&mut self, broken: bool, open: &str, close: &str, items: &[Value], each: &mut impl FnMut(&mut Self, &Value)) {
        let k = self.open(open);
        let mut first = true;
        for v in items {
            if self.item(v) {
                continue;
            }
            if broken {
                self.nl();
            } else if !first {
                self.w(" ");
            }
            first = false;
            let c = v["comments"].clone();
            self.above(&c);
            self.enter(&c);
            each(self, v);
            self.free(&c);
            self.leave();
            self.w(",");
            self.trailing(&c);
            if !broken && !c["trailing"].is_null() {
                self.nl();
            }
        }
        if broken {
            self.nl();
        }
        self.close(close, k);
    }

    /// A block's statements, or a body's members, one a line, each with its comments.
    fn lines(&mut self, items: &[Value], mut each: impl FnMut(&mut Self, &Value)) {
        for v in items {
            if self.item(v) {
                continue;
            }
            let c = comments_of(v);
            self.above(&c);
            self.enter(&c);
            each(self, v);
            self.leave();
            self.trailing(&c);
            self.nl();
        }
    }

    // ---- files

    fn file(&mut self, f: &Value) {
        if let Some(t) = f["front_matter"]["text"].as_str() {
            self.w("---\n");
            self.w(t);
            if !t.is_empty() {
                self.nl();
            }
            self.w("---\n");
        }
        for i in f["head"].as_array().into_iter().flatten() {
            self.item(i);
        }
        if let Some(p) = f["package"].as_str() {
            let c = f["package_comments"].clone();
            self.above(&c);
            self.w("package ");
            self.w(p);
            if let Some(v) = f["version"].as_u64() {
                self.w(&format!("@{}", v));
            }
            self.w(";");
            self.trailing(&c);
            self.nl();
            self.nl();
        }
        // the file's items in the order they were written; comment items among them
        let mut items: Vec<(u64, &str, &Value)> = Vec::new();
        for (key, part) in [("imports", "import"), ("declarations", "decl"), ("expose", "expose"), ("comment_items", "item")] {
            for v in f[key].as_array().into_iter().flatten() {
                items.push((start(v), part, v));
            }
        }
        if !f["state"].is_null() {
            items.push((start(&f["state"]), "state", &f["state"]));
        }
        items.sort_by_key(|(at, _, _)| *at);
        let mut defs = false;
        let mut last = "";
        for (_, part, v) in items {
            defs = true;
            // a blank line between declarations; the imports together
            if !(last == "import" && part == "import") {
                self.blank();
            }
            last = part;
            if part == "item" {
                self.item(v);
                continue;
            }
            let c = comments_of(v);
            self.above(&c);
            self.enter(&c);
            match part {
                "import" => self.import(v),
                "decl" => self.decl(v),
                "state" => self.state(v),
                _ => self.expose(v),
            }
            self.leave();
            self.trailing(&c);
            self.nl();
        }
        if f["kind"] == "document" {
            if defs {
                self.w("@@@\n");
            }
            if let Some(p) = f["prose"].as_array() {
                self.prose(p);
            }
        }
    }

    fn import(&mut self, i: &Value) {
        self.w("import ");
        self.w(i["pack"].as_str().unwrap_or(""));
        if let Some(n) = i["name"].as_str() {
            self.w(".");
            self.w(n);
        }
        if let Some(a) = i["as"].as_str() {
            self.w(" as ");
            self.w(a);
        }
        self.w(";");
    }

    // ---- declarations

    fn decl(&mut self, d: &Value) {
        let Some((kind, o)) = d.as_object().and_then(|m| m.iter().next()) else { return };
        let id = o["id"].as_str().unwrap_or("");
        let name = Self::last(id).to_string();
        match kind.as_str() {
            "enum" => {
                self.w(&format!("enum {}", name));
                self.generics(o);
                self.w(" ");
                self.cases(o);
            }
            "token" => {
                self.w(&format!("token {}: ", name));
                self.ty(&o["value"]);
                self.implements(o);
                self.w(" ");
                self.cases(o);
            }
            "record" => {
                self.w(&format!("record {}", name));
                self.generics(o);
                self.implements(o);
                self.w(" ");
                self.members(o, &[("fields", "field"), ("requires", "require")]);
            }
            "type" => {
                self.w(&format!("type {} = ", name));
                self.ty(&o["is"]);
                self.w(";");
            }
            "interface" => {
                if o["widget"] == true {
                    self.w("widget ");
                }
                self.w(&format!("interface {}", name));
                self.generics(o);
                self.w(" ");
                self.members(o, &[("fields", "field"), ("fns", "sig"), ("events", "event")]);
            }
            "widget" | "extern_widget" => self.widget(kind == "extern_widget", &name, o),
            "fn" | "extern_fn" => {
                if kind == "extern_fn" {
                    self.w("extern ");
                }
                self.fn_decl(o);
                if o["body"].is_null() {
                    self.w(";");
                }
            }
            "extern_type" => {
                self.w(&format!("extern type {}", name));
                self.generics(o);
                self.implements(o);
                self.w(";");
            }
            "extern" => {
                self.w(&format!("extern {} ", name));
                let k = self.open("{");
                self.nl();
                self.member_lines(o, &[("values", "field"), ("fns", "sig")]);
                if let Some(t) = o["tokens"].as_array() {
                    self.part_above(o, "tokens");
                    self.w(&format!("tokens {};\n", t.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")));
                }
                self.close("}", k);
            }
            "let" => {
                self.w(&format!("let {}: ", name));
                self.ty(&o["type"]);
                self.w(" = ");
                self.expr(&o["value"], Level::Pick);
                self.w(";");
            }
            "tokens" => {
                self.w(&format!("tokens {} ", name));
                let k = self.open("{");
                self.nl();
                let lines = o["lines"].as_array().cloned().unwrap_or_default();
                self.lines(&lines, |w, l| {
                    let token = w.name(l["token"].as_str().unwrap_or(""));
                    w.w(&format!("{}.{} = ", token, l["case"].as_str().unwrap_or("")));
                    w.expr(&l["value"], Level::Pick);
                    w.w(";");
                });
                self.close("}", k);
            }
            "pack" => {
                let package = id.split('/').next().unwrap_or("");
                self.w(&format!("pack {}@{} ", package, o["version"]));
                let k = self.open("{");
                self.nl();
                let uses: Vec<String> = o["uses"].as_array().into_iter().flatten().map(|u| format!("{}@{}", u["name"].as_str().unwrap_or(""), u["version"])).collect();
                if !uses.is_empty() {
                    self.part_above(o, "uses");
                    self.w(&format!("uses {};\n", uses.join(", ")));
                }
                let provides: Vec<&str> = o["provides"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                if !provides.is_empty() {
                    self.part_above(o, "provides");
                    self.w(&format!("provides {};\n", provides.join(", ")));
                }
                let implements: Vec<&str> = o["implements"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                if !implements.is_empty() {
                    self.part_above(o, "implements");
                    self.w(&format!("implements {};\n", implements.join(", ")));
                }
                for i in o["comment_items"].as_array().into_iter().flatten() {
                    self.item(i);
                }
                self.close("}", k);
            }
            "migration" => {
                let (from, to) = (&o["from"], &o["to"]);
                self.w(&format!("migration {}@{} to {}@{} ", from["name"].as_str().unwrap_or(""), from["version"], to["name"].as_str().unwrap_or(""), to["version"]));
                let k = self.open("{");
                self.nl();
                let mut items: Vec<(u64, &str, Value)> = Vec::new();
                for (key, part) in [("fns", "fn"), ("blocks", "block"), ("comment_items", "item")] {
                    for v in o[key].as_array().into_iter().flatten() {
                        items.push((start(v), part, v.clone()));
                    }
                }
                items.sort_by_key(|(at, _, _)| *at);
                let mut first = true;
                for (_, part, v) in items {
                    if part == "item" {
                        self.item(&v);
                        continue;
                    }
                    if !first {
                        self.body_blank();
                    }
                    first = false;
                    let c = comments_of(&v);
                    self.above(&c);
                    self.enter(&c);
                    if part == "fn" {
                        self.fn_decl(&v);
                    } else {
                        let target = v["target"].as_str().unwrap_or("");
                        let path = target.split_once('/').map_or(target, |(_, p)| p);
                        self.w(&format!("{} {} ", v["kind"].as_str().unwrap_or(""), path));
                        self.migration_body(&v["body"]);
                    }
                    self.leave();
                    self.trailing(&c);
                    self.nl();
                }
                self.close("}", k);
            }
            "migration_fn" => {
                self.w(&format!("migration fn {}", name));
                self.params(&o["params"]);
                self.w(" ");
                self.migration_body(&o["body"]);
            }
            other => self.fail(format!("a declaration of a kind this xtmd can't write back: {}", other)),
        }
    }

    fn generics(&mut self, o: &Value) {
        self.generics_of(&o["generics"]);
    }

    fn generics_of(&mut self, g: &Value) {
        let Some(g) = g.as_array().filter(|g| !g.is_empty()) else { return };
        let parts: Vec<String> = g
            .iter()
            .map(|p| {
                let bounds: Vec<String> = p["bounds"].as_array().into_iter().flatten().map(|b| self.type_text(b)).collect();
                if bounds.is_empty() {
                    p["name"].as_str().unwrap_or("").to_string()
                } else {
                    format!("{}: {}", p["name"].as_str().unwrap_or(""), bounds.join(" + "))
                }
            })
            .collect();
        self.w(&format!("<{}>", parts.join(", ")));
    }

    fn implements(&mut self, o: &Value) {
        let Some(list) = o["implements"].as_array().filter(|l| !l.is_empty()) else { return };
        let parts: Vec<String> = list.iter().map(|t| self.type_text(t)).collect();
        self.w(&format!(" implements {}", parts.join(", ")));
    }

    fn cases(&mut self, o: &Value) {
        let cases = o["cases"].as_array().cloned().unwrap_or_default();
        self.list("{ ", " }", &cases, |w, c| {
            w.w(c["name"].as_str().unwrap_or(""));
            if let Some(fields) = c["fields"].as_array() {
                let fields = fields.clone();
                w.list("(", ")", &fields, |w, f| w.field(f));
            }
        });
    }

    /// A body's members, each kind in its list, back in the order they were written.
    fn members(&mut self, o: &Value, kinds: &[(&str, &'static str)]) {
        let k = self.open("{");
        self.nl();
        self.member_lines(o, kinds);
        self.close("}", k);
    }

    fn member_lines(&mut self, o: &Value, kinds: &[(&str, &'static str)]) {
        let mut items: Vec<(u64, &str, Value)> = Vec::new();
        for (key, part) in kinds {
            for v in o[*key].as_array().into_iter().flatten() {
                items.push((start(v), part, v.clone()));
            }
        }
        for v in o["comment_items"].as_array().into_iter().flatten() {
            items.push((start(v), "item", v.clone()));
        }
        items.sort_by_key(|(at, _, _)| *at);
        let mut last = "";
        for (_, part, v) in items {
            if part == "item" {
                self.item(&v);
                continue;
            }
            self.gap(last, part);
            last = part;
            let c = comments_of(&v);
            self.above(&c);
            self.enter(&c);
            match part {
                "field" => {
                    self.field(&v);
                    self.w(";");
                }
                "require" => self.require(&v),
                "sig" => {
                    self.sig(&v);
                    self.w(";");
                }
                "event" => self.event(&v),
                _ => {}
            }
            self.free(&c);
            self.leave();
            self.trailing(&c);
            self.nl();
        }
    }

    /// The comments of a member written into its owner: `view`, `markdown`, `tokens`, `uses`.
    fn part_above(&mut self, o: &Value, part: &str) -> Value {
        let c = o["part_comments"].as_array().into_iter().flatten().find(|p| p["part"] == part).map(|p| p["comments"].clone()).unwrap_or(Value::Null);
        self.above(&c);
        c
    }

    fn field(&mut self, f: &Value) {
        if f["mutable"] == true && !f["id"].as_str().unwrap_or("").starts_with("$state.") {
            self.w("mut ");
        }
        self.w(f["name"].as_str().unwrap_or(""));
        self.w(": ");
        self.ty(&f["type"]);
        if !f["default"].is_null() {
            self.w(" = ");
            self.expr(&f["default"], Level::Pick);
        }
    }

    fn require(&mut self, r: &Value) {
        self.w(if r["warn"] == true { "warn " } else { "require " });
        self.w(&quote(r["message"].as_str().unwrap_or("")));
        self.w(" ");
        self.block(&r["body"]);
    }

    fn event(&mut self, e: &Value) {
        self.w("event ");
        self.w(e["name"].as_str().unwrap_or(""));
        self.params(&e["params"]);
        self.w(";");
    }

    fn sig(&mut self, s: &Value) {
        self.w("fn ");
        let id = s["id"].as_str().unwrap_or("");
        if let Some(on) = s["on"].as_str() {
            self.w(&self.name(on));
            self.generics_of(&s["on_generics"]);
            self.w(".");
        }
        self.w(s["name"].as_str().unwrap_or(Self::last(id)));
        self.generics(s);
        self.params(&s["params"]);
        if !s["returns"].is_null() {
            self.w(": ");
            self.ty(&s["returns"]);
        }
    }

    fn fn_decl(&mut self, f: &Value) {
        self.sig(f);
        if !f["body"].is_null() {
            self.w(" ");
            self.block(&f["body"]);
        }
    }

    fn params(&mut self, ps: &Value) {
        let ps = ps.as_array().cloned().unwrap_or_default();
        self.list("(", ")", &ps, |w, p| {
            if p["mutable"] == true {
                w.w("mut ");
            }
            if p["self"] == true {
                w.w("self");
                return;
            }
            w.w(p["name"].as_str().unwrap_or(""));
            if !p["type"].is_null() {
                w.w(": ");
                w.ty(&p["type"]);
            }
        });
    }

    fn widget(&mut self, external: bool, name: &str, o: &Value) {
        if external {
            self.w("extern ");
        }
        self.w(&format!("widget {}", name));
        self.generics(o);
        let inputs: Vec<Value> = o["inputs"].as_array().into_iter().flatten().filter(|i| i["param"] != true).cloned().collect();
        self.list("(", ")", &inputs, |w, f| w.field(f));
        self.implements(o);
        // an extern widget with no Markdown is a surface's, which ends at `;` when it has nothing else
        let lists = ["slots", "events", "requires", "own", "lets", "fns", "tokens", "uses", "comment_items"];
        let params = o["inputs"].as_array().into_iter().flatten().any(|i| i["param"] == true);
        let written = |m: &str| !o[m].is_null();
        if external && !params && lists.iter().all(|m| o[*m].as_array().map_or(true, |a| a.is_empty())) && !written("view") && !written("markdown") && !written("piece") {
            self.w(";");
            return;
        }
        let k = self.open(" {");
        self.nl();
        // the body's members in the order the grammar has them, each kind in the order written
        let params: Vec<Value> = o["inputs"].as_array().into_iter().flatten().filter(|i| i["param"] == true).cloned().collect();
        let mut items: Vec<(u64, &str, Value)> = params.into_iter().map(|p| (start(&p), "param", p)).collect();
        for (key, part) in [("slots", "slot"), ("events", "event"), ("requires", "require"), ("own", "own"), ("lets", "let"), ("fns", "fn"), ("comment_items", "item")] {
            for v in o[key].as_array().into_iter().flatten() {
                items.push((start(v), part, v.clone()));
            }
        }
        items.sort_by_key(|(at, _, _)| *at);
        let tokens = o["tokens"].as_array().cloned();
        let uses = o["uses"].as_array().cloned();
        let mut looks_written = false;
        let write_looks = |w: &mut Self| {
            if let Some(t) = &tokens {
                w.part_above(o, "tokens");
                w.w(&format!("tokens {};\n", t.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")));
            }
            if let Some(u) = &uses {
                w.part_above(o, "uses");
                w.w(&format!("uses {};\n", u.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")));
            }
        };
        let mut last = "";
        for (_, part, v) in items {
            if part == "item" {
                self.item(&v);
                continue;
            }
            // an extern widget's looks and needs come before its rules
            if !looks_written && matches!(part, "require" | "own" | "let" | "fn") {
                write_looks(self);
                looks_written = true;
            }
            self.gap(last, part);
            last = part;
            let c = comments_of(&v);
            self.above(&c);
            self.enter(&c);
            match part {
                "param" => {
                    self.w("param ");
                    self.field(&v);
                    self.w(";");
                }
                "slot" => {
                    self.w(&format!("slot {}: ", v["name"].as_str().unwrap_or("")));
                    self.ty(&v["type"]);
                    self.w(";");
                }
                "event" => self.event(&v),
                "require" => self.require(&v),
                "own" => self.var("mut", &v),
                "let" => self.var("let", &v),
                _ => self.fn_decl(&v),
            }
            self.free(&c);
            self.leave();
            self.trailing(&c);
            self.nl();
        }
        if !looks_written {
            write_looks(self);
        }
        if let Some(view) = o["view"].as_array() {
            self.body_blank();
            let c = self.part_above(o, "view");
            self.enter(&c);
            self.w("view ");
            self.block_items(view);
            self.leave();
            self.trailing(&c);
            self.nl();
        }
        // markdown follows view directly, and anything else after a blank line
        if o.get("view").is_none() && (o.get("piece").is_some() || o.get("markdown").is_some()) {
            self.body_blank();
        }
        if let Some(p) = o["piece"].as_str() {
            self.part_above(o, "markdown");
            self.w(&format!("markdown #{};\n", p));
        } else if let Some(md) = o["markdown"].as_array() {
            let c = self.part_above(o, "markdown");
            self.enter(&c);
            if md.is_empty() {
                self.w("markdown { }");
            } else {
                self.w("markdown {\n");
                self.prose(md);
                self.w("}");
            }
            self.leave();
            self.trailing(&c);
            self.nl();
        }
        self.close("}", k);
    }

    fn var(&mut self, word: &str, v: &Value) {
        self.w(&format!("{} {}: ", word, v["name"].as_str().unwrap_or("")));
        self.ty(&v["type"]);
        self.w(" = ");
        self.expr(&v["value"], Level::Pick);
        self.w(";");
    }

    fn state(&mut self, s: &Value) {
        self.w("$state ");
        self.in_state = true;
        self.members(s, &[("fields", "field"), ("requires", "require")]);
        self.in_state = false;
    }

    fn expose(&mut self, e: &Value) {
        let channel = e["channel"].as_str().unwrap_or("");
        self.w(&format!("expose {} ", Self::last(channel)));
        let k = self.open("{");
        self.nl();
        let fns = e["fns"].as_array().cloned().unwrap_or_default();
        self.lines(&fns, |w, f| w.fn_decl(f));
        self.close("}", k);
    }

    fn migration_body(&mut self, body: &Value) {
        let k = self.open("{");
        self.nl();
        let items = body.as_array().cloned().unwrap_or_default();
        self.lines(&items, |w, s| {
            if let Some(b) = s.get("execute") {
                w.w("execute ");
                w.block_items(b.as_array().map(Vec::as_slice).unwrap_or(&[]));
            } else if let Some(e) = s.get("event") {
                w.w(&format!("event {} ", e["name"].as_str().unwrap_or("")));
                w.migration_body(&e["body"]);
            } else if let Some(c) = s.get("call") {
                w.w(c["name"].as_str().unwrap_or(""));
                let args = c["args"].as_array().cloned().unwrap_or_default();
                w.list("(", ")", &args, |w, a| {
                    w.named(a);
                });
                w.w(";");
            } else if let Some(st) = s.get("stmt") {
                w.stmt_kind(st);
            }
        });
        self.close("}", k);
    }

    // ---- prose

    /// Prose: Markdown runs, calls and control lines, one after another with a blank line between; a comment
    /// line has none, since a blank line beside it is the run's own.
    fn prose(&mut self, items: &[Value]) {
        for (k, b) in items.iter().enumerate() {
            let comment = |v: &Value| v["comment"].is_string();
            if k > 0 && !comment(b) && !comment(&items[k - 1]) {
                self.nl();
            }
            self.prose_item(b);
        }
    }

    /// A block of prose one step in, closed by `}` on a line of its own.
    fn prose_block(&mut self, items: &[Value]) {
        self.w(" {\n");
        self.indent.push_str("    ");
        self.prose(items);
        self.indent.truncate(self.indent.len() - 4);
        self.w(&self.indent.clone());
        self.w("}");
    }

    fn prose_item(&mut self, b: &Value) {
        let indent = self.indent.clone();
        if let Some(m) = b.get("markdown") {
            let holes = m["holes"].as_array().cloned().unwrap_or_default();
            let mut next = holes.iter();
            for line in m["markdown"].as_str().unwrap_or("").split('\n') {
                if !line.trim().is_empty() {
                    self.w(&indent);
                }
                for (k, piece) in line.split('\u{fffc}').enumerate() {
                    if k > 0 {
                        self.w("\\{");
                        if let Some(h) = next.next() {
                            self.flat += 1;
                            self.expr(h, Level::Pick);
                            self.flat -= 1;
                        }
                        self.w("}");
                    }
                    self.w(piece);
                }
                self.nl();
            }
            return;
        }
        if let Some(t) = b["comment"].as_str() {
            self.w(&indent);
            self.w(t);
            self.nl();
            return;
        }
        self.w(&indent);
        let c = b["comments"].clone();
        self.enter(&c);
        if let Some(u) = b.get("call") {
            self.w("@");
            self.w(&self.name(u["widget"].as_str().unwrap_or("")));
            let args = u["args"].as_array().cloned().unwrap_or_default();
            if !args.is_empty() {
                self.list("(", ")", &args, |w, a| {
                    w.named(a);
                });
            }
            let slots = u["slots"].as_array().cloned().unwrap_or_default();
            if slots.is_empty() {
                self.w(";");
            } else {
                let mut inner: Vec<Value> = Vec::new();
                for s in &slots {
                    if s["children"] == true {
                        inner.extend(s["prose"].as_array().cloned().unwrap_or_default());
                    } else {
                        inner.push(json!({ "slot": { "slot": Self::last(s["slot"].as_str().unwrap_or("")), "prose": s["prose"] } }));
                    }
                }
                self.prose_block(&inner);
            }
        } else if let Some(i) = b.get("if") {
            for (k, br) in i["branches"].as_array().into_iter().flatten().enumerate() {
                self.w(if k == 0 { "@if (" } else { " else if (" });
                self.cond(br);
                self.w(")");
                self.prose_block(br["prose"].as_array().map(Vec::as_slice).unwrap_or(&[]));
            }
            if let Some(e) = i["else"].as_array() {
                self.w(" else");
                self.prose_block(e);
            }
        } else if let Some(f) = b.get("for") {
            self.w(if f["mutable"] == true { "@mut for (" } else { "@for (" });
            if let Some(key) = f["key"].as_str() {
                self.w(key);
                self.w(", ");
            }
            self.w(f["name"].as_str().unwrap_or(""));
            self.w(" in ");
            self.expr(&f["over"], Level::Pick);
            self.w(")");
            self.prose_block(f["prose"].as_array().map(Vec::as_slice).unwrap_or(&[]));
        } else if let Some(m) = b.get("match") {
            self.w("@match (");
            self.expr(&m["subject"], Level::Pick);
            self.w(") {\n");
            for case in m["cases"].as_array().into_iter().flatten() {
                self.w(&indent);
                self.w("    case ");
                self.patterns(&case["patterns"]);
                self.indent.push_str("    ");
                self.prose_block(case["prose"].as_array().map(Vec::as_slice).unwrap_or(&[]));
                self.indent.truncate(indent.len());
                self.nl();
            }
            self.w(&indent);
            self.w("}");
        } else if let Some(sl) = b.get("slot") {
            self.w(&format!("@slot {}", sl["slot"].as_str().unwrap_or("")));
            self.prose_block(sl["prose"].as_array().map(Vec::as_slice).unwrap_or(&[]));
        } else {
            self.fail(format!("a part of prose of a kind this xtmd can't write back: {}", b));
        }
        self.leave();
        self.nl();
    }

    // ---- statements

    fn block(&mut self, b: &Value) {
        self.block_items(b.as_array().map(Vec::as_slice).unwrap_or(&[]));
    }

    /// A lambda's or a case's block: on one line when nothing in it has a comment, which fmt keeps when
    /// its statements are simple.
    fn short_block(&mut self, b: &Value) {
        let stmts = b.as_array().cloned().unwrap_or_default();
        let next = self.nodes.last().map_or(usize::MAX, |n| n.next);
        let bracketed = self.nodes.last().map_or(false, |n| n.starts.iter().chain(n.ends.iter()).any(|(at, _)| *at == next));
        let quiet = !bracketed && !stmts.is_empty() && stmts.iter().all(|s| s["comment"].is_null() && comments_of(s).is_null());
        if !quiet {
            self.block(b);
            return;
        }
        let k = self.open("{ ");
        for (i, s) in stmts.iter().enumerate() {
            if i > 0 {
                self.w(" ");
            }
            self.stmt_kind(s);
        }
        self.close(" }", k);
    }

    fn block_items(&mut self, stmts: &[Value]) {
        let k = self.open("{");
        self.nl();
        let stmts = stmts.to_vec();
        self.lines(&stmts, |w, s| w.stmt_kind(s));
        self.close("}", k);
    }

    fn stmt_kind(&mut self, s: &Value) {
        let Some((kind, v)) = s.as_object().and_then(|m| m.iter().find(|(k, _)| !matches!(k.as_str(), "span" | "comments"))) else { return };
        let c = s["comments"].clone();
        match kind.as_str() {
            "let" => self.var("let", v),
            "mut" => self.var("mut", v),
            "return" => {
                self.w("return");
                if !v["value"].is_null() {
                    self.w(" ");
                    self.expr(&v["value"], Level::Pick);
                }
                self.free(&c);
                self.w(";");
            }
            "invoke" => {
                self.w(&format!("invoke {}", v["event"].as_str().unwrap_or("")));
                let args = v["args"].as_array().cloned().unwrap_or_default();
                self.list("(", ")", &args, |w, a| w.expr(a, Level::Pick));
                self.w(";");
            }
            "if" => {
                for (k, b) in v["branches"].as_array().into_iter().flatten().enumerate() {
                    self.w(if k == 0 { "if (" } else { " else if (" });
                    self.cond(b);
                    self.w(") ");
                    self.block(&b["body"]);
                }
                if !v["else"].is_null() {
                    self.w(" else ");
                    self.block(&v["else"]);
                }
            }
            "for" => {
                if v["mutable"] == true {
                    self.w("mut ");
                }
                self.w("for (");
                if let Some(key) = v["key"].as_str() {
                    self.w(key);
                    self.w(", ");
                }
                self.w(v["name"].as_str().unwrap_or(""));
                self.w(" in ");
                self.expr(&v["over"], Level::Pick);
                self.w(") ");
                self.block(&v["body"]);
            }
            "match" => {
                self.w("match (");
                self.expr(&v["subject"], Level::Pick);
                self.w(") ");
                let k = self.open("{");
                self.nl();
                let cases = v["cases"].as_array().cloned().unwrap_or_default();
                self.lines(&cases, |w, c| {
                    w.w("case ");
                    w.patterns(&c["patterns"]);
                    w.w(" ");
                    w.short_block(&c["body"]);
                });
                self.close("}", k);
            }
            "use" => {
                self.use_(v);
                if v["slots"].as_array().map_or(true, |s| s.is_empty()) {
                    self.free(&c);
                    self.w(";");
                }
            }
            "expr" => {
                self.expr(v, Level::Pick);
                self.free(&c);
                self.w(";");
            }
            "set" => {
                self.expr(&v["place"], Level::Pick);
                self.w(" = ");
                self.expr(&v["value"], Level::Pick);
                self.free(&c);
                self.w(";");
            }
            other => self.fail(format!("a statement of a kind this xtmd can't write back: {}", other)),
        }
    }

    fn cond(&mut self, b: &Value) {
        if let Some(o) = b.get("opens") {
            self.w(if o["mutable"] == true { "mut " } else { "let " });
            self.w(&format!("{}: ", o["name"].as_str().unwrap_or("")));
            self.ty(&o["type"]);
            self.w(" = ");
            self.expr(&o["value"], Level::Pick);
        } else {
            self.expr(&b["cond"], Level::Pick);
        }
    }

    fn patterns(&mut self, ps: &Value) {
        let ps = ps.as_array().cloned().unwrap_or_default();
        if ps.is_empty() {
            self.w("else");
            return;
        }
        let parts: Vec<String> = ps
            .iter()
            .map(|p| {
                if let Some(c) = p.get("case") {
                    let mut s = format!("{}.{}", self.name(c["enum"].as_str().unwrap_or("")), c["name"].as_str().unwrap_or(""));
                    if let Some(b) = c["binds"].as_array() {
                        s.push_str(&format!("({})", b.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")));
                    }
                    s
                } else if let Some(n) = p["num"].as_str() {
                    n.to_string()
                } else if let Some(t) = p["text"].as_str() {
                    quote(t)
                } else {
                    p["bool"].to_string()
                }
            })
            .collect();
        self.w(&parts.join(", "));
    }

    // ---- uses

    fn use_(&mut self, u: &Value) {
        self.w("@");
        self.w(&self.name(u["widget"].as_str().unwrap_or("")));
        let args = u["args"].as_array().cloned().unwrap_or_default();
        if !args.is_empty() {
            self.list("(", ")", &args, |w, a| {
                w.named(a);
            });
        }
        let slots = u["slots"].as_array().cloned().unwrap_or_default();
        if slots.is_empty() {
            return;
        }
        self.w(" ");
        if slots.len() == 1 && slots[0]["children"] == true {
            self.block(&slots[0]["view"]);
            return;
        }
        let k = self.open("{");
        self.nl();
        let mut items: Vec<Value> = slots.clone();
        items.extend(u["comment_items"].as_array().cloned().unwrap_or_default());
        items.sort_by_key(start);
        self.lines(&items, |w, s| {
            w.w(Self::last(s["slot"].as_str().unwrap_or("")));
            w.w(" ");
            w.block(&s["view"]);
        });
        self.close("}", k);
    }

    // ---- expressions

    fn expr(&mut self, e: &Value, at: Level) {
        let c = e["comments"].clone();
        self.free(&c);
        let Some((kind, v)) = e.as_object().and_then(|m| m.iter().find(|(k, _)| !matches!(k.as_str(), "span" | "type" | "comments"))) else {
            self.fail("an expression with nothing in it".into());
            return;
        };
        let own = match kind.as_str() {
            "op" => level_of(v["op"].as_str().unwrap_or(""), v["args"].as_array().map_or(0, Vec::len)),
            "pick" => Level::Pick,
            "range" => Level::Unary,
            "lambda" => Level::Pick,
            "try" => Level::Unary,
            _ => Level::Postfix,
        };
        let bracket = own < at;
        if bracket {
            self.w("(");
        }
        self.expr_kind(kind, v);
        if bracket {
            self.w(")");
        }
    }

    fn expr_kind(&mut self, kind: &str, v: &Value) {
        match kind {
            "num" | "duration" => self.w(v.as_str().unwrap_or("")),
            "bool" => self.w(&v.to_string()),
            "text" => self.w(&quote(v.as_str().unwrap_or(""))),
            "template" => {
                self.w("\"");
                let pieces = v["pieces"].as_array().cloned().unwrap_or_default();
                let holes = v["holes"].as_array().cloned().unwrap_or_default();
                for (k, p) in pieces.iter().enumerate() {
                    self.w(&escape(p.as_str().unwrap_or("")));
                    if let Some(h) = holes.get(k) {
                        self.w("\\{");
                        self.flat += 1;
                        self.expr(h, Level::Pick);
                        self.flat -= 1;
                        self.w("}");
                    }
                }
                self.w("\"");
            }
            "get" => {
                if v["self"] == true {
                    self.w("self");
                } else if let Some(s) = v["state"].as_str().filter(|s| self.in_state && !s.is_empty()) {
                    self.w(s);
                } else if let Some(s) = v["state"].as_str() {
                    self.w(if s.is_empty() { "$state".to_string() } else { format!("$state.{}", s) }.as_str());
                } else if let Some(l) = v["let"].as_str() {
                    self.w(&self.name(l));
                } else if let Some(x) = v["extern"].as_str() {
                    // a channel's value: `channel.name`
                    self.w(&self.name(x));
                } else {
                    let n = ["local", "input", "own", "slot"].iter().find_map(|k| v[*k].as_str()).unwrap_or("");
                    self.w(n);
                }
            }
            "list" => {
                let items = v.as_array().cloned().unwrap_or_default();
                self.list("[", "]", &items, |w, x| w.expr(x, Level::Pick));
            }
            "record" => {
                if let Some(of) = v["of"].as_str() {
                    self.w(&self.name(of));
                    self.w(" ");
                }
                self.inits(v);
            }
            "with" => {
                self.expr(&v["of"], Level::Postfix);
                self.w(" with ");
                self.inits(v);
            }
            "field" => {
                self.expr(&v["of"], Level::Postfix);
                self.w(".");
                self.w(v["field"].as_str().unwrap_or(""));
            }
            "case" => {
                self.w(&format!("{}.{}", self.name(v["of"].as_str().unwrap_or("")), v["case"].as_str().unwrap_or("")));
                if let Some(args) = v["args"].as_array() {
                    let args = args.clone();
                    self.list("(", ")", &args, |w, a| w.expr(a, Level::Pick));
                }
            }
            "none" => self.w("opt.NONE"),
            "some" => {
                self.w("opt.SOME");
                self.list("(", ")", &[v.clone()], |w, a| w.expr(a, Level::Pick));
            }
            "index" => {
                self.expr(&v["of"], Level::Postfix);
                self.w("[");
                self.expr(&v["index"], Level::Pick);
                self.w("]");
            }
            "call" => {
                let id = v["fn"].as_str().unwrap_or("");
                let name = self.fn_name(id);
                if !v["self"].is_null() {
                    self.expr(&v["self"], Level::Postfix);
                    self.w(".");
                } else if let Some((_, rest)) = id.split_once('/') {
                    // a fn called on a type: `text.from(…)`
                    if let Some((ty, _)) = rest.rsplit_once('.') {
                        self.w(&self.name(&format!("{}/{}", id.split('/').next().unwrap_or(""), ty)));
                        self.w(".");
                    }
                }
                self.w(&name);
                let args = v["args"].as_array().cloned().unwrap_or_default();
                self.list("(", ")", &args, |w, a| w.expr(a, Level::Pick));
            }
            "apply" => {
                self.expr(&v["fn"], Level::Postfix);
                let args = v["args"].as_array().cloned().unwrap_or_default();
                self.list("(", ")", &args, |w, a| w.expr(a, Level::Pick));
            }
            "op" => {
                let op = v["op"].as_str().unwrap_or("");
                let args = v["args"].as_array().cloned().unwrap_or_default();
                if args.len() == 1 {
                    self.w(op);
                    self.expr(&args[0], Level::Unary);
                } else if args.len() == 2 {
                    let own = level_of(op, 2);
                    // left to right: a right operand at the same level is bracketed, and the
                    // operators that don't chain bracket both
                    let chains = matches!(own, Level::Or | Level::And | Level::Sum | Level::Term);
                    let left = if chains { own } else { next(own) };
                    self.expr(&args[0], left);
                    self.w(&format!(" {} ", op));
                    self.expr(&args[1], next(own));
                }
            }
            "pick" => {
                self.expr(&v["cond"], Level::Or);
                self.w(" ? ");
                self.expr(&v["then"], Level::Pick);
                self.w(" : ");
                self.expr(&v["else"], Level::Pick);
            }
            "range" => {
                let bound = |w: &mut Self, b: &Value| {
                    let simple = b.get("num").is_some() || b.get("duration").is_some() || b.get("get").is_some();
                    // a minus in front of a number that is a bound is that number's sign
                    let negative = b["op"]["op"] == "-" && b["op"]["args"].as_array().map_or(false, |a| a.len() == 1 && a[0].get("num").is_some());
                    if negative {
                        w.w("-");
                        w.expr(&b["op"]["args"][0], Level::Postfix);
                    } else if simple {
                        w.expr(b, Level::Postfix);
                    } else {
                        w.w("(");
                        w.expr(b, Level::Pick);
                        w.w(")");
                    }
                };
                match (v.get("low"), v.get("high")) {
                    (Some(l), Some(h)) => {
                        bound(self, l);
                        self.w("..");
                        bound(self, h);
                    }
                    (Some(l), None) => {
                        bound(self, l);
                        self.w("...");
                    }
                    (None, Some(h)) => {
                        self.w("...");
                        bound(self, h);
                    }
                    (None, None) => self.w("..."),
                }
                if let Some(s) = v.get("step") {
                    self.w(" step ");
                    bound(self, s);
                }
            }
            "try" => {
                self.w("try ");
                self.expr(v, Level::Postfix);
            }
            "lambda" => {
                self.w("fn");
                self.params(&v["params"]);
                if !v["returns"].is_null() {
                    self.w(": ");
                    self.ty(&v["returns"]);
                }
                self.w(" ");
                self.short_block(&v["body"]);
            }
            "use" => self.use_(v),
            other => self.fail(format!("an expression of a kind this xtmd can't write back: {}", other)),
        }
    }

    /// A widget's argument, `name = value`, or a value alone for a fn called as a widget is.
    fn named(&mut self, a: &Value) {
        if let Some(n) = a["name"].as_str().filter(|n| !n.is_empty()) {
            self.w(&format!("{} = ", n));
        }
        self.expr(&a["value"], Level::Pick);
    }

    fn inits(&mut self, v: &Value) {
        let fields = v["fields"].as_array().cloned().unwrap_or_default();
        self.list("{ ", " }", &fields, |w, f| {
            if f["mutable"] == true {
                w.w("mut ");
            }
            w.w(&format!("{} = ", f["name"].as_str().unwrap_or("")));
            w.expr(&f["value"], Level::Pick);
        });
    }

    // ---- types

    fn ty(&mut self, t: &Value) {
        let s = self.type_text(t);
        self.w(&s);
    }

    fn type_text(&self, t: &Value) -> String {
        let name = t["name"].as_str().unwrap_or("");
        let mut s = match name {
            "fn" => {
                let ps: Vec<String> = t["params"].as_array().into_iter().flatten().map(|p| format!("{}{}: {}", if p["mut"] == true { "mut " } else { "" }, p["name"].as_str().unwrap_or(""), self.type_text(&p["type"]))).collect();
                let mut s = format!("fn({})", ps.join(", "));
                if !t["returns"].is_null() {
                    s.push_str(&format!(": {}", self.type_text(&t["returns"])));
                }
                s
            }
            "record" => {
                let fs: Vec<String> = t["fields"].as_array().into_iter().flatten().map(|f| format!("{}{}: {}", if f["mut"] == true { "mut " } else { "" }, f["name"].as_str().unwrap_or(""), self.type_text(&f["type"]))).collect();
                format!("{{ {} }}", fs.join(", "))
            }
            "any" => format!("any {}", self.name(t["interface"].as_str().unwrap_or(""))),
            _ => {
                let mut s = self.name(name);
                if let Some(args) = t["args"].as_array() {
                    let a: Vec<String> = args.iter().map(|x| self.type_text(x)).collect();
                    s.push_str(&format!("<{}>", a.join(", ")));
                }
                s
            }
        };
        let c = &t["contract"];
        if let Some(l) = c.get("length") {
            s.push_str(&format!("[{}]", self.bounds(l, true)));
        }
        if let Some(r) = c.get("range") {
            s.push(' ');
            s.push_str(&self.bounds(r, false));
            if let Some(st) = c.get("step") {
                s.push_str(&format!(" step {}", self.ir_value(st)));
            }
        } else if let Some(st) = c.get("step") {
            s.push_str(&format!(" step {}", self.ir_value(st)));
        }
        if let Some(only) = c["only"].as_array() {
            s.push_str(&format!(" only({})", only.iter().map(|v| self.ir_value(v)).collect::<Vec<_>>().join(", ")));
        }
        s
    }

    /// A contract's bounds: `0..10`, `1...`, `...10`, or a length's `3`.
    fn bounds(&self, b: &Value, length: bool) -> String {
        let (low, high) = (b.get("low").map(|v| self.ir_value(v)), b.get("high").map(|v| self.ir_value(v)));
        match (low, high) {
            (Some(l), Some(h)) if length && l == h => l,
            (Some(l), Some(h)) => format!("{}..{}", l, h),
            (Some(l), None) => format!("{}...", l),
            (None, Some(h)) => format!("...{}", h),
            (None, None) => "...".into(),
        }
    }

    /// A value in a contract, as the IR has it: a constant, worked out, written as a value of its type.
    fn ir_value(&self, v: &Value) -> String {
        if let Some(n) = v["num"].as_str() {
            return n.to_string();
        }
        if let Some(d) = v["duration"].as_str() {
            return format!("{}ms", d);
        }
        if let Some(t) = v["text"].as_str() {
            return quote(t);
        }
        if let Some(b) = v["bool"].as_bool() {
            return b.to_string();
        }
        if let Some(t) = v["token"].as_str() {
            return self.name(t);
        }
        if let Some(l) = v["list"].as_array() {
            return format!("[{}]", l.iter().map(|x| self.ir_value(x)).collect::<Vec<_>>().join(", "));
        }
        if v.get("none").is_some() {
            return "opt.NONE".into();
        }
        if let Some(x) = v.get("some") {
            // a value where an opt is taken is that opt's SOME
            return self.ir_value(x);
        }
        if let Some(r) = v.get("record") {
            let fields: Vec<String> = r["fields"].as_object().into_iter().flatten().map(|(k, x)| format!("{} = {}", k, self.ir_value(x))).collect();
            let ty = r["type"].as_str().unwrap_or("record");
            let body = if fields.is_empty() { "{ }".to_string() } else { format!("{{ {} }}", fields.join(", ")) };
            return if ty == "record" { body } else { format!("{} {}", self.name(ty), body) };
        }
        if let Some(t) = v.get("tokval") {
            return format!("{}.{}({})", self.name(t["type"].as_str().unwrap_or("")), t["name"].as_str().unwrap_or(""), self.ir_value(&t["value"]));
        }
        if let Some(c) = v.get("case") {
            let mut s = format!("{}.{}", self.name(c["enum"].as_str().unwrap_or("")), c["name"].as_str().unwrap_or(""));
            if let Some(values) = c["values"].as_object().filter(|m| !m.is_empty()) {
                s.push_str(&format!("({})", values.values().map(|x| self.ir_value(x)).collect::<Vec<_>>().join(", ")));
            }
            return s;
        }
        format!("({})", v)
    }
}

fn next(l: Level) -> Level {
    match l {
        Level::Pick => Level::Or,
        Level::Or => Level::And,
        Level::And => Level::Equal,
        Level::Equal => Level::Compare,
        Level::Compare => Level::Sum,
        Level::Sum => Level::Term,
        Level::Term => Level::Unary,
        _ => Level::Postfix,
    }
}

/// A text literal: its characters, with the escapes that need one.
fn quote(t: &str) -> String {
    format!("\"{}\"", escape(t))
}

fn escape(t: &str) -> String {
    let mut s = String::new();
    for c in t.chars() {
        match c {
            '\\' => s.push_str("\\\\"),
            '"' => s.push_str("\\\""),
            '\n' => s.push_str("\\n"),
            '\t' => s.push_str("\\t"),
            c if (c as u32) < 0x20 => s.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => s.push(c),
        }
    }
    s
}

/// Where a part of a resolved IR was written, to put the parts of a body back in their order; one a
/// step made, with no span, goes last.
fn start(v: &Value) -> u64 {
    let own = v["span"][0].as_u64();
    let inner = v.as_object().filter(|o| o.len() == 1).and_then(|o| o.values().next()).and_then(|x| x["span"][0].as_u64());
    own.or(inner).unwrap_or(u64::MAX)
}

/// A node's comments, beside its span or, on a declaration, inside its one key.
fn comments_of(v: &Value) -> Value {
    if !v["comments"].is_null() {
        return v["comments"].clone();
    }
    v.as_object().filter(|o| o.len() == 1).and_then(|o| o.values().next()).map(|x| x["comments"].clone()).unwrap_or(Value::Null)
}
