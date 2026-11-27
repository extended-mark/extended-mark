//! A pack's migration applied to one file (page 6), on what the compiler resolved of it, never on its
//! text: the file is compiled against the surface of the version it is written for, which gives its
//! resolved IR, where every use of the pack and every other reference to it is known by what it
//! names. The migration's statements change each of them in the resolved IR, in order, and decompile
//! prints the file again. A value a statement writes is made by the compiler too (values.rs), so a
//! use's own code goes where it went as a whole, in brackets where it needs them, and a name the
//! migration writes is the declaration it means, given its import.
//!
//! What a statement can't do to a use is the file's refusal, or with `--allow-todos`, the use left as it
//! was with a TODO above it.

pub mod api;
pub(crate) mod ops;
pub mod values;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use crate::check::{compile_rir, Checker, Source};
use crate::migrate::step::{ask, confirm, TODO};
use crate::migrate::Pack;
use crate::syntax::ast::{self, Expr, ExprKind, MigrationItem, Stmt};
use crate::syntax::printer::print_expr;
use crate::syntax::Lines;

use api::Decl;
use ops::{arg_name, bare, case_of, flatten, literal_text, member, place, why_of, Op};
use values::{constant, fill, printed, type_text, Probe, PROBE};

/// What applying one migration to one file came to.
#[derive(Default, Debug)]
pub struct Done {
    pub changes: usize,
    /// Each value and use a `drop` takes out, with its file and line.
    pub deletions: Vec<String>,
    pub notices: Vec<String>,
    pub refused: Vec<String>,
    /// What `--allow-todos` left for a person, each with a TODO above it.
    pub todos: Vec<String>,
    /// Each value a statement renamed, set or added, where, with the statement's why.
    pub changed: Vec<String>,
}

/// One file of `main`, the files compiled together (a document, or a pack's files), moved by one of
/// `pack`'s migrations from `pack@from`: its new text, when anything changed and it can be written,
/// and what the migration came to. `sources` is the rest of the build.
pub fn move_file(base: &Checker, sources: &[Source], pack: &Pack, from: u32, path: &str, main: &[Source], module: &str) -> (Option<String>, Done) {
    let mut done = Done::default();
    let world = crate::migrate::world_at(sources, pack, from);
    let text = main.iter().find(|s| s.path == path).map_or("", |s| s.text.as_str());
    let (compiled, rir) = compile_rir(base, &[world], main, module);
    let Some(rir) = rir.filter(|_| compiled.module.is_some()) else {
        let first = compiled.errors().find(|d| d.file == path).map(|d| d.diagnostic.message.clone()).unwrap_or_else(|| "an error in a pack".into());
        done.refused.push(format!("it doesn't compile against {}@{}, the version it is written for: {}", pack.name, from, first));
        return (None, done);
    };
    let Some(mut file) = rir["files"].as_array().into_iter().flatten().find(|f| f["path"] == path).cloned() else {
        done.refused.push("the compiler gave nothing for it".into());
        return (None, done);
    };
    let m = Moving::new(base, sources, pack, from, path, text);
    m.run(&mut file, &mut done);
    // a pack that uses this one uses the version it moves to
    for d in file.get_mut("declarations").and_then(Value::as_array_mut).into_iter().flatten() {
        for u in d.pointer_mut("/pack/uses").and_then(Value::as_array_mut).into_iter().flatten() {
            if u["name"] == pack.name.as_str() && u["version"] == from {
                u["version"] = json!(from + 1);
                done.changes += 1;
            }
        }
    }
    if (done.changes == 0 && done.todos.is_empty()) || !done.refused.is_empty() {
        return (None, done);
    }
    let printed = match crate::decompile::file(&file) {
        Ok(p) => p.text,
        Err(e) => {
            done.refused.push(format!("it can't be printed back: {}", e.join("; ")));
            return (None, done);
        }
    };
    // what is printed has to mean what the migration made, unless a TODO is left for a person to finish
    if done.todos.is_empty() && main.len() == 1 {
        let main: Vec<Source> = main.iter().map(|s| if s.path == path { Source { path: path.to_string(), text: printed.clone() } } else { s.clone() }).collect();
        let (c, again) = compile_rir(base, &[m.world_new.clone()], &main, module);
        let again = again.filter(|_| c.module.is_some()).and_then(|r| r["files"].as_array().into_iter().flatten().find(|f| f["path"] == path).cloned());
        match again {
            None => {
                let first = c.errors().find(|d| d.file == path).map(|d| d.diagnostic.message.clone()).unwrap_or_else(|| "an error in a pack".into());
                done.refused.push(format!("what {} makes of it doesn't compile: {}", m.label, first));
            }
            Some(again) => {
                if let Some(d) = crate::migrate::pipeline::first_difference(&crate::migrate::pipeline::written(&file), &crate::migrate::pipeline::written(&again), "") {
                    done.refused.push(format!("what was printed compiles to another resolved IR than {} made, at {}", m.label, d));
                }
            }
        }
    }
    (Some(printed), done)
}

/// One block of the migration, as it applies.
struct Block {
    /// `chart`, or `text.slug`.
    key: String,
    kind: String,
    /// `kit/chart`.
    full: String,
    ops: Vec<Op>,
    old: Decl,
    new: Decl,
    /// Its name once the block's statements are done, and the pack that has it: the migration's own,
    /// or one its file imports.
    now: String,
    now_pack: String,
    /// Whether a fn's statements change its arguments, which its uses as a value can't follow.
    args_change: bool,
    /// Each member's new name, by its old one.
    renamed: BTreeMap<String, String>,
    /// Members it drops, each with why, and whether it drops the whole declaration.
    dropped: BTreeMap<String, String>,
    gone: Option<String>,
    /// Each case's new name, by its old one; each dropped case, with why.
    cases: BTreeMap<String, String>,
    cases_gone: BTreeMap<String, String>,
}

pub struct Moving<'a> {
    pack: &'a Pack,
    /// The migration's file's imports: each pack, the one name it brings in or all, and its alias.
    imports: Vec<(String, Option<String>, Option<String>)>,
    /// The migration's file, and the pack's files at the version it moves to.
    step_text: &'a str,
    new_src: Vec<Source>,
    label: String,
    path: &'a str,
    lines: Lines<'a>,
    probe: Probe<'a>,
    world_new: Vec<Source>,
    blocks: Vec<Block>,
}

/// What the migration's code sees of a use: each member's value, what the use gives or the older
/// version's default, and which the use gives.
#[derive(Clone, Default)]
struct Env {
    given: BTreeMap<String, Value>,
    defaults: BTreeMap<String, String>,
    /// The file's own constant `let`s, by id.
    lets: BTreeMap<String, Value>,
    /// The types of an execute's own variables.
    types: BTreeMap<String, String>,
    /// The block's declaration, for what the run asks.
    key: String,
}

/// An execute's way out.
enum Flow {
    Next,
    Return(Option<Expr>),
    Unknown(String),
}

/// A use as the statements work on it.
#[derive(Clone)]
struct Use {
    /// Its widget's name, as the statements so far leave it.
    name: String,
    /// Each argument's object: its name, its value and its comments.
    args: Vec<Value>,
    /// Each slot's object, its name the slot's own.
    slots: Vec<(String, Value)>,
    /// Widgets put around it, outermost last: each one's name, arguments and slots.
    wrappers: Vec<(String, Vec<Value>, Vec<(String, Value)>)>,
    /// Taken out, with why.
    removed: Option<String>,
    /// Put in its place: the value an input of it was given, which is a view (`return @value;`).
    placed: Option<Value>,
    /// In prose, or in a view.
    prose: bool,
    span: Value,
}

impl Use {
    fn arg(&self, n: &str) -> Option<usize> {
        self.args.iter().position(|a| a["name"] == n)
    }
}

impl<'a> Moving<'a> {
    pub fn new(base: &'a Checker, sources: &'a [Source], pack: &'a Pack, from: u32, path: &'a str, text: &'a str) -> Moving<'a> {
        let to = from + 1;
        let (step, step_text) = &pack.steps[&from];
        let old: Vec<Source> = pack.surfaces.get(&from).cloned().into_iter().collect();
        let current = to >= pack.version || !pack.surfaces.contains_key(&to);
        let new: Vec<Source> = if current { pack.files.clone() } else { pack.surfaces.get(&to).cloned().into_iter().collect() };
        let world_new = if current { sources.to_vec() } else { crate::migrate::world_at(sources, pack, to) };
        let probe = Probe::new(base, world_new.clone(), &pack.name, step, step_text, new.clone());
        let step_text = step_text.as_str();
        // what the migration's file imports: a use can become one of another pack's
        let imports: Vec<(String, Option<String>, Option<String>)> = crate::syntax::parse(step_text, crate::syntax::Entry::Xtmd)
            .file
            .map(|f| f.items.iter().filter_map(|n| if let ast::Item::Import(i) = &n.item { Some((i.path[0].name.clone(), i.path.get(1).map(|x| x.name.clone()), i.alias.as_ref().map(|x| x.name.clone()))) } else { None }).collect())
            .unwrap_or_default();
        let mut blocks = Vec::new();
        for item in step.items.iter() {
            let MigrationItem::Block(b) = item else { continue };
            let key = b.path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join(".");
            let mut ops = Vec::new();
            flatten(&b.body, &pack.fns, &BTreeMap::new(), &mut ops, 0);
            let (mut now, mut renamed, mut dropped, mut gone, mut cases, mut cases_gone) = (key.clone(), BTreeMap::new(), BTreeMap::new(), None, BTreeMap::new(), BTreeMap::new());
            for op in &ops {
                match op {
                    Op::Call(o, args) => match (o.as_str(), member(args), arg_name(args, "to")) {
                        ("rename", Some(m), Some(to)) => {
                            let first = renamed.iter().find(|(_, v)| **v == m).map(|(k, _): (&String, &String)| k.clone()).unwrap_or(m);
                            renamed.insert(first, to);
                        }
                        ("rename", None, _) if args.iter().any(|(n, _)| n == "from") => {
                            let from = args.iter().find(|(n, _)| n == "from").and_then(|(_, v)| case_of(v));
                            let to = args.iter().find(|(n, _)| n == "to").and_then(|(_, v)| case_of(v));
                            if let (Some(f), Some(t)) = (from, to) {
                                cases.insert(f, t);
                            }
                        }
                        ("rename", None, Some(to)) => now = rename_key(&now, &to),
                        // a fn on a type that was renamed too: `to = rich_text.from`
                        ("rename", None, None) if ops::dotted(args, "to").is_some() => now = ops::dotted(args, "to").unwrap(),
                        ("drop", Some(m), _) => {
                            dropped.insert(m, why_of(args));
                        }
                        ("drop", None, _) => match args.iter().find(|(n, _)| n == "from").and_then(|(_, v)| case_of(v)) {
                            Some(c) => {
                                cases_gone.insert(c, why_of(args));
                            }
                            None => gone = Some(why_of(args)),
                        },
                        _ => {}
                    },
                    Op::Event(e, inner) => {
                        for i in inner {
                            if let Op::Call(o, a) = i {
                                if o == "rename" {
                                    if let Some(to) = arg_name(a, "to") {
                                        renamed.insert(e.clone(), to);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            let old_decl = api::decl(&old, &key).unwrap_or_default();
            // a name the newer version doesn't have is one of a pack the migration's file imports
            let (now_pack, new_decl) = match api::decl(&new, &now) {
                Some(d) => (pack.name.clone(), d),
                None => imports
                    .iter()
                    .filter(|(_, n, _)| n.as_deref().map_or(true, |n| n == now))
                    .find_map(|(p, _, _)| api::decl(&pack_files(sources, p), &now).map(|d| (p.clone(), d)))
                    .unwrap_or_else(|| (pack.name.clone(), Decl::default())),
            };
            let args_change = b.kind.name == "fn" && ops.iter().any(|o| matches!(o, Op::Call(n, a) if matches!(n.as_str(), "add" | "drop") && a.iter().any(|(k, _)| k == "arg")));
            blocks.push(Block { full: format!("{}/{}", pack.name, key), key, kind: b.kind.name.clone(), ops, old: old_decl, new: new_decl, now, now_pack, args_change, renamed, dropped, gone, cases, cases_gone });
        }
        Moving { pack, step_text, imports, new_src: new, label: format!("{}@{}", pack.name, to), path, lines: Lines::new(text), probe, world_new, blocks }
    }

    fn line(&self, span: &Value) -> usize {
        span[0].as_u64().map_or(0, |s| self.lines.place(s as u32).line as usize)
    }

    /// Every use and reference in the file changed, then the imports it needs.
    pub fn run(&self, file: &mut Value, done: &mut Done) {
        let lets: BTreeMap<String, Value> = file["declarations"].as_array().into_iter().flatten().filter_map(|d| Some((d["let"]["id"].as_str()?.to_string(), d["let"]["value"].clone()))).filter(|(_, v)| constant(v)).collect();
        let env = Env { lets, ..Env::default() };
        let packs: BTreeSet<String> = std::iter::once(self.pack.name.clone()).chain(self.imports.iter().map(|(p, _, _)| p.clone())).collect();
        let mut before = BTreeSet::new();
        ids(file, &packs, &mut before);
        let mut keys: Vec<String> = file.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        keys.retain(|k| k != "comments" && k != "left_out" && k != "imports");
        // in the order the file has them: its questions are numbered as they come up
        keys.sort_by_key(|k| first_start(&file[k.as_str()]).unwrap_or(u64::MAX));
        for k in keys {
            let left = match &mut file[k.as_str()] {
                Value::Array(a) => self.list(&k, a, &env, done),
                x => self.walk(x, &k, &json!([0]), &env, done),
            };
            // nothing above a declaration holds a TODO: said all the same
            for why in left {
                self.refuse(done, &json!([0]), why);
            }
        }
        self.imports(file, &before, done);
    }

    /// What a statement can't do: the file is refused, or with `--allow-todos` it is said, and the TODO
    /// goes above what holds it.
    fn cannot(&self, done: &mut Done, span: &Value, why: String, todo: String) -> Vec<String> {
        if TODO.load(std::sync::atomic::Ordering::Relaxed) {
            done.todos.push(format!("line {}: {}", self.line(span), todo));
            vec![todo]
        } else {
            done.refused.push(format!("line {}: {}", self.line(span), why));
            Vec::new()
        }
    }

    fn refuse(&self, done: &mut Done, span: &Value, why: String) {
        done.refused.push(format!("line {}: {}", self.line(span), why));
    }

    /// A list's items: each moved in the order it is written, so its questions come up in that order,
    /// then what goes in beside them, the last first, so what is put in before an item moves none
    /// still to come.
    fn list(&self, key: &str, items: &mut Vec<Value>, env: &Env, done: &mut Done) -> Vec<String> {
        let none = json!([0]);
        let walked: Vec<Vec<String>> = items.iter_mut().map(|x| self.walk(x, key, &none, env, done)).collect();
        let merge = matches!(key, "prose" | "markdown") && walked.iter().all(Vec::is_empty);
        let mut up = Vec::new();
        for (i, todos) in walked.into_iter().enumerate().rev() {
            // a use taken out whole stays as a comment
            if let Some(lines) = items[i].get("xtmd_kept").cloned() {
                let span = items[i]["span"].clone();
                let lines: Vec<String> = lines.as_array().into_iter().flatten().filter_map(|l| l.as_str().map(str::to_string)).collect();
                if key == "prose" || key == "markdown" {
                    items.remove(i);
                    for l in lines.into_iter().rev() {
                        items.insert(i, json!({ "comment": format!("@// {}", l), "span": span }));
                    }
                } else {
                    items[i] = json!({ "comment": lines.into_iter().map(|l| format!("// {}", l)).collect::<Vec<_>>(), "span": span });
                }
            }
            if todos.is_empty() {
                continue;
            }
            if crate::migrate::rir::v1::walk::holds(key, &items[i]) {
                for t in todos {
                    put_todo(items, i, key, &t);
                }
            } else {
                up.splice(0..0, todos);
            }
        }
        // a value placed as a hole is Markdown, and one run with the Markdown beside it, as the
        // printed file reads
        if merge {
            for i in (1..items.len()).rev() {
                let (Some(a), Some(b)) = (items[i - 1].get("markdown"), items[i].get("markdown")) else { continue };
                let text = format!("{}\n\n{}", a["markdown"].as_str().unwrap_or(""), b["markdown"].as_str().unwrap_or(""));
                let holes: Vec<Value> = a["holes"].as_array().into_iter().flatten().chain(b["holes"].as_array().into_iter().flatten()).cloned().collect();
                let span = json!([items[i - 1]["span"][0], items[i]["span"][1]]);
                items[i - 1] = json!({ "markdown": { "markdown": text, "holes": holes }, "span": span });
                items.remove(i);
            }
        }
        up
    }

    /// A node, what is in it first: each use inside a use is moved before the use that holds it.
    fn walk(&self, v: &mut Value, key: &str, at: &Value, env: &Env, done: &mut Done) -> Vec<String> {
        let mut up = Vec::new();
        // where it stands: its own span, or the nearest one around it
        let at = v.get("span").cloned().unwrap_or_else(|| at.clone());
        // a use or a record a block changes: what it is given first, then the use, which sets its members
        let planned = (matches!(key, "use" | "call") && v["widget"].as_str().map_or(false, |w| self.block(w, &["widget"]).is_some()))
            || (key == "record" && v.get("fields").is_some() && v["of"].as_str().map_or(false, |r| self.block(r, &["record"]).is_some()));
        if planned {
            for members in ["args", "fields"] {
                for a in v.get_mut(members).and_then(Value::as_array_mut).into_iter().flatten() {
                    let Some(x) = a.get_mut("value") else { continue };
                    up.extend(self.walk(x, "value", &at, env, done));
                }
            }
            for s in v.get_mut("slots").and_then(Value::as_array_mut).into_iter().flatten() {
                for k in ["prose", "view"] {
                    if let Some(items) = s.get_mut(k).and_then(Value::as_array_mut) {
                        up.extend(self.list(k, items, env, done));
                    }
                }
            }
            up.extend(self.node(v, key, &at, env, done));
            return up;
        }
        match v {
            Value::Object(o) => {
                let keys: Vec<String> = o.keys().filter(|k| *k != "comments" && *k != "left_out").cloned().collect();
                for k in keys {
                    let x = o.get_mut(&k).unwrap();
                    match x {
                        Value::Array(a) => up.extend(self.list(&k, a, env, done)),
                        x => up.extend(self.walk(x, &k, &at, env, done)),
                    }
                }
                up.extend(self.node(v, key, &at, env, done));
            }
            Value::Array(a) => {
                for x in a.iter_mut() {
                    up.extend(self.walk(x, key, &at, env, done));
                }
            }
            _ => {}
        }
        up
    }

    fn block(&self, id: &str, kinds: &[&str]) -> Option<&Block> {
        self.blocks.iter().find(|b| b.full == id && kinds.contains(&b.kind.as_str()))
    }

    /// One node: a use, a record written out, a fn's call or a case, as its block says; any other
    /// reference renamed where it is.
    fn node(&self, v: &mut Value, key: &str, at: &Value, env: &Env, done: &mut Done) -> Vec<String> {
        // a use taken out: the item that holds it is kept as a comment
        for k in ["use", "call"] {
            if let Some(kept) = v.get(k).and_then(|u| u.get("xtmd_kept")).cloned() {
                v["xtmd_kept"] = kept;
                return Vec::new();
            }
        }
        // a use whose value is placed in its stead: a hole in prose, `@x;` in a view, the value
        // itself where a value goes
        for k in ["use", "call"] {
            let Some(value) = v.get(k).and_then(|u| u.get("xtmd_placed")).cloned() else { continue };
            let span = v.get("span").cloned().unwrap_or_else(|| at.clone());
            *v = match key {
                "prose" | "markdown" => json!({ "markdown": { "markdown": "\u{fffc}", "holes": [value] }, "span": span }),
                "view" | "body" => match ["local", "input", "slot"].iter().find_map(|g| value.pointer(&format!("/get/{}", g)).and_then(Value::as_str)) {
                    Some(name) => json!({ "use": { "widget": name, "args": [], "span": span }, "span": span }),
                    None => {
                        let w = format!("{} places this use's value where it stands, and in a view only a value with a name is placed: give it one (`let x: rich_text = …;`), then `@x;`", self.label);
                        return self.cannot(done, &span, w.clone(), w);
                    }
                },
                _ => value,
            };
            return Vec::new();
        }
        if let Some(w) = v["widget"].as_str().map(str::to_string) {
            if let (Some(b), true) = (self.block(&w, &["widget"]), key == "use" || key == "call") {
                return self.a_use(v, b, key == "call", at, env, done);
            }
        }
        if let Some(r) = v["of"].as_str().map(str::to_string) {
            if let (Some(b), true, true) = (self.block(&r, &["record"]), key == "record", v.get("fields").is_some()) {
                return self.a_record(v, b, at, env, done);
            }
        }
        if let Some(f) = v["fn"].as_str().map(str::to_string) {
            if let (Some(b), "call") = (self.block(&f, &["fn"]), key) {
                return self.a_call(v, b, at, env, done);
            }
        }
        if key == "case" {
            let of = v["of"].as_str().or(v["enum"].as_str()).map(str::to_string);
            if let Some(b) = of.as_deref().and_then(|of| self.block(of, &["enum", "token"])) {
                return self.a_case(v, b, at, done);
            }
        }
        self.rename(v, at, done)
    }

    /// Every other reference, renamed where it is: a type, a field read, a fn given as a value, a
    /// member's target. One to what is gone is left for a person.
    fn rename(&self, v: &mut Value, at: &Value, done: &mut Done) -> Vec<String> {
        let Some(o) = v.as_object_mut() else { return Vec::new() };
        let span = at.clone();
        let mut todos = Vec::new();
        let mut short: Vec<(String, String)> = Vec::new();
        for (k, x) in o.iter_mut() {
            if matches!(k.as_str(), "text" | "markdown" | "comment" | "comments" | "raw") {
                continue;
            }
            let Some(s) = x.as_str() else { continue };
            for b in &self.blocks {
                if s == b.full {
                    if b.args_change {
                        let w = format!("{} changes {}'s arguments, and here it is given as a value: write the fn it is given as yourself", self.label, b.key);
                        todos.extend(self.cannot(done, &span, w.clone(), w));
                    } else if let Some(why) = &b.gone {
                        let w = format!("{} drops {}{}: what read it is to change", self.label, b.key, why);
                        todos.extend(self.cannot(done, &span, w.clone(), w));
                    } else if b.now != b.key || b.now_pack != self.pack.name {
                        *x = json!(format!("{}/{}", b.now_pack, b.now));
                        done.changes += 1;
                    }
                    break;
                }
                let Some(m) = s.strip_prefix(&format!("{}.", b.full)) else { continue };
                if b.kind == "fn" {
                    continue;
                }
                if let Some(why) = b.dropped.get(m) {
                    let w = format!("{} drops {}.{}{}: what read it is to change", self.label, b.key, m, why);
                    todos.extend(self.cannot(done, &span, w.clone(), w));
                    break;
                }
                let to = b.renamed.get(m).cloned().unwrap_or_else(|| m.to_string());
                let now = format!("{}/{}.{}", b.now_pack, b.now, to);
                if now != s {
                    if to != m {
                        short.push((m.to_string(), to));
                    }
                    *x = json!(now);
                    done.changes += 1;
                }
                break;
            }
        }
        // the name written beside a target: a field read's, a record field's
        for (was, now) in short {
            for k in ["field", "name"] {
                if o.get(k).and_then(Value::as_str) == Some(was.as_str()) {
                    o.insert(k.to_string(), json!(now));
                }
            }
        }
        todos
    }

    /// A case of an enum or a token type: renamed, put to a person (`split`), or gone.
    fn a_case(&self, v: &mut Value, b: &Block, at: &Value, done: &mut Done) -> Vec<String> {
        let name_key = if v.get("case").is_some() { "case" } else { "name" };
        let Some(case) = v[name_key].as_str().map(str::to_string) else { return Vec::new() };
        let span = at.clone();
        let line = self.line(&span);
        let mut todos = self.rename(v, at, done);
        if let Some(why) = b.cases_gone.get(&case) {
            let w = format!("{} dropped {}{}: write what it is now", self.label, case, why);
            todos.extend(self.cannot(done, &span, w.clone(), w));
            return todos;
        }
        for op in &b.ops {
            let Op::Call(o, args) = op else { continue };
            if args.iter().find(|(n, _)| n == "from").and_then(|(_, v)| case_of(v)).as_deref() != Some(case.as_str()) {
                continue;
            }
            match o.as_str() {
                "rename" => {
                    let Some(to) = b.cases.get(&case) else { continue };
                    done.changed.push(format!("{}:{}  {} → {}, renamed by {}{}", self.path, line, case, to, self.label, why_of(args)));
                    v[name_key] = json!(to);
                    done.changes += 1;
                }
                "split" => {
                    let options: Vec<String> = match args.iter().find(|(n, _)| n == "to").map(|(_, v)| &v.kind) {
                        Some(ExprKind::List(xs)) => xs.iter().filter_map(case_of).collect(),
                        _ => Vec::new(),
                    };
                    let why = args.iter().find(|(n, _)| n == "why").map(|(_, v)| literal_text(v)).unwrap_or_default();
                    let key = format!("{}:{} {}", self.path, line, case);
                    match ask(&key, &format!("{}:{} uses {}, which {} replaces{}", self.path, line, case, self.label, if why.is_empty() { String::new() } else { format!(": {}", why) }), &options) {
                        Some(pick) => {
                            done.changed.push(format!("{}:{}  {} → {}, as answered{}", self.path, line, case, pick, if why.is_empty() { String::new() } else { format!(" ({})", why) }));
                            v[name_key] = json!(pick);
                            done.changes += 1;
                        }
                        None => {
                            let said = format!("{} splits {} into {}, and only a person knows which this one is: run xtmd migrate in a terminal, or answer it in --answers as [split] \"{}\" = \"{}\"", self.label, case, options.join(", "), key, options.first().cloned().unwrap_or_default());
                            let todo = format!("{} splits {} into {}: write the one this is{}", self.label, case, options.join(" or "), if why.is_empty() { String::new() } else { format!(" ({})", why) });
                            todos.extend(self.cannot(done, &span, said, todo));
                        }
                    }
                }
                _ => {}
            }
            break;
        }
        todos
    }

    // ---- uses

    /// A use of a widget: each statement of its block applied in order, then written back.
    fn a_use(&self, v: &mut Value, b: &Block, prose: bool, at: &Value, env: &Env, done: &mut Done) -> Vec<String> {
        let span = at.clone();
        let line = self.line(&span);
        let mut u = Use {
            name: b.key.clone(),
            args: v["args"].as_array().cloned().unwrap_or_default(),
            slots: v["slots"].as_array().into_iter().flatten().map(|s| (last(s["slot"].as_str().unwrap_or("")).to_string(), s.clone())).collect(),
            wrappers: Vec::new(),
            removed: None,
            placed: None,
            prose,
            span: span.clone(),
        };
        let before = (u.name.clone(), u.args.clone(), u.slots.clone());
        let mut said = Vec::new();
        let mut changed = Vec::new();
        for op in &b.ops {
            if u.removed.is_some() {
                break;
            }
            if let Err(why) = self.apply(op, &mut u, b, env, line, &mut said, &mut changed, done) {
                let mut w = why.clone();
                if !w.contains(&self.label) {
                    w = format!("{}: {}", self.label, w);
                }
                return self.cannot(done, &span, w.clone(), w);
            }
        }
        done.notices.extend(said);
        done.changed.extend(changed);
        if let Some(value) = u.placed.take() {
            done.changed.push(format!("{}:{}  {} → the value it was given, placed where it stands", self.path, line, b.key));
            *v = json!({ "xtmd_placed": value });
            done.changes += 1;
            return Vec::new();
        }
        if let Some(why) = &u.removed {
            let first = printed_use(v, prose).lines().next().unwrap_or("").trim_end_matches(';').to_string();
            done.deletions.push(format!("{}:{}  {} (kept as a comment){}", self.path, line, first, why));
            let mut kept = vec![format!("{} dropped {}{}", self.label, b.key, why)];
            kept.extend(printed_use(v, prose).lines().map(str::to_string));
            *v = json!({ "xtmd_kept": kept });
            done.changes += 1;
            return Vec::new();
        }
        if (u.name.clone(), u.args.clone(), u.slots.clone()) == before && u.wrappers.is_empty() {
            return Vec::new();
        }
        done.changes += 1;
        let Some(o) = v.as_object_mut() else { return Vec::new() };
        let (home, new) = self.home(&u.name).unwrap_or_else(|| (b.now_pack.clone(), b.new.clone()));
        self.write_use(o, &home, &u.name, u.args.clone(), u.slots.clone(), &new);
        // each widget put around it holds it in its first slot not given otherwise
        let mut inner = Value::Object(o.clone());
        for (w, args, slots) in &u.wrappers {
            let (wp, wd) = self.home(w).unwrap_or_else(|| (self.pack.name.clone(), Decl::default()));
            let free = wd.slots.iter().find(|s| !slots.iter().any(|(n, _)| *n == s.name)).map(|s| s.name.clone()).unwrap_or_else(|| "body".into());
            let item = obj(if prose { "call" } else { "use" }, inner, &span);
            let mut slots = slots.clone();
            slots.push((free.clone(), obj(if prose { "prose" } else { "view" }, json!([item]), &span)));
            let mut wo = Map::new();
            wo.insert("span".into(), span.clone());
            self.write_use(&mut wo, &wp, w, args.clone(), slots, &wd);
            inner = Value::Object(wo);
        }
        if let Value::Object(x) = inner {
            *o = x;
        }
        Vec::new()
    }

    /// The pack of a name the migration writes, and its declaration there: the newer version's own, or
    /// one of a pack the migration's file imports.
    fn home(&self, name: &str) -> Option<(String, Decl)> {
        if let Some(d) = api::decl(&self.new_src, name) {
            return Some((self.pack.name.clone(), d));
        }
        self.imports.iter().filter(|(_, n, _)| n.as_deref().map_or(true, |n| n == name)).find_map(|(p, _, _)| api::decl(&pack_files(&self.world_new, p), name).map(|d| (p.clone(), d)))
    }

    /// A use's object written for the widget `name` of `pack`, as the newer version has it.
    fn write_use(&self, o: &mut Map<String, Value>, pack: &str, name: &str, args: Vec<Value>, slots: Vec<(String, Value)>, new: &Decl) {
        let id = format!("{}/{}", pack, name);
        let args: Vec<Value> = args
            .into_iter()
            .map(|mut a| {
                let n = a["name"].as_str().unwrap_or("").to_string();
                a["target"] = json!(format!("{}.{}", id, n));
                a
            })
            .collect();
        // a block of plain children goes to a widget with one slot; with more, each is named
        let one = new.slots.len() == 1;
        let slots: Vec<Value> = slots
            .into_iter()
            .map(|(n, mut s)| {
                s["slot"] = json!(format!("{}.{}", id, n));
                if one {
                    s["children"] = json!(true);
                } else if let Some(x) = s.as_object_mut() {
                    x.remove("children");
                }
                s
            })
            .collect();
        o.insert("widget".into(), json!(id));
        o.insert("args".into(), json!(args));
        if slots.is_empty() {
            o.remove("slots");
        } else {
            o.insert("slots".into(), json!(slots));
        }
    }

    /// One statement applied to a use.
    #[allow(clippy::too_many_arguments)]
    fn apply(&self, op: &Op, u: &mut Use, b: &Block, env: &Env, line: usize, said: &mut Vec<String>, changed: &mut Vec<String>, done: &mut Done) -> Result<(), String> {
        let key = &b.key;
        let mut e = env.clone();
        e.bind(&u.args, b);
        match op {
            Op::If(branches, otherwise) => {
                for (c, ops) in branches {
                    if self.holds(&e, c, line)? {
                        for op in ops {
                            self.apply(op, u, b, env, line, said, changed, done)?;
                        }
                        return Ok(());
                    }
                }
                for op in otherwise {
                    self.apply(op, u, b, env, line, said, changed, done)?;
                }
            }
            Op::Execute(blk) => match self.execute(blk, &mut e, said, &format!("{}:{}  {}, {}", self.path, line, self.label, key), line)? {
                Flow::Return(Some(x)) => {
                    let ExprKind::View(call) = &x.kind else { return Err(format!("{}'s execute for {} returns something other than a use", self.label, key)) };
                    // `return @value;`: the view one of its inputs was given, placed where the use stands
                    if call.args.is_none() && call.block.is_none() && b.old.input(&call.name.name).is_some() {
                        let given = e.given.get(&call.name.name).cloned().ok_or_else(|| format!("{}'s execute for {} places `{}`, which this use doesn't give", self.label, key, call.name.name))?;
                        u.placed = Some(given);
                        return Ok(());
                    }
                    let new = self.home(&call.name.name).map(|(_, d)| d).ok_or_else(|| format!("{}'s execute for {} returns a use of `{}`, which {} doesn't provide", self.label, key, call.name.name, self.label))?;
                    // the use's own blocks go where what it returns places them: `@body;` for its slot `body`
                    let mut slots = Vec::new();
                    match &call.block {
                        None if !u.slots.is_empty() => {
                            return Err(format!("{}'s execute for {} leaves out this use's block: what it returns places it, as `@{}(…) {{ @{}; }}`", self.label, key, call.name.name, u.slots[0].0));
                        }
                        None => {}
                        Some(ast::CallBlock::Children(blk)) => {
                            let into = new.slots.first().map(|s| s.name.clone()).ok_or_else(|| format!("{}'s execute for {} gives `{}` a block, and it has no slot", self.label, key, call.name.name))?;
                            for st in &blk.stmts {
                                let named = match &st.item {
                                    Stmt::Expr(Expr { kind: ExprKind::View(v), .. }) if v.args.is_none() && v.block.is_none() => u.slots.iter().find(|(n, _)| *n == v.name.name).cloned(),
                                    _ => None,
                                };
                                let Some((_, entry)) = named else { return Err(format!("{}'s execute for {} returns a block with code of its own, which xtmd migrate can't write yet", self.label, key)) };
                                slots.push((into.clone(), entry));
                            }
                        }
                        Some(ast::CallBlock::Named(_)) => return Err(format!("{}'s execute for {} returns a block with named slots, which xtmd migrate can't write yet", self.label, key)),
                    }
                    let mut args = Vec::new();
                    for a in call.args.iter().flat_map(|g| g.iter()) {
                        let n = a.name.as_ref().map(|n| n.name.clone()).unwrap_or_default();
                        let ty = new.input(&n).map(|m| m.ty.clone()).unwrap_or_default();
                        // what the use gave, by its name, goes as it is
                        let value = match bare(&a.value).and_then(|x| e.given.get(&x).cloned()) {
                            Some(v) => v,
                            None => self.value(&e, &a.value, &ty, line)?,
                        };
                        args.push(json!({ "name": n, "value": value, "span": u.span }));
                    }
                    u.name = call.name.name.clone();
                    u.args = args;
                    u.slots = slots;
                }
                Flow::Return(None) => u.removed = Some(String::new()),
                Flow::Next => return Err(format!("{}'s execute for {} ends without a return", self.label, key)),
                Flow::Unknown(why) => return Err(format!("{}'s execute for {} can't decide this use: {}", self.label, key, why)),
            },
            Op::Event(name, inner) => {
                let mut current = name.clone();
                for i in inner {
                    let Op::Call(o, args) = i else { continue };
                    let pos = u.arg(&current);
                    match o.as_str() {
                        "rename" => {
                            if let Some(to) = arg_name(args, "to") {
                                if let Some(p) = pos {
                                    u.args[p]["name"] = json!(to.clone());
                                }
                                current = to;
                            }
                        }
                        "add" | "drop" => {
                            let Some(p) = pos else { continue };
                            let param = arg_name(args, "arg").ok_or_else(|| format!("{} {}s an argument of {} with no name", self.label, o, name))?;
                            let ty = b.new.events.iter().find(|(n, _)| *n == current).and_then(|(_, ps)| ps.iter().find(|m| m.name == param)).map(|m| m.ty.clone());
                            self.handler(&mut u.args[p]["value"], o, &param, ty.as_deref()).map_err(|why| format!("{} {}s the argument `{}` of {}'s handlers, and {}", self.label, o, param, current, why))?;
                        }
                        _ => {}
                    }
                }
            }
            Op::Call(o, args) => {
                let why = why_of(args);
                let mem = member(args);
                let when = args.iter().find(|(n, _)| n == "when").map(|(_, v)| v.clone());
                match o.as_str() {
                    "rename" => {
                        if let (Some(m), Some(to)) = (&mem, arg_name(args, "to")) {
                            if let Some(p) = u.arg(m) {
                                changed.push(format!("{}:{}  {}: {} → {}{}", self.path, line, key, m, to, why));
                                u.args[p]["name"] = json!(to);
                            } else if let Some(s) = u.slots.iter_mut().find(|(n, _)| n == m) {
                                changed.push(format!("{}:{}  {}: {} → {}{}", self.path, line, key, m, to, why));
                                s.0 = to;
                            }
                        } else if let Some(to) = arg_name(args, "to") {
                            if !args.iter().any(|(n, _)| n == "from") {
                                changed.push(format!("{}:{}  {} → {}{}", self.path, line, key, to, why));
                                u.name = to;
                            }
                        }
                    }
                    "drop" => match &mem {
                        Some(m) => {
                            if let Some(p) = u.arg(m) {
                                let value = &u.args[p]["value"];
                                if value.get("lambda").is_some() {
                                    return Err(format!("{} drops {}.{}, and this use gives it code of its own: move it yourself", self.label, key, m));
                                }
                                done.deletions.push(format!("{}:{}  {} = {}{}", self.path, line, m, shown(value), why));
                                u.args.remove(p);
                            } else if let Some(p) = u.slots.iter().position(|(n, _)| n == m) {
                                let s = u.slots.remove(p).1;
                                done.deletions.push(format!("{}:{}  {}'s block (kept as a comment){}", self.path, line, m, why));
                                let _ = s;
                            }
                        }
                        None => u.removed = Some(why.clone()),
                    },
                    "set" | "add" => {
                        let m = mem.ok_or("a set with no member")?;
                        if let Some(w) = &when {
                            if !self.holds(&e, w, line)? {
                                return Ok(());
                            }
                        }
                        let x = args.iter().find(|(n, _)| n == "value").map(|(_, v)| v.clone()).ok_or("a set with no value")?;
                        let ty = b.new.input(&m).or(b.old.input(&m)).map(|i| i.ty.clone()).unwrap_or_default();
                        let value = self.value(&e, &x, &ty, line)?;
                        match u.arg(&m) {
                            Some(p) => {
                                changed.push(format!("{}:{}  {}: {} = {}, was {}{}", self.path, line, key, m, shown(&value), shown(&u.args[p]["value"]), why));
                                u.args[p]["value"] = value;
                            }
                            None => {
                                changed.push(format!("{}:{}  {}: {} = {}, added{}", self.path, line, key, m, shown(&value), why));
                                u.args.push(json!({ "name": m, "value": value, "span": u.span }));
                            }
                        }
                    }
                    "notify" => {
                        let holds = match &when {
                            Some(w) => self.holds(&e, w, line)?,
                            None => true,
                        };
                        if holds {
                            said.push(format!("{}:{}  {}, {}{}", self.path, line, self.label, key, why));
                        }
                    }
                    "move" => {
                        let from = args.iter().find(|(n, _)| n == "from").and_then(|(_, v)| place(v)).ok_or("a move with no `from`")?;
                        let to = args.iter().find(|(n, _)| n == "to").and_then(|(_, v)| place(v)).ok_or("a move with no `to`")?;
                        self.move_value(u, &from, &to, b)?;
                    }
                    "nest" => {
                        let into = arg_name(args, "into").ok_or("a nest with no `into`")?;
                        let inputs: Vec<String> = match args.iter().find(|(n, _)| n == "inputs").map(|(_, v)| &v.kind) {
                            Some(ExprKind::List(xs)) => xs.iter().filter_map(bare).collect(),
                            _ => Vec::new(),
                        };
                        for i in inputs {
                            self.move_value(u, &[i.clone()], &[into.clone(), i], b)?;
                        }
                    }
                    "flatten" => {
                        let r = arg_name(args, "input").ok_or("a flatten with no `input`")?;
                        if let Some(p) = u.arg(&r) {
                            let fields = u.args[p]["value"]["record"]["fields"].as_array().cloned().ok_or_else(|| format!("{} flattens {}.{}, and this use gives it something other than a record written out", self.label, key, r))?;
                            let span = u.args[p]["span"].clone();
                            u.args.remove(p);
                            for f in fields.into_iter().rev() {
                                u.args.insert(p, json!({ "name": f["name"], "value": f["value"], "span": span }));
                            }
                        }
                    }
                    "wrap" => {
                        let parent = arg_name(args, "parent").ok_or("a wrap with no `parent`")?;
                        u.wrappers.push((parent, Vec::new(), Vec::new()));
                    }
                    "split" | "example" => {}
                    other => return Err(format!("{} uses `{}`, which xtmd migrate can't apply", self.label, other)),
                }
            }
        }
        Ok(())
    }

    /// A handler given for an event, with an argument added or taken out: in place where it is written
    /// as a fn; one given by name is for a person.
    fn handler(&self, h: &mut Value, op: &str, param: &str, ty: Option<&str>) -> Result<(), String> {
        let Some(l) = h.get_mut("lambda") else { return Err("this handler is given by name: wrap it in a fn yourself".into()) };
        let params = l["params"].as_array_mut().ok_or("this handler has no parameters to change")?;
        match op {
            "add" => {
                if params.iter().any(|p| p["name"] == param) {
                    return Ok(());
                }
                let ty = ty.ok_or("the newer version doesn't say its type")?;
                let t = self.probe.rir_type(ty).ok_or_else(|| format!("its type `{}` can't be written here", ty))?;
                params.push(json!({ "name": param, "type": t }));
            }
            _ => {
                let Some(p) = params.iter().position(|x| x["name"] == param) else { return Ok(()) };
                if reads_local(&l["body"], param) {
                    return Err("this handler reads it: change it yourself".into());
                }
                l["params"].as_array_mut().unwrap().remove(p);
            }
        }
        Ok(())
    }

    /// What a use gives at one place, moved to another: an input, a field of a record input, a slot,
    /// or an input or slot of the widget a wrap put around it.
    fn move_value(&self, u: &mut Use, from: &[String], to: &[String], b: &Block) -> Result<(), String> {
        let key = &b.key;
        // take it out: an argument's value, or a slot's block
        enum Got {
            Value(Value),
            Slot(Value),
        }
        // a slot moved to another of the same use stays where it is written
        let mut slot_at = None;
        let got = match from {
            [m] => match (u.arg(m), u.slots.iter().position(|(n, _)| n == m)) {
                (Some(p), _) => Got::Value(u.args.remove(p)["value"].clone()),
                (None, Some(p)) => {
                    slot_at = Some(p);
                    Got::Slot(u.slots.remove(p).1)
                }
                (None, None) => return Ok(()),
            },
            [r, f] => {
                let Some(p) = u.arg(r) else { return Ok(()) };
                let fields = u.args[p].pointer_mut("/value/record/fields").and_then(Value::as_array_mut).ok_or_else(|| format!("{} moves {}.{}.{}, and this use doesn't write {} out as a record", self.label, key, r, f, r))?;
                let Some(fp) = fields.iter().position(|x| x["name"] == f.as_str()) else { return Ok(()) };
                let v = fields.remove(fp)["value"].clone();
                if fields.is_empty() {
                    u.args.remove(p);
                }
                Got::Value(v)
            }
            _ => return Err(format!("{} moves from a place it can't name", self.label)),
        };
        let new = |w: &str| self.home(w).map(|(_, d)| d).unwrap_or_default();
        // where it goes: a widget around it, or this one
        let (target, place): (Option<usize>, &[String]) = match to {
            [w, rest @ ..] if !rest.is_empty() && u.wrappers.iter().any(|(n, ..)| n == w) => (u.wrappers.iter().position(|(n, ..)| n == w), rest),
            _ => (None, to),
        };
        let decl = match target {
            Some(t) => new(&u.wrappers[t].0),
            None => self.home(&u.name).map(|(_, d)| d).unwrap_or_else(|| b.new.clone()),
        };
        let span = u.span.clone();
        let prose = u.prose;
        let (args, slots) = match target {
            Some(t) => {
                let w = &mut u.wrappers[t];
                (&mut w.1, &mut w.2)
            }
            None => (&mut u.args, &mut u.slots),
        };
        match (place, got) {
            ([s], got) if decl.slot(s).is_some() => {
                let block = match got {
                    Got::Slot(s) => s,
                    Got::Value(v) => as_block(&v, prose, &span).ok_or_else(|| format!("{} moves {} into the slot {}, and this use gives it a value that isn't a text", self.label, from.join("."), s))?,
                };
                match slot_at.filter(|p| target.is_none() && *p <= slots.len()) {
                    Some(p) => slots.insert(p, (s.clone(), block)),
                    None => slots.push((s.clone(), block)),
                }
            }
            ([m], got) => {
                let v = match got {
                    Got::Value(v) => v,
                    Got::Slot(s) => as_text(&s).ok_or_else(|| format!("{} moves the slot {} into {}, a text, and this use fills it with more than plain text: move it yourself", self.label, from.join("."), m))?,
                };
                args.push(json!({ "name": m, "value": v, "span": span }));
            }
            ([r, f], Got::Value(v)) => {
                let ty = decl.input(r).map(|i| i.ty.clone()).ok_or_else(|| format!("{} moves into {}.{}, which {} doesn't have", self.label, r, f, decl_name(&decl, key)))?;
                let field = json!({ "name": f, "target": format!("{}/{}.{}", self.pack.name, ty, f), "value": v, "span": span });
                match args.iter().position(|a| a["name"] == r.as_str()) {
                    Some(p) => {
                        let fields = args[p].pointer_mut("/value/record/fields").and_then(Value::as_array_mut).ok_or_else(|| format!("{} moves into {}.{}, and this use gives {} something other than a record written out", self.label, r, f, r))?;
                        fields.push(field);
                    }
                    None => args.push(json!({ "name": r, "value": { "record": { "of": format!("{}/{}", self.pack.name, ty), "fields": [field] }, "span": span }, "span": span })),
                }
            }
            _ => return Err(format!("{} moves to a place it can't name", self.label)),
        }
        Ok(())
    }

    // ---- records and fns

    /// A record written out: its block's statements, over its fields.
    fn a_record(&self, v: &mut Value, b: &Block, at: &Value, env: &Env, done: &mut Done) -> Vec<String> {
        let span = at.clone();
        let line = self.line(&span);
        // a record that became a type the language has built in isn't written out in the newer version:
        // what it holds stays as written, for a person
        if crate::check::BUILTINS.contains(&b.now.as_str()) && b.new.kind.is_empty() {
            let how = if b.now == "rich_text" { " (rich_text.from(…), rich_text.plain(…))" } else { "" };
            let was = if b.key == b.now { String::new() } else { format!(" (`{}` before)", b.key) };
            let w = format!("{}'s {}{} is built in, and isn't written out as a record: make it from what it holds{}", self.label, b.now, was, how);
            return self.cannot(done, &span, w.clone(), w);
        }
        let mut fields = v["fields"].as_array().cloned().unwrap_or_default();
        let mut name = b.key.clone();
        for op in &b.ops {
            let Op::Call(o, args) = op else { continue };
            let mut e = env.clone();
            e.bind(&fields, b);
            let why = why_of(args);
            let mem = member(args);
            let r: Result<(), String> = (|| {
                match (o.as_str(), &mem) {
                    ("rename", Some(m)) => {
                        let to = arg_name(args, "to").ok_or("a rename with no `to`")?;
                        if let Some(f) = fields.iter_mut().find(|f| f["name"] == m.as_str()) {
                            f["name"] = json!(to);
                        }
                    }
                    ("rename", None) => {
                        if let Some(to) = arg_name(args, "to") {
                            name = to;
                        }
                    }
                    ("drop", Some(m)) => {
                        if let Some(p) = fields.iter().position(|f| f["name"] == m.as_str()) {
                            done.deletions.push(format!("{}:{}  {} = {}{}", self.path, line, m, shown(&fields[p]["value"]), why));
                            fields.remove(p);
                        }
                    }
                    ("set" | "add", Some(m)) => {
                        if let Some(w) = args.iter().find(|(n, _)| n == "when").map(|(_, v)| v) {
                            if !self.holds(&e, w, line)? {
                                return Ok(());
                            }
                        }
                        let x = args.iter().find(|(n, _)| n == "value").map(|(_, v)| v.clone()).ok_or("a set with no value")?;
                        let ty = b.new.input(m).or(b.old.input(m)).map(|i| i.ty.clone()).unwrap_or_default();
                        let value = self.value(&e, &x, &ty, line)?;
                        match fields.iter().position(|f| f["name"] == m.as_str()) {
                            Some(p) => {
                                done.changed.push(format!("{}:{}  {}: {} = {}, was {}{}", self.path, line, b.key, m, shown(&value), shown(&fields[p]["value"]), why));
                                fields[p]["value"] = value;
                            }
                            None => {
                                done.changed.push(format!("{}:{}  {}: {} = {}, added{}", self.path, line, b.key, m, shown(&value), why));
                                fields.push(json!({ "name": m, "value": value, "span": span }));
                            }
                        }
                    }
                    ("notify", _) => done.notices.push(format!("{}:{}  {}, {}{}", self.path, line, self.label, b.key, why)),
                    ("drop", None) => return Err(format!("{} drops {}{}: write what this is now", self.label, b.key, why)),
                    ("example" | "split", _) => {}
                    (other, _) => return Err(format!("{} uses `{}` on a record, which xtmd migrate can't apply", self.label, other)),
                }
                Ok(())
            })();
            if let Err(w) = r {
                return self.cannot(done, &span, w.clone(), w);
            }
        }
        let id = format!("{}/{}", self.pack.name, name);
        for f in fields.iter_mut() {
            let n = f["name"].as_str().unwrap_or("").to_string();
            f["target"] = json!(format!("{}.{}", id, n));
        }
        if v["fields"] != json!(fields) || v["of"] != id.as_str() {
            v["fields"] = json!(fields);
            v["of"] = json!(id);
            done.changes += 1;
        }
        Vec::new()
    }

    /// A call of a fn: renamed, and its arguments added, dropped and set where the newer one has them.
    fn a_call(&self, v: &mut Value, b: &Block, at: &Value, env: &Env, done: &mut Done) -> Vec<String> {
        let span = at.clone();
        let line = self.line(&span);
        let mut args: Vec<(String, Value)> = b.old.inputs.iter().map(|m| m.name.clone()).zip(v["args"].as_array().cloned().unwrap_or_default()).collect();
        let given = v["args"].as_array().map_or(0, Vec::len);
        if given != args.len() {
            let w = format!("{}: this call of {} gives {} arguments, and {}@{} has {}", self.label, b.key, given, self.pack.name, self.label.rsplit('@').next().and_then(|x| x.parse::<u32>().ok()).map_or(0, |x| x - 1), b.old.inputs.len());
            return self.cannot(done, &span, w.clone(), w);
        }
        // a fn dropped whole: what calls it is for a person
        if let Some(why) = &b.gone {
            let w = format!("{} drops {}{}: write what this is now", self.label, b.key, why);
            return self.cannot(done, &span, w.clone(), w);
        }
        for op in &b.ops {
            let Op::Call(o, a) = op else { continue };
            let mut e = env.clone();
            let named: Vec<Value> = args.iter().map(|(n, x)| json!({ "name": n, "value": x })).collect();
            e.bind(&named, b);
            let why = why_of(a);
            let r: Result<(), String> = (|| {
                match o.as_str() {
                    "rename" => {
                        if let (Some(m), Some(to)) = (arg_name(a, "arg"), arg_name(a, "to")) {
                            if let Some(x) = args.iter_mut().find(|(n, _)| *n == m) {
                                x.0 = to;
                            }
                        }
                    }
                    "add" | "set" => {
                        let m = arg_name(a, "arg").ok_or("an argument with no name")?;
                        if let Some(w) = a.iter().find(|(n, _)| n == "when").map(|(_, v)| v) {
                            if !self.holds(&e, w, line)? {
                                return Ok(());
                            }
                        }
                        let x = a.iter().find(|(n, _)| n == "value").map(|(_, v)| v.clone()).ok_or("an argument with no value")?;
                        let ty = b.new.input(&m).map(|i| i.ty.clone()).ok_or_else(|| format!("{} adds the argument `{}` to {}, which the newer {} doesn't take", self.label, m, b.key, b.now))?;
                        let value = self.value(&e, &x, &ty, line)?;
                        done.changed.push(format!("{}:{}  {}: {} = {}{}{}", self.path, line, b.key, m, shown(&value), if o == "add" { ", added" } else { "" }, why));
                        match args.iter().position(|(n, _)| *n == m) {
                            Some(p) => args[p].1 = value,
                            None => args.push((m, value)),
                        }
                    }
                    "drop" => {
                        let m = arg_name(a, "arg").ok_or("an argument with no name")?;
                        if let Some(p) = args.iter().position(|(n, _)| *n == m) {
                            done.deletions.push(format!("{}:{}  {} = {}{}", self.path, line, m, shown(&args[p].1), why));
                            args.remove(p);
                        }
                    }
                    "notify" => done.notices.push(format!("{}:{}  {}, {}{}", self.path, line, self.label, b.key, why)),
                    "example" => {}
                    other => return Err(format!("{} uses `{}` on a fn, which xtmd migrate can't apply", self.label, other)),
                }
                Ok(())
            })();
            if let Err(w) = r {
                return self.cannot(done, &span, w.clone(), w);
            }
        }
        // placed where the newer version has them, by name
        let mut placed = Vec::new();
        for m in &b.new.inputs {
            match args.iter().position(|(n, _)| *n == m.name) {
                Some(p) => placed.push(args.remove(p).1),
                None => {
                    let w = format!("{}: {} takes `{}` now, and nothing gives it", self.label, b.now, m.name);
                    return self.cannot(done, &span, w.clone(), w);
                }
            }
        }
        if let Some((n, _)) = args.first() {
            let w = format!("{}: {} doesn't take `{}` now: drop it in the migration", self.label, b.now, n);
            return self.cannot(done, &span, w.clone(), w);
        }
        let id = format!("{}/{}", b.now_pack, b.now);
        if v["args"] != json!(placed) || v["fn"] != id.as_str() {
            v["args"] = json!(placed);
            v["fn"] = json!(id);
            done.changes += 1;
        }
        Vec::new()
    }

    // ---- values

    /// A use's code with each value it gives in place of the member's name, as the run shows it.
    fn shown_here(&self, e: &Env, x: &Expr) -> String {
        let mut x = x.clone();
        ops::walk_names(&mut x, &mut |n| match (e.given.get(n), e.defaults.get(n)) {
            (Some(v), _) => Some(raw(&shown(v))),
            (None, Some(d)) => Some(raw(d)),
            _ => None,
        });
        print_expr(&x)
    }

    /// Whether a condition holds for a use: worked out from what it gives, or put to a person.
    fn holds(&self, e: &Env, c: &Expr, line: usize) -> Result<bool, String> {
        let c = self.decided(e, c, line)?;
        if let ExprKind::Bool(b) = c.kind {
            return Ok(b);
        }
        if let Some(src) = self.constant_source(e, &c) {
            if let Some(crate::eval::Value::Bool(b)) = self.probe.eval("", "bool", &src) {
                return Ok(b);
            }
        }
        let key = format!("{}:{} {}: {}", self.path, line, e.key, print_expr(&c));
        let asks = format!("{}:{}: does `{}` hold, which is `{}` here? {} applies a statement to {} where it does", self.path, line, print_expr(&c), self.shown_here(e, &c), self.label, e.key);
        match ask(&key, &asks, &["yes".to_string(), "no".to_string()]) {
            Some(a) => Ok(a == "yes"),
            None => Err(format!("whether `{}` holds here depends on more than this use gives: answer it in --answers as [split] \"{}\" = \"yes\" or \"no\"", print_expr(&c), key)),
        }
    }

    /// `given`, `confirm` and `ask` answered: what a person decides stands in the code as its value.
    fn decided(&self, e: &Env, x: &Expr, line: usize) -> Result<Expr, String> {
        let mut x = x.clone();
        let mut failed = None;
        decide(&mut x, &mut |name, args| match name {
            "given" => args.first().and_then(|(_, v)| bare(v)).map(|m| lit_bool(e.given.contains_key(&m))),
            "confirm" => {
                let why = args.iter().find(|(n, _)| n == "why").map(|(_, v)| literal_text(v)).unwrap_or_default();
                confirm(&why).map(lit_bool)
            }
            "ask" => {
                let why = args.iter().find(|(n, _)| n == "why").map(|(_, v)| literal_text(v)).unwrap_or_default();
                let options: Vec<Expr> = match args.iter().find(|(n, _)| n == "options").map(|(_, v)| &v.kind) {
                    Some(ExprKind::List(xs)) => xs.iter().cloned().collect(),
                    _ => Vec::new(),
                };
                // each option as this use has it
                let shown: Vec<String> = options.iter().map(|o| self.shown_here(e, o)).collect();
                let key = format!("{}:{} {}: {}", self.path, line, e.key, why);
                match ask(&key, &format!("{}:{}: {}", self.path, line, why), &shown) {
                    Some(pick) => shown.iter().position(|s| *s == pick).map(|k| options[k].clone()),
                    None => {
                        failed = Some(format!("{} asks \"{}\" of this use: answer it in --answers as [split] \"{}\" = '{}'", self.label, why, key, shown.first().cloned().unwrap_or_default()));
                        None
                    }
                }
            }
            _ => None,
        });
        match failed {
            Some(f) => Err(f),
            None => Ok(x),
        }
    }

    /// The code with every value the use gives written in as a constant, when each is one.
    fn constant_source(&self, e: &Env, x: &Expr) -> Option<String> {
        self.constant_source_with(e, x, false)
    }

    /// As `constant_source`; with `lets`, a constant `let` of the document's is its value too. Without,
    /// it stays the name the author wrote, which the newer version reads as well.
    fn constant_source_with(&self, e: &Env, x: &Expr, lets: bool) -> Option<String> {
        let mut x = self.inlined(x);
        let mut ok = true;
        ops::walk_names(&mut x, &mut |n| {
            let text = match (e.given.get(n), e.defaults.get(n)) {
                (Some(v), _) => {
                    let v = if lets { e.resolved(v) } else { v.clone() };
                    if !constant(&v) {
                        ok = false;
                        return None;
                    }
                    printed(&v, &v["type"])
                }
                (None, Some(d)) => Some(d.clone()),
                (None, None) => return None,
            };
            match text {
                Some(t) => Some(raw(&format!("({})", t))),
                None => {
                    ok = false;
                    None
                }
            }
        });
        ok.then(|| print_expr(&x))
    }

    /// A call of the migration's own fn whose body only returns a call of one the newer version
    /// provides, as that call: a use can be given it.
    fn inlined(&self, x: &Expr) -> Expr {
        let mut x = x.clone();
        inline(&mut x, &|name, args| {
            let f = self.probe.fns.get(name)?;
            let [only] = f.body.as_ref()?.stmts.as_slice() else { return None };
            let Stmt::Return(Some(r)) = &only.item else { return None };
            let calls = match &r.kind {
                ExprKind::Call { callee, .. } => match &callee.kind {
                    ExprKind::Name(n) => Some(n.name.clone()),
                    ExprKind::Field { name, .. } => Some(name.name.clone()),
                    _ => None,
                },
                _ => None,
            }?;
            if self.probe.private.contains(&calls) || !api::names(&self.new_src).contains(&calls) && api::decl(&self.new_src, &calls).is_none() {
                return None;
            }
            let given: BTreeMap<String, Expr> = f.sig.params.iter().map(|p| p.name.name.clone()).zip(args.iter().cloned()).collect();
            let mut r = r.clone();
            ops::walk_names(&mut r, &mut |n| given.get(n).map(|v| Expr { kind: ExprKind::Paren(Box::new(v.clone())), span: v.span }));
            Some(r)
        });
        x
    }

    /// The value a statement writes into a use, as its resolved IR: worked out where the use gives
    /// constants, else the migration's code with the use's values in it.
    fn value(&self, e: &Env, x: &Expr, ty: &str, line: usize) -> Result<Value, String> {
        let x = self.decided(e, x, line)?;
        if let Some(src) = self.constant_source(e, &x) {
            let worked = if ty.is_empty() { self.any_eval(&src) } else { self.probe.eval("", ty, &src) };
            if let Some(w) = worked.and_then(|v| self.probe.written(&v)) {
                let ty = if ty.is_empty() { self.any_type(&w) } else { ty.to_string() };
                return self.probe.rir("", &ty, &w);
            }
        }
        if ty.is_empty() {
            return Err(format!("{} writes a value whose type it doesn't say: give its `let` a type", self.label));
        }
        let x = self.inlined(&x);
        let names = values::names_in(&x);
        if let Some(own) = names.iter().find(|n| self.probe.private.contains(*n) && !e.given.contains_key(*n)) {
            // what only runs when migrating is run over a constant `let` of the document's, as
            // its value: there is no other way to write it
            if let Some(w) = self.constant_source_with(e, &x, true).and_then(|src| self.probe.eval("", ty, &src)).and_then(|v| self.probe.written(&v)) {
                return self.probe.rir("", ty, &w);
            }
            return Err(format!("{} works this out with `{}`, which only runs when migrating, and this use gives a value only known as the page runs: write it yourself", self.label, own));
        }
        // each value the use gives stands in as a call, which its own resolved IR then replaces
        let (mut decls, mut given) = (String::new(), Vec::new());
        let mut x = x;
        let mut failed = None;
        ops::walk_names(&mut x, &mut |n| match (e.given.get(n), e.defaults.get(n)) {
            (Some(v), _) => {
                let Some(t) = type_text(&v["type"], &self.pack.name) else {
                    failed = Some(format!("{} writes `{}` into its value, and its type can't be named in {}", self.label, n, self.label));
                    return None;
                };
                let k = given.len();
                decls.push_str(&format!("extern fn {}_{}(): {};\n", PROBE, k, t));
                given.push(v.clone());
                Some(raw(&format!("{}_{}()", PROBE, k)))
            }
            (None, Some(d)) => Some(raw(&format!("({})", d))),
            (None, None) => None,
        });
        if let Some(f) = failed {
            return Err(f);
        }
        let mut r = self.probe.rir(&decls, ty, &print_expr(&x)).map_err(|why| format!("{}'s value for this use doesn't compile: {}", self.label, why))?;
        fill(&mut r, &given);
        Ok(r)
    }

    /// An `execute` run over a use: what it returns, or why it can't say. Its variables are worked
    /// out as it goes, so a `mut` and a `for` hold only where each value is a constant.
    fn execute(&self, b: &ast::Block, e: &mut Env, said: &mut Vec<String>, at: &str, line: usize) -> Result<Flow, String> {
        for n in &b.stmts {
            match &n.item {
                Stmt::Return(r) => return Ok(Flow::Return(r.clone())),
                Stmt::Let(var) | Stmt::Mut(var) => {
                    let ty = self.step_text[var.ty.span.range()].to_string();
                    let v = self.value(e, &var.value, &ty, line)?;
                    if matches!(n.item, Stmt::Mut(_)) && !constant(&v) {
                        return Ok(Flow::Unknown(format!("`mut {}` holds a value only known as the page runs", var.name.name)));
                    }
                    e.types.insert(var.name.name.clone(), ty);
                    e.given.insert(var.name.name.clone(), v);
                }
                Stmt::Assign { place, value } => {
                    let Some(name) = bare(place) else { return Ok(Flow::Unknown(format!("`{} = …`", print_expr(place)))) };
                    let ty = e.types.get(&name).cloned().unwrap_or_default();
                    let v = self.value(e, value, &ty, line)?;
                    if !constant(&v) {
                        return Ok(Flow::Unknown(format!("`{} = {}`, a value only known as the page runs", name, print_expr(value))));
                    }
                    e.given.insert(name, v);
                }
                Stmt::For(f) => {
                    let Some(list) = self.constant_source(e, &f.over).and_then(|s| ["list<number>", "list<text>", "list<bool>"].iter().find_map(|t| self.probe.eval("", t, &s).map(|v| (v, *t)))) else {
                        return Ok(Flow::Unknown(format!("`for ({} in {})` over a list only known as the page runs", f.name.name, print_expr(&f.over))));
                    };
                    let (crate::eval::Value::List(items), ty) = list else { return Ok(Flow::Unknown("a `for` over something other than a list".into())) };
                    let item_ty = ty.trim_start_matches("list<").trim_end_matches('>').to_string();
                    for i in &items.items {
                        let Some(w) = self.probe.written(&i.value) else { return Ok(Flow::Unknown("a `for` over values with no literal".into())) };
                        e.given.insert(f.name.name.clone(), self.probe.rir("", &item_ty, &w)?);
                        match self.execute(&f.body, e, said, at, line)? {
                            Flow::Next => {}
                            other => return Ok(other),
                        }
                    }
                }
                Stmt::Expr(x) => {
                    if let Some((op, args)) = ops::call(x) {
                        if op == "notify" {
                            let when = args.iter().find(|(n, _)| n == "when").map(|(_, v)| v.clone());
                            if when.map_or(Ok(true), |w| self.holds(e, &w, line))? {
                                said.push(format!("{}: {}", at, args.iter().find(|(n, _)| n == "why").map(|(_, v)| literal_text(v)).unwrap_or_default()));
                            }
                        }
                    }
                }
                Stmt::If(i) => {
                    let mut taken = None;
                    for (c, blk) in &i.branches {
                        let ast::Cond::Expr(c) = c else { return Ok(Flow::Unknown("an `if` that opens an opt".into())) };
                        if self.holds(e, c, line)? {
                            taken = Some(blk);
                            break;
                        }
                    }
                    let Some(blk) = taken.or(i.otherwise.as_ref()) else { continue };
                    match self.execute(blk, e, said, at, line)? {
                        Flow::Next => {}
                        other => return Ok(other),
                    }
                }
                Stmt::Match(m) => {
                    let subject = self.decided(e, &m.subject, line)?;
                    let value = self.constant_source(e, &subject).and_then(|s| self.any_eval(&s));
                    let case = match &value {
                        Some(v) => m.cases.iter().find(|c| c.patterns.is_empty() || c.patterns.iter().any(|p| pattern_is(p, v))),
                        // which case it is, a person says
                        None => {
                            let options: Vec<String> = m.cases.iter().map(|c| if c.patterns.is_empty() { "else".to_string() } else { c.patterns.iter().map(pattern_text).collect::<Vec<_>>().join(", ") }).collect();
                            let key = format!("{}:{} {}: match ({})", self.path, line, e.key, print_expr(&m.subject));
                            let asks = format!("{}:{}: which case is `{}`, which is `{}` here ({})", self.path, line, print_expr(&m.subject), self.shown_here(e, &subject), options.join(" or "));
                            match ask(&key, &asks, &options) {
                                Some(pick) => options.iter().position(|o| *o == pick).map(|k| &m.cases.items[k].item),
                                None => return Err(format!("{}: answer it in --answers as [split] \"{}\" = \"{}\"", asks, key, options.first().cloned().unwrap_or_default())),
                            }
                        }
                    };
                    if let Some(c) = case {
                        match self.execute(&c.body, e, said, at, line)? {
                            Flow::Next => {}
                            other => return Ok(other),
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(Flow::Next)
    }

    /// A constant's value whatever its type: a text, a number or a bool, tried in turn.
    fn any_eval(&self, src: &str) -> Option<crate::eval::Value> {
        ["text", "number", "bool"].iter().find_map(|t| self.probe.eval("", t, src))
    }

    /// The type of a constant written out, of those `any_eval` tries.
    fn any_type(&self, src: &str) -> String {
        ["text", "number", "bool"].iter().find(|t| self.probe.rir("", t, src).is_ok()).map_or("text", |t| t).to_string()
    }

    // ---- imports

    /// Every name of another pack the file now writes has its import, written as the migration's file
    /// imports it; a name that would mean another declaration in this file stops it.
    fn imports(&self, file: &mut Value, before: &BTreeSet<(String, String)>, done: &mut Done) {
        let pack = self.pack.name.clone();
        // core is imported by every file
        if pack == "core" {
            return;
        }
        let mut kept = Vec::new();
        for mut i in file["imports"].as_array().cloned().unwrap_or_default() {
            if i["pack"] == pack.as_str() {
                if let Some(n) = i["name"].as_str().map(str::to_string) {
                    if let Some(b) = self.blocks.iter().find(|b| b.key == n) {
                        if b.gone.is_some() {
                            done.changes += 1;
                            continue;
                        }
                        if b.now != b.key || b.now_pack != pack {
                            i["pack"] = json!(b.now_pack);
                            i["name"] = json!(b.now);
                            i["target"] = json!(format!("{}/{}", b.now_pack, b.now));
                            done.changes += 1;
                        }
                    }
                }
            }
            kept.push(i);
        }
        let packs: BTreeSet<String> = std::iter::once(pack.clone()).chain(self.imports.iter().map(|(p, _, _)| p.clone())).collect();
        let mut now = BTreeSet::new();
        ids(file, &packs, &mut now);
        let mut core = BTreeSet::new();
        ids(file, &BTreeSet::from(["core".to_string()]), &mut core);
        let own: BTreeSet<String> = file["declarations"].as_array().into_iter().flatten().filter_map(|d| d.as_object()?.values().next()?["id"].as_str().map(str::to_string)).filter_map(|id| id.split_once('/').map(|(_, n)| n.to_string())).collect();
        for (p, n) in now.difference(before) {
            let covered = kept.iter().any(|i| i["pack"] == p.as_str() && i.get("comment").is_none() && (i.get("name").is_none() || (i["name"] == n.as_str() && i.get("as").is_none())));
            if own.contains(n) {
                self.refuse(done, &json!([0]), format!("{} writes `{}`, {}'s, and in this file `{}` is the document's own: rename the file's, then migrate it", self.label, n, p, n));
                continue;
            }
            if core.contains(&("core".to_string(), n.clone())) {
                self.refuse(done, &json!([0]), format!("{} writes `{}`, {}'s: it uses core's `{}`, and once migrated `{}` would be {}'s", self.label, n, p, n, n, p));
                continue;
            }
            if covered || kept.iter().any(|i| i["pack"] == p.as_str() && i["name"] == n.as_str()) {
                continue;
            }
            // as the migration's file has it: the whole pack, or the one name
            let whole = *p != pack && self.imports.iter().any(|(ip, n, _)| ip == p && n.is_none());
            if whole {
                if !kept.iter().any(|i| i["pack"] == p.as_str() && i.get("name").is_none()) {
                    kept.push(json!({ "pack": p }));
                }
            } else {
                kept.push(json!({ "pack": p, "name": n, "target": format!("{}/{}", p, n) }));
            }
            done.changes += 1;
        }
        file["imports"] = json!(kept);
    }
}

impl Env {
    /// A use's values by member name: what it gives, and the older version's defaults for the rest.
    fn bind(&mut self, args: &[Value], b: &Block) {
        self.key = b.key.clone();
        for m in &b.old.inputs {
            if let Some(d) = &m.default {
                self.defaults.insert(m.name.clone(), d.clone());
                // and by the name a rename gave it
                if let Some(to) = b.renamed.get(&m.name) {
                    self.defaults.insert(to.clone(), d.clone());
                }
            }
        }
        for a in args {
            if let Some(n) = a["name"].as_str() {
                self.given.insert(n.to_string(), a["value"].clone());
            }
        }
    }

    /// A value with a constant `let` of the file's in place of its name.
    fn resolved(&self, v: &Value) -> Value {
        match v["get"]["let"].as_str().and_then(|id| self.lets.get(id)) {
            Some(x) => x.clone(),
            None => v.clone(),
        }
    }
}

/// A name that is printed as it is: code put where an expression goes.
fn raw(text: &str) -> Expr {
    Expr { kind: ExprKind::Name(ast::Ident { name: text.to_string(), span: crate::syntax::Span::default() }), span: crate::syntax::Span::default() }
}

fn lit_bool(b: bool) -> Expr {
    Expr { kind: ExprKind::Bool(b), span: crate::syntax::Span::default() }
}

/// Each call of a word a person decides (`given`, `confirm`, `ask`) replaced by what `f` gives.
fn decide(e: &mut Expr, f: &mut dyn FnMut(&str, &[(String, Expr)]) -> Option<Expr>) {
    if let Some((name, args)) = ops::call(e) {
        if matches!(name.as_str(), "given" | "confirm" | "ask") {
            if let Some(x) = f(&name, &args) {
                *e = x;
            }
            return;
        }
    }
    match &mut e.kind {
        ExprKind::Call { callee, args } => {
            decide(callee, f);
            for a in args.items.iter_mut() {
                decide(&mut a.item.value, f);
            }
        }
        ExprKind::Field { target, .. } | ExprKind::Paren(target) | ExprKind::Try(target) | ExprKind::Unary { operand: target, .. } => decide(target, f),
        ExprKind::Binary { left, right, .. } => {
            decide(left, f);
            decide(right, f);
        }
        ExprKind::Ternary { cond, then, otherwise } => {
            decide(cond, f);
            decide(then, f);
            decide(otherwise, f);
        }
        ExprKind::List(xs) => {
            for x in xs.items.iter_mut() {
                decide(&mut x.item, f);
            }
        }
        ExprKind::Record { inits, .. } => {
            for i in inits.items.iter_mut() {
                decide(&mut i.item.value, f);
            }
        }
        _ => {}
    }
}

/// A pattern as it is written.
fn pattern_text(p: &ast::Pattern) -> String {
    match p {
        ast::Pattern::Case { ty, case, .. } => format!("{}.{}", ty.name, case.name),
        ast::Pattern::Number { negative, text, .. } => format!("{}{}", if *negative { "-" } else { "" }, text),
        ast::Pattern::Text(t) => format!("\"{}\"", t.parts.iter().map(|p| if let ast::TextPart::Piece { raw, .. } = p { raw.clone() } else { String::new() }).collect::<String>()),
        ast::Pattern::Bool(b, _) => b.to_string(),
    }
}

/// Each call `f` gives code for, replaced by it.
fn inline(e: &mut Expr, f: &dyn Fn(&str, &[Expr]) -> Option<Expr>) {
    if let ExprKind::Call { callee, args } = &e.kind {
        if let ExprKind::Name(n) = &callee.kind {
            let given: Vec<Expr> = args.iter().map(|a| a.value.clone()).collect();
            if let Some(x) = f(&n.name, &given) {
                *e = x;
                return;
            }
        }
    }
    match &mut e.kind {
        ExprKind::Call { callee, args } => {
            inline(callee, f);
            for a in args.items.iter_mut() {
                inline(&mut a.item.value, f);
            }
        }
        ExprKind::Field { target, .. } | ExprKind::Paren(target) | ExprKind::Unary { operand: target, .. } => inline(target, f),
        ExprKind::Binary { left, right, .. } => {
            inline(left, f);
            inline(right, f);
        }
        ExprKind::Ternary { cond, then, otherwise } => {
            inline(cond, f);
            inline(then, f);
            inline(otherwise, f);
        }
        _ => {}
    }
}

fn pattern_is(p: &ast::Pattern, v: &crate::eval::Value) -> bool {
    use crate::eval::Value as V;
    match (p, v) {
        (ast::Pattern::Case { ty, case, binds: None }, V::Case(c)) => c.ty.rsplit('/').next() == Some(ty.name.as_str()) && *c.name == *case.name,
        (ast::Pattern::Number { negative, text, .. }, V::Num(n)) => (if *negative { format!("-{}", text) } else { text.clone() }) == n.show(),
        (ast::Pattern::Text(t), V::Text(s)) => t.parts.iter().map(|p| if let ast::TextPart::Piece { raw, .. } = p { raw.clone() } else { String::new() }).collect::<String>() == **s,
        (ast::Pattern::Bool(b, _), V::Bool(x)) => b == x,
        _ => false,
    }
}

/// A TODO above the item at `i` of the list `key`: a line of its own in prose, else a comment above it.
fn put_todo(items: &mut Vec<Value>, i: usize, key: &str, what: &str) {
    if key == "prose" || key == "markdown" {
        let line = format!("@// TODO(xtmd migrate): {}", what);
        if i > 0 && items[i - 1]["comment"] == line.as_str() {
            return;
        }
        let span = items[i]["span"].clone();
        items.insert(i, json!({ "comment": line, "span": span }));
        return;
    }
    let line = format!("// TODO(xtmd migrate): {}", what);
    let node = &mut items[i];
    let tagged = node.as_object().map_or(false, |o| o.len() == 1 && !o.contains_key("span") && o.values().all(Value::is_object));
    let node = if tagged { node.as_object_mut().unwrap().values_mut().next().unwrap() } else { node };
    let Some(o) = node.as_object_mut() else { return };
    let c = o.entry("comments").or_insert_with(|| json!({}));
    let Some(c) = c.as_object_mut() else { return };
    let l = c.entry("leading").or_insert_with(|| json!([]));
    if let Some(l) = l.as_array_mut() {
        if !l.iter().any(|x| x == line.as_str()) {
            l.push(json!(line));
        }
    }
}

/// Where the first thing in `v` that has a place starts.
fn first_start(v: &Value) -> Option<u64> {
    match v {
        Value::Object(o) => o.get("span").and_then(|s| s[0].as_u64()).or_else(|| o.values().filter_map(first_start).min()),
        Value::Array(a) => a.iter().filter_map(first_start).min(),
        _ => None,
    }
}

/// A pack's own files in the build, its surfaces left out.
fn pack_files(sources: &[Source], pack: &str) -> Vec<Source> {
    sources
        .iter()
        .filter(|s| crate::syntax::parse(&s.text, crate::syntax::Entry::Xtmd).file.map_or(false, |f| f.surface.is_none() && f.package.map_or(false, |p| p.item.name == pack)))
        .cloned()
        .collect()
}

/// `{ key: value, "span": span }`.
fn obj(key: &str, value: Value, span: &Value) -> Value {
    let mut o = Map::new();
    o.insert(key.to_string(), value);
    o.insert("span".into(), span.clone());
    Value::Object(o)
}

/// The last part of an id: a member's own name.
fn last(id: &str) -> &str {
    id.rsplit('.').next().unwrap_or(id)
}

/// A renamed declaration's key: a fn on a type keeps its type, `text.slug` to `text.slugify`.
fn rename_key(old: &str, to: &str) -> String {
    match old.rsplit_once('.') {
        Some((on, _)) if !to.contains('.') => format!("{}.{}", on, to),
        _ => to.to_string(),
    }
}

fn decl_name(_d: &Decl, key: &str) -> String {
    key.to_string()
}

/// A value as a person reads it in what the run says.
fn shown(v: &Value) -> String {
    printed(v, &v["type"]).unwrap_or_else(|| "…".into())
}

/// A use as the file had it, for the comment it stays as.
fn printed_use(v: &Value, prose: bool) -> String {
    let item = obj(if prose { "call" } else { "use" }, v.clone(), &v["span"]);
    if prose {
        let file = json!({ "path": "kept.xtmd", "kind": "document", "imports": [], "declarations": [], "prose": [item] });
        crate::decompile::file(&file).map(|p| p.text.trim_start_matches("@@@\n").trim().to_string()).unwrap_or_default()
    } else {
        crate::migrate::rir::v1::walk::printed(&[item]).join("\n")
    }
}

/// A text a use gave, as the block of a slot: its words as prose, or a hole of what makes it.
fn as_block(v: &Value, prose: bool, span: &Value) -> Option<Value> {
    if !prose {
        return None;
    }
    let item = match v["text"].as_str() {
        Some(t) => json!({ "markdown": { "markdown": t, "holes": [] }, "span": span }),
        None if v["type"]["name"] == "core/text" => json!({ "markdown": { "markdown": "\u{fffc}", "holes": [v] }, "span": span }),
        None => return None,
    };
    Some(json!({ "prose": [item], "span": span }))
}

/// A slot's block as a text, where it holds one paragraph of plain words, with holes or none: its
/// lines are one text, as Markdown reads them.
fn as_text(s: &Value) -> Option<Value> {
    let items = s["prose"].as_array()?;
    let [one] = items.as_slice() else { return None };
    let m = &one["markdown"];
    let t = m["markdown"].as_str()?;
    let lines: Vec<&str> = t.lines().map(str::trim).collect();
    let block = |l: &str| l.is_empty() || l.starts_with(['#', '>', '-', '+', '|']) || l.split_once(". ").map_or(false, |(n, _)| n.chars().all(|c| c.is_ascii_digit()));
    // Markdown's own marks, and a paragraph more, aren't a text's
    if lines.iter().any(|l| block(l)) || t.chars().any(|c| matches!(c, '*' | '_' | '`' | '[' | '<' | '\\')) {
        return None;
    }
    let words = lines.join(" ");
    let holes = m["holes"].as_array().cloned().unwrap_or_default();
    if holes.is_empty() {
        return Some(json!({ "text": words, "type": { "name": "core/text" }, "span": one["span"] }));
    }
    let pieces: Vec<&str> = words.split('\u{fffc}').collect();
    if pieces.len() != holes.len() + 1 {
        return None;
    }
    Some(json!({ "template": { "pieces": pieces, "holes": holes }, "type": { "name": "core/text" }, "span": one["span"] }))
}

/// Whether code reads a local of this name.
fn reads_local(v: &Value, name: &str) -> bool {
    match v {
        Value::Object(o) => o.get("get").map_or(false, |g| g["local"] == name) || o.values().any(|x| reads_local(x, name)),
        Value::Array(a) => a.iter().any(|x| reads_local(x, name)),
        _ => false,
    }
}

/// Each top-level name of `packs` the file's strings name, as its pack and its name.
fn ids(v: &Value, packs: &BTreeSet<String>, out: &mut BTreeSet<(String, String)>) {
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                if matches!(k.as_str(), "text" | "markdown" | "comment" | "comments" | "left_out" | "imports") {
                    continue;
                }
                if let Some((p, rest)) = x.as_str().and_then(|s| s.split_once('/')) {
                    if packs.contains(p) {
                        out.insert((p.to_string(), rest.split('.').next().unwrap_or(rest).to_string()));
                    }
                }
                ids(x, packs, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| ids(x, packs, out)),
        _ => {}
    }
}
