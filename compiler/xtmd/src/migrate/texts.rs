//! 0.3's text to 0.4's: `md` is `rich_text`, which holds blocks as well as sentences, and one is placed
//! rather than given to `@blocks`. A `rich_text` is a view, so what read its parts is left for a person.

use crate::check::codes;
use crate::syntax::Entry;

use crate::migrate::releases::{Changes, Files};
use crate::migrate::slots::in_prose;
use crate::migrate::step::Edit;

/// The records and enums that held what was read from Markdown.
const PARTS: [&str; 5] = ["segment", "part", "marks", "block", "list_item"];

const HELD: &str = "a rich_text is a view in 0.4: its calls are drawn, and nothing reads the parts of it";

pub fn step(f: &Files) -> Changes {
    let mut out = Changes::default();
    let mut left: Vec<(String, usize, String)> = Vec::new();
    for x in &f.compiled.diagnostics {
        let Some(text) = f.text(&x.file) else { continue };
        let d = &x.diagnostic;
        let (start, end) = (d.span.start as usize, d.span.end as usize);
        let Some(at) = text.get(start..end) else { continue };
        let edit = match d.code {
            codes::UNKNOWN_TYPE | codes::UNKNOWN_NAME if at == "md" || at.starts_with("md.") => Some(Edit { start, end: start + 2, text: "rich_text".into() }),
            codes::UNKNOWN_WIDGET if at == "blocks" => match place(text, &x.file, start) {
                Some(e) => Some(e),
                None => {
                    left.push((x.file.clone(), start, "place the rich_text here: `\\{…}` in prose, or `@name;` in code".into()));
                    None
                }
            },
            codes::UNKNOWN_TYPE if PARTS.contains(&at) || at == "rich_text" => {
                left.push((x.file.clone(), start, HELD.into()));
                None
            }
            codes::UNKNOWN_NAME if PARTS.contains(&at.split('.').next().unwrap_or(at)) => {
                left.push((x.file.clone(), start, HELD.into()));
                None
            }
            codes::UNKNOWN_FIELD if d.message.starts_with("rich_text has no field") => {
                left.push((x.file.clone(), start, HELD.into()));
                None
            }
            codes::NOT_COMPARABLE if d.message.contains("`rich_text`") => {
                left.push((x.file.clone(), start, "a rich_text is a view in 0.4, and isn't compared: compare the text it is read from, or what it shows (`.show()`)".into()));
                None
            }
            _ => None,
        };
        if let Some(e) = edit {
            if !out.edits.iter().any(|(p, o)| *p == x.file && o.start == e.start) {
                out.edits.push((x.file.clone(), e));
            }
        }
    }
    // an edit inside another waits for the next round
    let edits = std::mem::take(&mut out.edits);
    out.edits = edits.iter().filter(|(p, e)| !edits.iter().any(|(q, o)| p == q && o.start <= e.start && e.end <= o.end && (o.start, o.end) != (e.start, e.end))).cloned().collect();
    if out.edits.is_empty() {
        out.todos = left;
    }
    out
}

/// `@blocks(value = x);` places `x`: `\{x}` in prose, `@x;` in code, with a `let` for what isn't a name.
fn place(text: &str, path: &str, name: usize) -> Option<Edit> {
    let at = text[..name].rfind('@')?;
    let open = name + "blocks".len();
    let open = open + text[open..].find('(').filter(|k| text[open..open + k].trim().is_empty())?;
    let close = closing(text, open)?;
    let semi = close + 1 + text[close + 1..].find(';').filter(|k| text[close + 1..close + 1 + k].trim().is_empty())?;
    let args = text[open + 1..close].trim().trim_end_matches(',').trim_end();
    let value = args.strip_prefix("value")?.trim_start().strip_prefix('=')?.trim();
    let entry = if path.ends_with(".xtmd") { Entry::Document } else { Entry::Xtmd };
    let text = if in_prose(text, entry, at) {
        if value.contains('\n') {
            return None;
        }
        format!("\\{{{}}}", value)
    } else if !value.is_empty() && value.chars().all(|c| c.is_alphanumeric() || c == '_') {
        format!("@{};", value)
    } else {
        let line = text[..at].rfind('\n').map_or(0, |k| k + 1);
        let indent = &text[line..][..text[line..].len() - text[line..].trim_start_matches(' ').len()];
        // each in a name of its own
        let n = text[..at].matches("@blocks").count();
        let held = if n == 0 { "held".to_string() } else { format!("held_{}", n + 1) };
        format!("let {}: rich_text = {};\n{}@{};", held, value, indent, held)
    };
    Some(Edit { start: at, end: semi + 1, text })
}

/// Where the parenthesis opened at `open` closes, its texts skipped.
fn closing(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0;
    let mut chars = text[open..].char_indices();
    let mut quoted = false;
    while let Some((k, c)) = chars.next() {
        match c {
            '\\' if quoted => {
                chars.next();
            }
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + k);
                }
            }
            _ => {}
        }
    }
    None
}
