//! The page as a host is given it (page 6, The host's side): the drawn tree as `tree.schema.json` says,
//! what changed in it, and the page's data, the same as every engine's. An engine in this process reads
//! [`Drawn`], the same nodes with the runtime's own values; what crosses to another reads these.

use std::collections::{BTreeMap, HashMap};

use xtmd_eval::{Failure, Value};
use xtmd_ir::ir;
use xtmd_ir::tree::{Change, Child, Contract, Data, Ends, Input, InputCase, InputFn, InputRange, InputRecord, InputTokval, Node, Tree};

use crate::page::{Drawn, Page};

type R<T> = Result<T, Failure>;

/// A fn a host calls: what it is, what its calls read, and how many times that has changed.
pub(crate) struct HeldFn {
    value: Value,
    deps: HashMap<String, u64>,
    version: i64,
}

/// A host's fns, by node and input.
pub(crate) type HeldFns = HashMap<String, HeldFn>;

impl Page {
    /// The page now, as JSON a host can draw.
    pub fn host_tree(&mut self) -> R<Tree> {
        let nodes = self.tree().to_vec();
        Ok(Tree { nodes: nodes.iter().map(|d| self.node_of(d)).collect::<R<_>>()? })
    }

    /// What changed in the tree since the last time this was asked, and the tree it makes: `last` with
    /// the changes made in it.
    pub fn host_changes(&mut self, last: &Tree) -> R<(Vec<Change>, Tree)> {
        let now = self.host_tree()?;
        let mut out = Vec::new();
        diff_nodes("", "nodes", &last.nodes, &now.nodes, &mut out);
        Ok((out, now))
    }

    /// The page's data: `$state`, and every widget's own variables.
    pub fn data(&mut self) -> R<Data> {
        let state = self.state_values().into_iter().map(|(k, v)| Ok((k, self.plain(&v)?))).collect::<R<_>>()?;
        let own = self.own_values().into_iter().map(|(p, vs)| Ok((p, vs.into_iter().map(|(k, v)| Ok((k, self.plain(&v)?))).collect::<R<BTreeMap<_, _>>>()?))).collect::<R<_>>()?;
        Ok(Data { state, own })
    }

    /// A fn input called, by its node's id, as a shade calls its shader.
    pub fn call(&mut self, id: &str, input: &str, args: &[Input]) -> R<Input> {
        let f = match self.find(id).and_then(|d| d.props.get(input).cloned()) {
            Some(Value::Opt(Some(f))) => (*f).clone(),
            Some(f) => f,
            None => Value::none(),
        };
        if !matches!(f, Value::Fn(_)) {
            return Err(Failure::ir(format!("{} has no fn `{}`", id, input)));
        }
        let at = format!("{}\n{}", id, input);
        self.held(&at, &f);
        let args = args.iter().map(|a| self.value_of(a)).collect::<R<Vec<_>>>()?;
        let (out, deps) = self.apply_reading(&f, args);
        if let Some(h) = self.held_fns().get_mut(&at) {
            h.deps.extend(deps);
        }
        match out? {
            Some(v) => self.plain(&v),
            None => Ok(Input::None(ir::Empty {})),
        }
    }

    /// A value a host gives, as the page holds it.
    pub fn value_of(&self, x: &Input) -> R<Value> {
        let json = serde_json::to_value(x).map_err(|e| Failure::ir(e.to_string()))?;
        let e: ir::Expr = serde_json::from_value(json).map_err(|e| Failure::ir(format!("widgets and fns are the page's own: {}", e)))?;
        self.value(&e)
    }

    // ---- inside

    fn node_of(&mut self, d: &Drawn) -> R<Node> {
        if let Some(why) = &d.failed {
            return Ok(Node { id: d.id.clone(), widget: d.widget.clone(), inputs: BTreeMap::new(), events: Vec::new(), writes: BTreeMap::new(), failed: Some(why.clone()) });
        }
        let mut inputs = BTreeMap::new();
        for (k, v) in &d.props {
            inputs.insert(k.clone(), self.input(d, v, &format!("{}\n{}", d.id, k))?);
        }
        for (k, kids) in &d.slots {
            inputs.insert(k.clone(), Input::Nodes(kids.iter().map(|n| self.node_of(n)).collect::<R<_>>()?));
        }
        let events = d.events.keys().filter(|e| self.listens(d, e)).cloned().collect();
        let mut writes = BTreeMap::new();
        for (name, ty) in self.mut_inputs(&d.widget) {
            let t = self.write_type(d, &name).unwrap_or(ty);
            writes.insert(name, self.contract(&t)?);
        }
        Ok(Node { id: d.id.clone(), widget: d.widget.clone(), inputs, events, writes, failed: None })
    }

    /// A value as the host is given it: the IR's form, its widgets nodes and its fns to call.
    fn input(&mut self, d: &Drawn, v: &Value, at: &str) -> R<Input> {
        if let Value::Content(_) = v {
            let kids = d.nodes(v).map(<[Drawn]>::to_vec).unwrap_or_default();
            return Ok(Input::Nodes(kids.iter().map(|n| self.node_of(n)).collect::<R<_>>()?));
        }
        Ok(match v {
            Value::Fn(_) => Input::Fn(InputFn { version: self.held(at, v) }),
            Value::List(l) => Input::List(l.items.iter().enumerate().map(|(k, i)| self.input(d, &i.value, &format!("{}[{}]", at, k))).collect::<R<_>>()?),
            Value::Record(r) => Input::Record(InputRecord { ty: r.ty.to_string(), fields: r.fields.iter().map(|(k, x)| Ok((k.to_string(), self.input(d, x, &format!("{}.{}", at, k))?))).collect::<R<_>>()? }),
            Value::Case(c) if self.is_token(&c.ty) => match c.values.first() {
                Some((_, x)) => Input::Tokval(Box::new(InputTokval { ty: c.ty.to_string(), name: c.name.to_string(), value: self.input(d, x, at)? })),
                None => Input::Token(format!("{}.{}", c.ty, c.name)),
            },
            Value::Case(c) => Input::Case(InputCase { enum_: c.ty.to_string(), name: c.name.to_string(), values: c.values.iter().map(|(k, x)| Ok((k.to_string(), self.input(d, x, &format!("{}.{}", at, k))?))).collect::<R<_>>()? }),
            Value::Opt(None) => Input::None(ir::Empty {}),
            Value::Opt(Some(x)) => Input::Some(Box::new(self.input(d, x, at)?)),
            Value::Range(r) => {
                let mut end = |x: &Option<Value>| x.as_ref().map(|x| self.input(d, x, at)).transpose();
                Input::Range(Box::new(InputRange { low: end(&r.low)?, high: end(&r.high)?, step: end(&r.step)? }))
            }
            Value::Num(n) => Input::Num(n.show()),
            Value::Text(t) => Input::Text(t.to_string()),
            Value::Bool(b) => Input::Bool(*b),
            Value::Duration(n) => Input::Duration(n.show()),
            Value::View(_) | Value::Content(_) => return Err(Failure::ir("a view isn't a value a host is given")),
        })
    }

    /// A value that holds no widgets.
    fn plain(&mut self, v: &Value) -> R<Input> {
        let none = Drawn::nothing();
        self.input(&none, v, "")
    }

    /// A fn as it is held: another when it was given other values, or something its calls read changed.
    fn held(&mut self, at: &str, f: &Value) -> i64 {
        let version = match self.held_fns().get(at) {
            None => 0,
            Some(h) => {
                let (was, deps, version) = (h.value.clone(), h.deps.clone(), h.version);
                if self.same_fn(&was, f) && self.is_current(&deps) {
                    return version;
                }
                version + 1
            }
        };
        self.held_fns().insert(at.to_string(), HeldFn { value: f.clone(), deps: HashMap::new(), version });
        version
    }

    /// What a value of a type has to fit, its aliases' contracts among them.
    fn contract(&mut self, t: &ir::Type) -> R<Contract> {
        let mut out = Contract { length: None, only: Vec::new(), range: None, step: None };
        let mut ty = Some(t.clone());
        for _ in 0..16 {
            let Some(now) = ty else { break };
            if let Some(c) = &now.contract {
                let mut worked = |e: &ir::Expr| -> R<Input> {
                    let v = self.value(e)?;
                    self.plain(&v)
                };
                let mut ends = |into: &mut Option<Ends>, b: &Option<ir::Bounds>| -> R<()> {
                    let Some(b) = b else { return Ok(()) };
                    let e = into.get_or_insert(Ends { high: None, low: None });
                    for (end, x) in [(&mut e.low, &b.low), (&mut e.high, &b.high)] {
                        if let (None, Some(x)) = (&end, x) {
                            *end = Some(worked(x)?);
                        }
                    }
                    Ok(())
                };
                ends(&mut out.range, &c.range)?;
                ends(&mut out.length, &c.length)?;
                if let (None, Some(s)) = (&out.step, &c.step) {
                    out.step = Some(worked(s)?);
                }
                if out.only.is_empty() {
                    out.only = c.only.iter().map(worked).collect::<R<_>>()?;
                }
            }
            ty = self.alias(&now.name);
        }
        Ok(out)
    }
}

// ---- what changed

/// Whether a node is the one it was, whatever changed in it.
fn kept(a: &Node, b: &Node) -> bool {
    a.id == b.id && a.widget == b.widget && a.failed.is_some() == b.failed.is_some()
}

/// What changed in the widgets of one input: which they are, or what is in them.
fn diff_nodes(id: &str, input: &str, a: &[Node], b: &[Node], out: &mut Vec<Change>) {
    let before: HashMap<&str, &Node> = a.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut children = Vec::new();
    let mut inside = Vec::new();
    for n in b {
        match before.get(n.id.as_str()).filter(|was| kept(was, n)) {
            Some(was) => {
                children.push(Child::Keep(n.id.clone()));
                diff(was, n, &mut inside);
            }
            None => children.push(Child::Node(n.clone())),
        }
    }
    let as_was = a.len() == b.len() && children.iter().zip(a).all(|(c, n)| matches!(c, Child::Keep(k) if *k == n.id));
    if !as_was {
        out.push(Change { id: id.to_string(), inputs: BTreeMap::new(), children: BTreeMap::from([(input.to_string(), children)]), events: None, writes: None });
    }
    out.extend(inside);
}

/// Whether two values differ only in what is in their widgets, whose changes are told apart.
fn diff_input(a: &Input, b: &Input, out: &mut Vec<Change>) -> bool {
    match (a, b) {
        (Input::Nodes(x), Input::Nodes(y)) => {
            if x.len() != y.len() || !x.iter().zip(y).all(|(p, q)| kept(p, q)) {
                return false;
            }
            x.iter().zip(y).for_each(|(p, q)| diff(p, q, out));
            true
        }
        (Input::List(x), Input::List(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| diff_input(p, q, out)),
        (Input::Some(x), Input::Some(y)) => diff_input(x, y, out),
        (Input::Record(x), Input::Record(y)) => x.ty == y.ty && x.fields.len() == y.fields.len() && x.fields.iter().all(|(k, p)| y.fields.get(k).map_or(false, |q| diff_input(p, q, out))),
        _ => a == b,
    }
}

/// What changed in a node that is still there.
fn diff(a: &Node, b: &Node, out: &mut Vec<Change>) {
    let mut own = Change { id: b.id.clone(), inputs: BTreeMap::new(), children: BTreeMap::new(), events: None, writes: None };
    let mut inside = Vec::new();
    for (k, x) in &b.inputs {
        match (a.inputs.get(k), x) {
            (Some(Input::Nodes(was)), Input::Nodes(now)) => {
                let mut found = Vec::new();
                diff_nodes(&b.id, k, was, now, &mut found);
                for c in found {
                    if c.id == b.id {
                        own.children.extend(c.children);
                    } else {
                        inside.push(c);
                    }
                }
            }
            (was, x) => {
                let mut deeper = Vec::new();
                if was.map_or(false, |was| diff_input(was, x, &mut deeper)) {
                    inside.extend(deeper);
                } else {
                    own.inputs.insert(k.clone(), x.clone());
                }
            }
        }
    }
    if a.events != b.events {
        own.events = Some(b.events.clone());
    }
    if a.writes != b.writes {
        own.writes = Some(b.writes.clone());
    }
    if !own.inputs.is_empty() || !own.children.is_empty() || own.events.is_some() || own.writes.is_some() {
        out.push(own);
    }
    out.extend(inside);
}

/// A tree with changes made in it: what a host that keeps its own copy does with them.
pub fn apply(tree: &Tree, changes: &[Change]) -> Tree {
    let by: HashMap<&str, &Change> = changes.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut old = HashMap::new();
    fn note<'a>(n: &'a Node, old: &mut HashMap<&'a str, &'a Node>) {
        old.insert(n.id.as_str(), n);
        n.inputs.values().for_each(|x| each(x, old));
    }
    fn each<'a>(x: &'a Input, old: &mut HashMap<&'a str, &'a Node>) {
        match x {
            Input::Nodes(ns) => ns.iter().for_each(|n| note(n, old)),
            Input::List(xs) => xs.iter().for_each(|x| each(x, old)),
            Input::Record(r) => r.fields.values().for_each(|x| each(x, old)),
            Input::Case(c) => c.values.values().for_each(|x| each(x, old)),
            Input::Some(x) => each(x, old),
            _ => {}
        }
    }
    tree.nodes.iter().for_each(|n| note(n, &mut old));
    struct Made<'a> {
        by: HashMap<&'a str, &'a Change>,
        old: HashMap<&'a str, &'a Node>,
    }
    impl Made<'_> {
        fn child(&self, c: &Child) -> Option<Node> {
            match c {
                Child::Keep(id) => self.old.get(id.as_str()).map(|n| self.node(n)),
                Child::Node(n) => Some(n.clone()),
            }
        }

        fn input(&self, x: &Input) -> Input {
            match x {
                Input::Nodes(ns) => Input::Nodes(ns.iter().map(|n| self.node(n)).collect()),
                Input::List(xs) => Input::List(xs.iter().map(|x| self.input(x)).collect()),
                Input::Record(r) => Input::Record(InputRecord { ty: r.ty.clone(), fields: r.fields.iter().map(|(k, x)| (k.clone(), self.input(x))).collect() }),
                Input::Some(x) => Input::Some(Box::new(self.input(x))),
                other => other.clone(),
            }
        }

        fn node(&self, n: &Node) -> Node {
            let mut out = n.clone();
            out.inputs = n.inputs.iter().map(|(k, x)| (k.clone(), self.input(x))).collect();
            if let Some(c) = self.by.get(n.id.as_str()) {
                out.inputs.extend(c.inputs.clone());
                for (k, kids) in &c.children {
                    out.inputs.insert(k.clone(), Input::Nodes(kids.iter().filter_map(|c| self.child(c)).collect()));
                }
                if let Some(e) = &c.events {
                    out.events = e.clone();
                }
                if let Some(w) = &c.writes {
                    out.writes = w.clone();
                }
            }
            out
        }
    }
    let made = Made { by, old };
    match made.by.get("").and_then(|c| c.children.get("nodes")) {
        Some(top) => Tree { nodes: top.iter().filter_map(|c| made.child(c)).collect() },
        None => Tree { nodes: tree.nodes.iter().map(|n| made.node(n)).collect() },
    }
}
