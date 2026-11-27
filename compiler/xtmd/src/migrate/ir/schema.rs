//! Whether JSON fits one of the IR's schemas, or the resolved IR's: the part of JSON Schema they are
//! written in.

use serde_json::Value;

/// The first place it doesn't fit, or none.
pub fn check(v: &Value, schema: &Value) -> Option<String> {
    at(v, schema, schema, "")
}

fn at(v: &Value, s: &Value, root: &Value, path: &str) -> Option<String> {
    if let Some(r) = s.get("$ref").and_then(Value::as_str) {
        return at(v, root.pointer(r.trim_start_matches('#'))?, root, path);
    }
    let no = |what: String| Some(format!("at {}: {}", if path.is_empty() { "its top" } else { path }, what));
    if let Some(c) = s.get("const") {
        return if v == c { None } else { no(format!("{} where {} goes", v, c)) };
    }
    if let Some(all) = s.get("enum").and_then(Value::as_array) {
        return if all.contains(v) { None } else { no(format!("{} isn't one of its values", v)) };
    }
    if let Some(all) = s.get("oneOf").and_then(Value::as_array) {
        // each way is an object of one key: the way with this key says what doesn't fit
        let key = v.as_object().filter(|o| o.len() == 1).and_then(|o| o.keys().next());
        let way = key.and_then(|k| all.iter().find(|a| a["properties"].get(k).is_some()));
        return match way {
            Some(a) => at(v, a, root, path),
            None if all.iter().any(|a| at(v, a, root, path).is_none()) => None,
            None => no(format!("{} isn't known", key.map_or("what is here".into(), |k| format!("`{}`", k)))),
        };
    }
    if let Some(all) = s.get("anyOf").and_then(Value::as_array) {
        // the first way's reason, when none fits
        let mut why = None;
        for a in all {
            match at(v, a, root, path) {
                None => return None,
                Some(w) => {
                    why.get_or_insert(w);
                }
            }
        }
        return why;
    }
    let is = |t: &str| match t {
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "integer" => v.is_i64() || v.is_u64(),
        "number" => v.is_number(),
        "boolean" => v.is_boolean(),
        "null" => v.is_null(),
        _ => true,
    };
    // one type, or a list of them (`["string", "null"]`)
    let ok = match &s["type"] {
        Value::String(t) => is(t),
        Value::Array(ts) => ts.iter().filter_map(Value::as_str).any(is),
        _ => true,
    };
    if !ok {
        return no(format!("a {} goes here", s["type"].as_str().map_or_else(|| s["type"].to_string(), String::from)));
    }
    if let Value::Array(items) = v {
        let Some(item) = s.get("items") else { return None };
        return items.iter().enumerate().find_map(|(k, x)| at(x, item, root, &format!("{}/{}", path, k)));
    }
    let Value::Object(o) = v else { return None };
    if let Some(missing) = s.get("required").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).find(|r| !o.contains_key(*r)) {
        return no(format!("`{}` is missing", missing));
    }
    let props = s.get("properties").and_then(Value::as_object);
    o.iter().find_map(|(k, x)| {
        let inside = format!("{}/{}", path, k);
        match (props.and_then(|p| p.get(k)), s.get("additionalProperties")) {
            (Some(p), _) => at(x, p, root, &inside),
            (None, Some(Value::Bool(false))) => Some(format!("at {}: it isn't known", inside)),
            (None, Some(p)) if p.is_object() => at(x, p, root, &inside),
            _ => None,
        }
    })
}
