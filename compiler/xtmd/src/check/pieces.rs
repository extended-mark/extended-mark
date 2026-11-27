//! Markdown's own pieces, which an extern may be: `markdown #quote;` (page 5). Any extern may be
//! any of them, so a piece says what it needs of the declaration, and nothing goes by a name.

use crate::check::*;

/// What a piece is made of: an input or a slot of the extern, by its name.
enum Part {
    /// An input of this type, as the IR names it.
    Input(&'static str, &'static [&'static str]),
    /// The same, which may be left out of the declaration.
    Maybe(&'static str, &'static [&'static str]),
    Slot(&'static str),
}

const WHOLE: &[&str] = &["core/int", "core/number"];
const TEXT: &[&str] = &["core/text"];
const RICH: &[&str] = &["core/rich_text"];
const BOOL: &[&str] = &["core/bool"];

fn parts(piece: &str) -> Option<&'static [Part]> {
    use Part::*;
    Some(match piece {
        "paragraph" => &[Input("value", RICH)],
        "heading" => &[Input("level", WHOLE), Input("value", RICH)],
        "bullets" => &[Maybe("ordered", BOOL), Maybe("start", WHOLE), Slot("items")],
        "item" => &[Maybe("done", &["core/opt<core/bool>", "core/bool"]), Slot("body")],
        "quote" => &[Slot("body")],
        "code" => &[Input("src", TEXT), Maybe("lang", TEXT)],
        "math" => &[Input("tex", &["core/latex"])],
        "table" => &[Input("head", &["core/list<core/rich_text>"]), Input("rows", &["core/list<core/list<core/rich_text>>"]), Maybe("align", &["core/list<core/cell_align>"])],
        "footnote" => &[Input("mark", TEXT), Slot("body")],
        "divider" => &[],
        "text" => &[Input("value", TEXT)],
        "bold" | "italic" | "strike" => &[Slot("body")],
        "link" => &[Input("to", TEXT), Slot("body")],
        "code_span" => &[Input("src", TEXT)],
        "inline_math" => &[Input("tex", &["core/latex"])],
        "inline_image" => &[Input("src", TEXT), Input("alt", TEXT)],
        "note_mark" => &[Input("mark", TEXT)],
        "line_break" => &[],
        _ => return None,
    })
}

pub const PIECES: &[&str] = &["paragraph", "heading", "bullets", "item", "quote", "code", "math", "table", "footnote", "divider", "text", "bold", "italic", "strike", "link", "code_span", "inline_math", "inline_image", "note_mark", "line_break"];

/// Whether a piece's input or slot holds a sentence, and no blocks.
pub fn takes_sentence(piece: &str, part: &str) -> bool {
    matches!((piece, part), ("paragraph" | "heading", "value") | ("table", "head" | "rows") | ("bold" | "italic" | "strike" | "link", "body"))
}

/// A type's name with its arguments', and no contracts: `core/list<core/rich_text>`.
fn bare(t: &ir::Type) -> String {
    if t.args.is_empty() {
        return t.name.clone();
    }
    format!("{}<{}>", t.name, t.args.iter().map(bare).collect::<Vec<_>>().join(", "))
}

impl Checker {
    /// `markdown #piece;`: the extern has what the piece is made of.
    pub(crate) fn check_piece(&mut self, d: DefId, piece: &ast::Ident) -> Option<String> {
        let file = self.defs[d].file;
        let Some(parts) = parts(&piece.name) else {
            self.unknown(file, codes::MARKDOWN_PIECE, piece.span, format!("Markdown has no piece `#{}`", piece.name), &piece.name, PIECES.iter().copied());
            return None;
        };
        let DefKind::Widget { inputs, slots, .. } = self.defs[d].kind.clone() else { return None };
        let widget = self.defs[d].name.clone();
        for p in parts {
            let wrong = match p {
                Part::Slot(n) => (!slots.iter().any(|s| s.name == *n)).then(|| format!("a slot `{}`", n)),
                Part::Input(n, types) | Part::Maybe(n, types) => match inputs.iter().find(|i| i.name == *n) {
                    Some(i) if types.contains(&bare(&self.unaliased(&i.ir)).as_str()) => None,
                    None if matches!(p, Part::Maybe(..)) => None,
                    _ => Some(format!("an input `{}: {}`", n, types[0].replace("core/", ""))),
                },
            };
            if let Some(what) = wrong {
                self.error(file, codes::MARKDOWN_PIECE, piece.span, format!("`#{}` is made of {}, which `{}` doesn't have", piece.name, what, widget));
                return None;
            }
        }
        Some(piece.name.clone())
    }

    /// A type with the pack's own names for it opened, its arguments' too.
    fn unaliased(&self, t: &ir::Type) -> ir::Type {
        let mut t = t.clone();
        for _ in 0..16 {
            // `int` is an alias core's pieces name
            let Some(to) = self.defs.iter().find_map(|d| match &d.kind {
                DefKind::Alias { target: Some((_, ir)) } if d.full == t.name && d.full != "core/int" => Some(ir.clone()),
                _ => None,
            }) else { break };
            t = to;
        }
        t.args = t.args.iter().map(|a| self.unaliased(a)).collect();
        t
    }

    /// `tokens color.LINE, …;`: each case by its full name.
    pub(crate) fn extern_tokens(&mut self, file: usize, refs: &[ast::TokenRef]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for r in refs {
            let Some(TypeSym::Def(d)) = self.lookup_type(file, &r.token.name) else {
                self.error(file, codes::EXTERN_TOKENS, r.token.span, format!("`{}` is not a token: an extern's `tokens` line lists the tokens its look comes from, as in `tokens color.LINE;`", r.token.name));
                continue;
            };
            let DefKind::Token { cases, .. } = self.defs[d].kind.clone() else {
                self.error(file, codes::EXTERN_TOKENS, r.token.span, format!("`{}` is not a token: an extern's `tokens` line lists the tokens its look comes from, as in `tokens color.LINE;`", r.token.name));
                continue;
            };
            self.note_ref(file, r.token.span, r.token.span, RefTarget::Decl(d));
            let full = self.defs[d].full.clone();
            match &r.case {
                // a case that carries its value is the caller's, so no look comes from it
                None => out.extend(cases.iter().filter(|c| c.fields.is_empty()).map(|c| format!("{}.{}", full, c.name))),
                Some(c) => {
                    if !cases.iter().any(|x| x.name == c.name) {
                        let names: Vec<String> = cases.iter().map(|x| x.name.clone()).collect();
                        self.unknown(file, codes::EXTERN_TOKENS, c.span, format!("`{}` has no case `{}`", r.token.name, c.name), &c.name, names.iter().map(|n| n.as_str()));
                        continue;
                    }
                    self.note_ref(file, c.span, c.span, RefTarget::Case(d, c.name.clone()));
                    out.push(format!("{}.{}", full, c.name));
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// `uses core.gfx;`: the levels drawing the extern needs, beside core.draw.
    pub(crate) fn extern_uses(&mut self, file: usize, refs: &[ast::UsesRef]) -> Vec<ir::Level> {
        let mut out = Vec::new();
        for r in refs {
            let level = match (r.pack.name.as_str(), r.level.name.as_str()) {
                ("core", "draw") => ir::Level::CoreDraw,
                ("core", "live") => ir::Level::CoreLive,
                ("core", "gfx") => ir::Level::CoreGfx,
                _ => {
                    self.error(file, codes::EXTERN_USES, r.pack.span.to(r.level.span), format!("there is no level `{}.{}`: an extern uses `core.draw`, `core.live` or `core.gfx`", r.pack.name, r.level.name));
                    continue;
                }
            };
            if !out.contains(&level) {
                out.push(level);
            }
        }
        out
    }
}
