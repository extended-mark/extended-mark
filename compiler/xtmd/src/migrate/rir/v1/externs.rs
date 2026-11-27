//! A4: every extern widget says what it is as Markdown. One that said nothing wrote nothing, which is
//! `markdown { }`. A surface holds signatures only, and is left as it is.

use serde_json::Value;

pub fn markdown(v: &mut Value) {
    for f in v["files"].as_array_mut().into_iter().flatten() {
        if f["kind"] == "surface" {
            continue;
        }
        for d in f.get_mut("declarations").and_then(Value::as_array_mut).into_iter().flatten() {
            if let Some(w) = d.get_mut("extern_widget").and_then(Value::as_object_mut) {
                w.entry("markdown").or_insert_with(|| Value::Array(Vec::new()));
            }
        }
    }
}
