//! A sentence drawn as one egui text layout: its calls, the marks around its words read from the
//! widgets they are in. Links and pictures are checked before use; nothing in a value is ever read as
//! Markdown or HTML.

use std::ops::Range;

use egui::text::{LayoutJob, TextFormat};
use egui::{Align, Color32, FontFamily, FontId, Stroke};
use xtmd_runtime::Drawn;

use crate::values::{tex, text};

/// Whether a link may be followed: http, https and mailto, or a relative one. Whitespace and
/// control characters are dropped first, as browsers do, so `java\tscript:` is caught.
pub fn safe_href(url: &str) -> Option<String> {
    let plain: String = url.chars().filter(|c| !(*c as u32 <= 0x20 || *c as u32 == 0x7f)).collect();
    if plain.is_empty() {
        return None;
    }
    match scheme(&plain) {
        None => Some(url.trim().to_string()),
        Some(s) if ["http", "https", "mailto"].contains(&s.as_str()) => Some(url.trim().to_string()),
        Some(_) => None,
    }
}

/// Whether a picture may be shown: http, https, relative, or a `data:` image.
pub fn safe_src(url: &str) -> Option<String> {
    let plain: String = url.chars().filter(|c| !(*c as u32 <= 0x20 || *c as u32 == 0x7f)).collect();
    if plain.to_ascii_lowercase().starts_with("data:image/") {
        return Some(url.trim().to_string());
    }
    match scheme(&plain) {
        None if !plain.is_empty() => Some(url.trim().to_string()),
        None => None,
        Some(s) if s == "http" || s == "https" => Some(url.trim().to_string()),
        Some(_) => None,
    }
}

fn scheme(s: &str) -> Option<String> {
    let colon = s.find(':')?;
    let head = &s[..colon];
    let mut chars = head.chars();
    let first = chars.next()?;
    if first.is_ascii_alphabetic() && chars.all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c)) {
        Some(head.to_ascii_lowercase())
    } else {
        None
    }
}

/// How a run of rich text is drawn: its font, colour and the accent for links.
#[derive(Clone, Copy)]
pub struct Style {
    pub size: f32,
    pub bold: bool,
    pub mono: bool,
    pub color: Color32,
    pub accent: Color32,
    pub code_bg: Color32,
    pub line: f32,
}

/// A rich text laid out: the job, and where each link is in its text, by byte range.
pub struct Laid {
    pub job: LayoutJob,
    pub links: Vec<(Range<usize>, String)>,
}

/// What a run of a sentence is marked as.
#[derive(Clone, Debug, Default)]
struct Marks {
    bold: bool,
    italic: bool,
    strike: bool,
    link: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Words,
    Code,
    Math,
    Note,
    Image,
}

/// Words of a sentence, with every mark around them.
pub struct Run {
    words: String,
    kind: Kind,
    marks: Marks,
}

/// A sentence's calls in order: runs of words, and the other widgets among them.
pub enum Piece<'a> {
    Run(Vec<Run>),
    View(&'a Drawn),
}

/// The words of a sentence, as drawn: what a reader of the page sees.
pub fn words(nodes: &[Drawn]) -> String {
    let mut out = String::new();
    for p in pieces(nodes) {
        if let Piece::Run(runs) = p {
            runs.iter().for_each(|r| out.push_str(&r.words));
        }
    }
    out
}

/// A sentence's runs of words, and the widgets among them, in order.
pub fn pieces(nodes: &[Drawn]) -> Vec<Piece<'_>> {
    fn push(out: &mut Vec<Piece<'_>>, words: String, kind: Kind, marks: &Marks) {
        if words.is_empty() {
            return;
        }
        let run = Run { words, kind, marks: marks.clone() };
        match out.last_mut() {
            Some(Piece::Run(runs)) => runs.push(run),
            _ => out.push(Piece::Run(vec![run])),
        }
    }
    fn walk<'a>(nodes: &'a [Drawn], marks: &Marks, out: &mut Vec<Piece<'a>>) {
        for d in nodes {
            let p = |n: &str| text(d.props.get(n));
            if d.failed.is_some() {
                out.push(Piece::View(d));
                continue;
            }
            match d.widget.as_str() {
                "core/words" => push(out, p("value"), Kind::Words, marks),
                "core/code_span" => push(out, p("src"), Kind::Code, marks),
                "core/inline_math" => push(out, tex(d.props.get("tex")), Kind::Math, marks),
                // a picture's words are its alt text, which it is named by
                "core/inline_image" => push(out, p("alt"), Kind::Image, marks),
                "core/note_mark" => push(out, p("mark"), Kind::Note, marks),
                "core/line_break" => push(out, "\n".into(), Kind::Words, marks),
                "core/bold" | "core/italic" | "core/strike" | "core/link" => {
                    let mut m = marks.clone();
                    match d.widget.as_str() {
                        "core/bold" => m.bold = true,
                        "core/italic" => m.italic = true,
                        "core/strike" => m.strike = true,
                        _ => m.link = Some(p("to")),
                    }
                    walk(d.slots.get("body").map_or(&[], Vec::as_slice), &m, out);
                }
                _ => out.push(Piece::View(d)),
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, &Marks::default(), &mut out);
    out
}

/// Lays a run of a sentence's words out for egui.
pub fn layout(runs: &[Run], s: Style) -> Laid {
    let mut job = LayoutJob::default();
    let mut links = Vec::new();
    for r in runs {
        let mono = s.mono || matches!(r.kind, Kind::Code | Kind::Math);
        let size = if r.kind == Kind::Note { s.size * 0.75 } else if mono && !s.mono { s.size * 0.9 } else { s.size };
        let family = if mono { FontFamily::Monospace } else { FontFamily::Proportional };
        let mut f = TextFormat { font_id: FontId::new(size, family), color: s.color, ..Default::default() };
        f.line_height = Some(s.size * s.line);
        if r.marks.bold || s.bold {
            // egui's default fonts have no bold face: the text is drawn darker and wider by a hair
            f.extra_letter_spacing = 0.4;
            f.color = strong(s.color);
        }
        if r.marks.italic || r.kind == Kind::Image {
            f.italics = true;
        }
        if r.marks.strike {
            f.strikethrough = Stroke::new(1.0, f.color);
        }
        if r.kind == Kind::Code {
            f.background = s.code_bg;
        }
        if r.kind == Kind::Note {
            f.valign = Align::TOP;
            f.color = s.accent;
        }
        let start = job.text.len();
        let safe = r.marks.link.as_deref().and_then(safe_href);
        if safe.is_some() {
            f.color = s.accent;
            f.underline = Stroke::new(1.0, s.accent);
        }
        job.append(&r.words, 0.0, f);
        if let Some(url) = safe {
            links.push((start..job.text.len(), url));
        }
    }
    Laid { job, links }
}

/// A colour a little stronger: what bold looks like without a bold face.
fn strong(c: Color32) -> Color32 {
    let [r, g, b, a] = c.to_array();
    let lum = (r as u32 + g as u32 + b as u32) / 3;
    let push = |x: u8| if lum > 128 { x.saturating_add(24) } else { x.saturating_sub(24) };
    Color32::from_rgba_premultiplied(push(r), push(g), push(b), a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_checked() {
        assert_eq!(safe_href("https://example.org").as_deref(), Some("https://example.org"));
        assert_eq!(safe_href("java\tscript:alert(1)"), None);
        assert_eq!(safe_href("/docs").as_deref(), Some("/docs"));
        assert_eq!(safe_src("javascript:alert(1)"), None);
        assert!(safe_src("data:image/png;base64,AAAA").is_some());
    }
}
