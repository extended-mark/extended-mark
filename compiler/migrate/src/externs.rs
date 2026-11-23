//! 0.3's words and extern widgets to 0.4's: `warn` is a keyword, and every extern widget says what it
//! is as Markdown.

use xtmd_syntax::ast::{DeclKind, Item, WidgetMember};
use xtmd_syntax::{names_before, parse, Entry};

use crate::step::Edit;

/// `warn` is a keyword: a name spelled so is `warn_`.
pub fn names(text: &str, entry: Entry) -> Result<Vec<Edit>, String> {
    Ok(names_before(text, entry, "warn").unwrap_or_default().into_iter().map(|s| Edit { start: s.end as usize, end: s.end as usize, text: "_".into() }).collect())
}

/// An extern widget with no markdown block wrote nothing, as `markdown { }` does.
pub fn markdown(text: &str, entry: Entry) -> Result<Vec<Edit>, String> {
    let mut out = Vec::new();
    let Some(file) = parse(text, entry).file else { return Ok(out) };
    // a surface holds signatures only
    if file.surface.is_some() {
        return Ok(out);
    }
    for n in &file.items {
        let Item::Decl(d) = &n.item else { continue };
        let DeclKind::ExternWidget(w) = &d.kind else { continue };
        match &w.body {
            // `;` ends it
            None => {
                let end = d.span.end as usize;
                let at = text[..end].rfind(';').unwrap_or(end);
                out.push(Edit { start: at, end: at + 1, text: " {\n    markdown { }\n}".into() });
            }
            Some(b) if !b.iter().any(|m| matches!(m, WidgetMember::Markdown(_))) => {
                let at = text[..b.span.end as usize].rfind('}').unwrap_or(b.span.end as usize);
                let blank = text[b.span.start as usize + 1..at].trim().is_empty();
                // on a line of its own, below what the body holds
                let (start, before) = if text[..at].ends_with('\n') { (at, if blank { "" } else { "\n" }) } else { (text[..at].trim_end().len(), if blank { "\n" } else { "\n\n" }) };
                out.push(Edit { start, end: at, text: format!("{}    markdown {{ }}\n", before) });
            }
            Some(_) => {}
        }
    }
    Ok(out)
}
