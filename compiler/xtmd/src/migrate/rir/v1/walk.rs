//! Walking a resolved IR: every node, with where it is (a JSON pointer) and the statement or
//! declaration that holds it, which a TODO goes above.

use serde_json::Value;

/// Where a node is: its file's index, its pointer in the file, and its holder's pointer.
#[derive(Clone, Debug)]
pub struct At {
    pub file: usize,
    pub node: String,
    pub holder: String,
}

/// Every object below each file, outermost first, with where it is.
pub fn each(v: &Value, f: &mut dyn FnMut(&Value, &At)) {
    for (i, file) in v["files"].as_array().into_iter().flatten().enumerate() {
        go(file, &mut At { file: i, node: String::new(), holder: String::new() }, f);
    }
}

fn go(v: &Value, at: &mut At, f: &mut dyn FnMut(&Value, &At)) {
    match v {
        Value::Object(o) => {
            f(v, at);
            for (k, x) in o {
                if k == "comments" || k == "left_out" {
                    continue;
                }
                let node = format!("{}/{}", at.node, esc(k));
                if let Value::Array(a) = x {
                    for (i, y) in a.iter().enumerate() {
                        let item = format!("{}/{}", node, i);
                        let holder = if holds(k, y) { item.clone() } else { at.holder.clone() };
                        go(y, &mut At { file: at.file, node: item, holder }, f);
                    }
                } else {
                    go(x, &mut At { file: at.file, node, holder: at.holder.clone() }, f);
                }
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                go(x, &mut At { file: at.file, node: format!("{}/{}", at.node, i), holder: at.holder.clone() }, f);
            }
        }
        _ => {}
    }
}

/// Whether an item of the list `key` is one a TODO goes above: a statement, a declaration, a member, a
/// block of prose; never an expression, which a list of arguments or a list's items hold.
pub fn holds(key: &str, item: &Value) -> bool {
    const LISTS: &[&str] = &["declarations", "body", "else", "view", "prose", "markdown", "fields", "inputs", "slots", "events", "fns", "then"];
    LISTS.contains(&key) && item.get("span").is_some() && item.get("comment").is_none() && (key != "fields" || item.get("id").is_some())
}

fn esc(k: &str) -> String {
    k.replace('~', "~0").replace('/', "~1")
}

/// The file's path.
pub fn path(v: &Value, file: usize) -> String {
    v["files"][file]["path"].as_str().unwrap_or("").to_string()
}

/// A TODO above the node at `holder` in `file`, once for each holder and text.
pub fn todo(v: &mut Value, file: usize, holder: &str, what: &str, asked: &mut crate::migrate::rir::Asked) {
    let p = path(v, file);
    // in prose a comment is a line of its own, `@//`, in the list the block is in
    let list_key = holder.rsplit('/').nth(1).unwrap_or("");
    if list_key == "prose" || (list_key == "markdown" && v["files"][file].pointer(holder).map_or(false, |n| n.get("markdown").is_some() || n.get("call").is_some())) {
        let (list, at) = holder.rsplit_once('/').unwrap_or(("", "0"));
        let line = format!("@// TODO(xtmd migrate): {}", what);
        let (Ok(at), Some(items)) = (at.parse::<usize>(), v["files"][file].pointer_mut(list).and_then(Value::as_array_mut)) else { return };
        if at > 0 && items[at - 1]["comment"] == line.as_str() {
            return;
        }
        let span = items[at]["span"].clone();
        items.insert(at, serde_json::json!({ "comment": line, "span": span }));
        asked.todos.push(format!("{}: {}", p, what));
        return;
    }
    let line = format!("// TODO(xtmd migrate): {}", what);
    let Some(node) = v["files"][file].pointer_mut(holder) else { return };
    if node["comments"]["leading"].as_array().map_or(false, |l| l.iter().any(|x| x == line.as_str())) {
        return;
    }
    asked.todo(node, &p, what);
}

/// View statements printed as this release prints them, a line each, for a comment that keeps them.
pub fn printed(stmts: &[Value]) -> Vec<String> {
    let file = serde_json::json!({
        "path": "kept.xtmd",
        "kind": "document",
        "imports": [],
        "prose": [],
        "declarations": [{ "widget": { "id": "kept/kept", "inputs": [], "slots": [], "events": [], "view": stmts, "markdown": [], "span": [0, 0] } }],
    });
    let p = match crate::decompile::file(&file) {
        Ok(p) => p,
        Err(e) => return vec![format!("(it couldn't be printed: {})", e.join("; "))],
    };
    let lines: Vec<&str> = p.text.lines().collect();
    let Some(open) = lines.iter().position(|l| l.trim() == "view {") else { return Vec::new() };
    let depth = lines[open].len() - lines[open].trim_start().len();
    let close = lines.iter().skip(open + 1).position(|l| l.trim() == "}" && l.len() - l.trim_start().len() == depth).map_or(lines.len(), |k| open + 1 + k);
    lines[open + 1..close].iter().map(|l| l.get(depth + 4..).unwrap_or(l.trim_start()).to_string()).collect()
}

/// Every object below each file, outermost first, changed in place by `f`, which says what it leaves
/// for a person; each TODO is written above the node's holder once the walk is done.
pub fn each_mut(v: &mut Value, asked: &mut crate::migrate::rir::Asked, f: &mut dyn FnMut(&mut Value) -> Option<String>) {
    let mut todos: Vec<(usize, String, String)> = Vec::new();
    let n = v["files"].as_array().map_or(0, Vec::len);
    for i in 0..n {
        go_mut(&mut v["files"][i], String::new(), String::new(), &mut |x, holder| {
            if let Some(what) = f(x) {
                todos.push((i, holder.to_string(), what));
            }
        });
    }
    apply(v, todos, asked);
}

/// TODOs found in a walk, written: each once, the last found first, so that a line put into a list
/// moves none of the places still to be written.
pub fn apply(v: &mut Value, todos: Vec<(usize, String, String)>, asked: &mut crate::migrate::rir::Asked) {
    let mut seen = std::collections::BTreeSet::new();
    let todos: Vec<(usize, String, String)> = todos.into_iter().filter(|t| seen.insert(t.clone())).collect();
    for (file, holder, what) in todos.into_iter().rev() {
        todo(v, file, &holder, &what, asked);
    }
}

fn go_mut(v: &mut Value, node: String, holder: String, f: &mut dyn FnMut(&mut Value, &str)) {
    match v {
        Value::Object(_) => {
            f(v, &holder);
            if let Value::Object(o) = v {
                for (k, x) in o.iter_mut() {
                    if k == "comments" || k == "left_out" {
                        continue;
                    }
                    let here = format!("{}/{}", node, esc(k));
                    if let Value::Array(a) = x {
                        for (i, y) in a.iter_mut().enumerate() {
                            let item = format!("{}/{}", here, i);
                            let h = if holds(k, y) { item.clone() } else { holder.clone() };
                            go_mut(y, item, h, f);
                        }
                    } else {
                        go_mut(x, here, holder.clone(), f);
                    }
                }
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter_mut().enumerate() {
                go_mut(x, format!("{}/{}", node, i), holder.clone(), f);
            }
        }
        _ => {}
    }
}
