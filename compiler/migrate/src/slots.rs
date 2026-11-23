//! 0.3's slots to 0.4's: a call left a slot out, or gave a plain block to a widget with several.
//!
//! A left-out slot of a widget these files declare becomes `opt`, and the widget's reads of it open it.
//! One of another pack's widget is given an empty block, where it may be empty. A slot that must hold
//! something is left for a person.

use std::collections::BTreeSet;

use xtmd_check::{SlotFind, SlotUse};
use xtmd_syntax::ast::{self, DeclKind, Item, WidgetMember};
use xtmd_syntax::{parse, Entry};

use crate::releases::{Changes, Files};
use crate::step::Edit;

/// Whether a slot may hold nothing: any content, or a list with no least length.
fn may_be_empty(ty: &xtmd_ir::ir::Type) -> bool {
    match ty.name.as_str() {
        "core/view" => true,
        "core/list" => match ty.contract.as_ref().and_then(|c| c.length.as_ref()).and_then(|l| l.low.as_ref()) {
            None => true,
            Some(xtmd_ir::ir::Expr::Num(n)) => n == "0",
            Some(_) => false,
        },
        _ => false,
    }
}

fn short(full: &str) -> &str {
    full.rsplit('/').next().unwrap_or(full)
}

pub fn step(f: &Files) -> Changes {
    let mut out = Changes::default();
    let finds = &f.compiled.slots;
    // a plain block first: what it leaves out is only known once it has a name
    for x in finds.iter().filter(|x| x.kind == SlotUse::PlainBlock) {
        if let Some(e) = f.text(&x.file).and_then(|t| name_the_block(t, x)) {
            out.edits.extend(e.into_iter().map(|e| (x.file.clone(), e)));
        }
    }
    if !out.edits.is_empty() {
        return out;
    }
    let mut opened: BTreeSet<(String, String)> = BTreeSet::new();
    for x in finds.iter().filter(|x| x.kind == SlotUse::LeftOut) {
        let Some(text) = f.text(&x.file) else { continue };
        let own = f.files.iter().any(|s| s.path == x.declared);
        if !may_be_empty(&x.ty) {
            out.todos.push((x.file.clone(), x.at.start as usize, format!("`{}` needs its slot `{}`, which this call leaves out: fill it, or make it `opt` in the widget", short(&x.widget), x.slot)));
        } else if own {
            if opened.insert((x.widget.clone(), x.slot.clone())) {
                open_slot(f, x, &mut out);
            }
        } else if let Some(e) = fill_empty(text, x, finds) {
            if !out.edits.iter().any(|(p, o)| *p == x.file && o.start == e.start && o.end == e.end) {
                out.edits.push((x.file.clone(), e));
            }
        } else {
            out.todos.push((x.file.clone(), x.at.start as usize, format!("give `{}` its slot `{}`", short(&x.widget), x.slot)));
        }
    }
    out
}

/// The widget's declaration, in the file it is in.
fn declaration<'a>(file: &'a ast::File, name: &str) -> Option<&'a ast::WidgetDecl> {
    file.items.iter().find_map(|n| match &n.item {
        Item::Decl(d) => match &d.kind {
            DeclKind::Widget(w) | DeclKind::ExternWidget(w) if w.name.name == name => Some(w),
            _ => None,
        },
        _ => None,
    })
}

/// `slot x: T` becomes `slot x: opt<T>`, and the widget's own reads of it open it.
fn open_slot(f: &Files, x: &SlotFind, out: &mut Changes) {
    let Some(text) = f.text(&x.declared) else { return };
    let entry = if x.declared.ends_with(".xtmd") { Entry::Document } else { Entry::Xtmd };
    let Some(file) = parse(text, entry).file else { return };
    let Some(slot) = declaration(&file, short(&x.widget)).and_then(|w| w.body.as_ref()).and_then(|b| {
        b.iter().find_map(|m| match m {
            WidgetMember::Slot(s) if s.name.name == x.slot => Some(s.clone()),
            _ => None,
        })
    }) else {
        return;
    };
    let ty = slot.ty.span;
    let written = text[ty.range()].to_string();
    out.edits.push((x.declared.clone(), Edit { start: ty.start as usize, end: ty.end as usize, text: format!("opt<{}>", written) }));
    let given = format!("{}_given", x.slot);
    for r in f.compiled.slots.iter().filter(|r| r.kind == SlotUse::Read && r.widget == x.widget && r.slot == x.slot) {
        let Some(text) = f.text(&r.file) else { continue };
        let (start, end) = (r.at.start as usize, r.at.end as usize);
        let line = text[..start].rfind('\n').map_or(0, |k| k + 1);
        let indent = &text[line..][..text[line..].len() - text[line..].trim_start_matches(' ').len()];
        let before = text[line..start].trim();
        let rest_end = text[end..].find('\n').map_or(text.len(), |k| end + k);
        let after = text[end..rest_end].trim();
        // `@body;`: placing an `opt` that holds nothing draws nothing
        if before.ends_with('@') {
            continue;
        }
        // `\{body}` on a line of its own, in a markdown block
        if before == "\\{" && after == "}" {
            out.edits.push((r.file.clone(), Edit { start: line, end: rest_end, text: format!("{i}@if (let {given}: {written} = {s}) {{\n{i}    \\{{{given}}}\n{i}}}", i = indent, s = x.slot) }));
        // anywhere else a list is read, none is no items
        } else if x.ty.name == "core/list" {
            out.edits.push((r.file.clone(), Edit { start: end, end, text: ".or([])".into() }));
        } else {
            out.todos.push((r.file.clone(), start, format!("`{}` may be left out now: open it here with `if (let …)`", x.slot)));
        }
    }
}

/// A prose call's block after it: where its `{` is and where its closing `}` line is.
fn prose_block(text: &str, x: &SlotFind) -> Option<(usize, usize, bool, String)> {
    let file = parse(text, Entry::Document).file?;
    fn walk<'a>(p: &'a ast::Prose, at: u32) -> Option<&'a ast::ProseCall> {
        p.nodes.iter().find_map(|n| match n {
            ast::ProseNode::Call(c) if c.call.name.span.start == at => Some(c),
            ast::ProseNode::Call(c) => c.block.as_ref().and_then(|(b, _)| walk(b, at)),
            ast::ProseNode::If { branches, otherwise, .. } => branches.iter().map(|(_, b, _)| b).chain(otherwise.iter().map(|(b, _)| b)).find_map(|b| walk(b, at)),
            ast::ProseNode::For { body, .. } | ast::ProseNode::Slot { body, .. } => walk(body, at),
            ast::ProseNode::Match { cases, .. } => cases.iter().find_map(|c| walk(&c.body, at)),
            _ => None,
        })
    }
    let mut proses: Vec<&ast::Prose> = file.prose.iter().collect();
    for n in &file.items {
        if let Item::Decl(d) = &n.item {
            if let DeclKind::Widget(w) | DeclKind::ExternWidget(w) = &d.kind {
                proses.extend(w.body.iter().flat_map(|b| b.iter()).filter_map(|m| if let WidgetMember::Markdown(m) = m { Some(&m.prose) } else { None }));
            }
        }
    }
    let c = proses.into_iter().find_map(|p| walk(p, x.at.start))?;
    let (b, _) = c.block.as_ref()?;
    let named = b.nodes.iter().any(|n| matches!(n, ast::ProseNode::Slot { .. }));
    let open = text[c.call.span.end as usize..].find('{')? + c.call.span.end as usize;
    let first = text[open..].find('\n')? + open + 1;
    let end = (c.span.end as usize).min(text.len());
    let close = text[..end].trim_end_matches('\n').rfind('}')?;
    let close = text[..close].rfind('\n').map_or(0, |k| k + 1);
    let line = text[..x.at.start as usize].rfind('\n').map_or(0, |k| k + 1);
    let indent = text[line..][..text[line..].len() - text[line..].trim_start_matches(' ').len()].to_string();
    Some((first, close, named, indent))
}

/// `@frame { … }` becomes `@frame { @slot body { … } }`.
fn name_the_block(text: &str, x: &SlotFind) -> Option<Vec<Edit>> {
    let (first, close, named, indent) = prose_block(text, x)?;
    if named {
        return None;
    }
    let inner = text[first..close].lines().map(|l| if l.trim().is_empty() { "\n".to_string() } else { format!("    {}\n", l) }).collect::<String>();
    // a block written flush with its call is indented under its slot's name
    let deep = text[first..close].lines().filter(|l| !l.trim().is_empty()).all(|l| l.starts_with(&format!("{}    ", indent)));
    let inner = if deep { inner } else { inner.lines().map(|l| if l.is_empty() { "\n".to_string() } else { format!("    {}\n", l) }).collect() };
    Some(vec![Edit { start: first, end: close, text: format!("{i}    @slot {s} {{\n{inner}{i}    }}\n", i = indent, s = x.slot) }])
}

/// An empty block for a slot a call leaves out: alone, or beside the slots the call names. A call that
/// leaves several out is given them all at once.
fn fill_empty(text: &str, x: &SlotFind, finds: &[SlotFind]) -> Option<Edit> {
    let left: Vec<&SlotFind> = finds.iter().filter(|o| o.kind == SlotUse::LeftOut && o.file == x.file && o.at == x.at).collect();
    if !left.iter().all(|o| may_be_empty(&o.ty)) {
        return None;
    }
    let end = x.whole.end as usize;
    let line = text[..x.at.start as usize].rfind('\n').map_or(0, |k| k + 1);
    let indent = text[line..][..text[line..].len() - text[line..].trim_start_matches(' ').len()].to_string();
    let entry = if x.file.ends_with(".xtmd") { Entry::Document } else { Entry::Xtmd };
    let prose = in_prose(text, entry, x.at.start as usize);
    if prose {
        if let Some((first, close, _, _)) = prose_block(text, x) {
            // as deep as the slots it names
            let deep = text[first..close].lines().find(|l| !l.trim().is_empty()).map_or(indent.len() + 4, |l| l.len() - l.trim_start_matches(' ').len());
            let slots: String = left.iter().map(|o| format!("{i}@slot {s} {{\n{i}}}\n", i = " ".repeat(deep), s = o.slot)).collect();
            return Some(Edit { start: close, end: close, text: slots });
        }
        let semi = text[end..].find(';').map(|k| end + k).filter(|k| text[end..*k].trim().is_empty())?;
        let slots: String = left.iter().map(|o| format!("{i}    @slot {s} {{\n{i}    }}\n", i = indent, s = o.slot)).collect();
        let block = format!(" {{\n{}{}}}", slots, indent);
        return Some(Edit { start: end, end: semi + 1, text: block });
    }
    let named: String = left.iter().map(|o| format!("{i}    {s} {{\n{i}    }}\n", i = indent, s = o.slot)).collect();
    if text[..end].ends_with('}') {
        let close = text[..end - 1].rfind('\n').map_or(end - 1, |k| k + 1);
        return Some(Edit { start: close, end: close, text: named });
    }
    let semi = text[end..].starts_with(';').then_some(end + 1)?;
    Some(Edit { start: end, end: semi, text: format!(" {{\n{}{}}}", named, indent) })
}

/// Whether a place is in prose: below a document's divider, or in a widget's markdown block.
pub fn in_prose(text: &str, entry: Entry, at: usize) -> bool {
    let at = at as u32;
    let Some(file) = parse(text, entry).file else { return false };
    if file.prose.is_some() && file.divider.map_or(true, |d| at > d.start) {
        return true;
    }
    file.items.iter().any(|n| match &n.item {
        Item::Decl(d) => match &d.kind {
            DeclKind::Widget(w) | DeclKind::ExternWidget(w) => w.body.iter().flat_map(|b| b.iter()).any(|m| matches!(m, WidgetMember::Markdown(m) if m.span.start <= at && at < m.span.end)),
            _ => false,
        },
        _ => false,
    })
}
