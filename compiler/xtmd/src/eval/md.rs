//! core's `md_inline`, in Rust: what the compiler reads a page's sentences with. It is
//! core/markdown.xtmdpack line for line, which engines run for the Markdown a page makes as it runs, and
//! compiler/xtmd/tests/check_md_inline.rs holds the two to the same segments.

use std::collections::HashMap;
use std::rc::Rc;

use crate::eval::interp::{Binding, Failure, Machine, R};
use crate::eval::value::{Case, Record, Value};

const HOLE: char = '\u{fffc}';
const LETTERS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const WORD: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const ASCII_PUNCT: &str = "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";
const PUNCT: &str = "¡§«¶·»¿×÷‐‑‒–—―‖‗‘’‚‛“”„‟†‡•‣․‥…‧‰‱′″‴‵‶‷‸‹›※‼‽‾‿⁀⁁⁂⁃⁄⁅⁆⁇⁈⁉⁊⁋⁌⁍⁎⁏⁐⁑⁒⁓⁔⁕⁖⁗⁘⁙⁚⁛⁜⁝⁞£€¥¢©®°±™、。〃〈〉《》「」『』【】〔〕〖〗〘〙〚〛〜〝〞〟";

/// A part of a sentence, before its segments are made.
#[derive(Clone)]
enum Part {
    Text(String),
    View(Value),
    Code(String),
    Math(Value),
    Image(String, String),
    Note(String),
    Break,
}

#[derive(Clone, Default, PartialEq)]
struct Marks {
    bold: bool,
    italic: bool,
    strike: bool,
    link: Option<String>,
}

#[derive(Clone)]
struct Piece {
    part: Part,
    marks: Marks,
    run: Option<char>,
    left: i64,
    size: i64,
    opens: bool,
    closes: bool,
    live: bool,
    gone: bool,
    hole: bool,
}

impl Piece {
    fn new(part: Part) -> Piece {
        Piece { part, marks: Marks::default(), run: None, left: 0, size: 0, opens: false, closes: false, live: false, gone: false, hole: false }
    }

    fn linked(text: String, link: String) -> Piece {
        Piece { marks: Marks { link: Some(link), ..Marks::default() }, ..Piece::new(Part::Text(text)) }
    }
}

struct Bracket {
    at: usize,
    image: bool,
    start: usize,
    active: bool,
}

/// What reading a link, an autolink or an entity found: where it ends, its destination and its text.
struct Link {
    end: usize,
    dest: String,
    label: String,
}

/// A sentence read: its characters, each hole one of its own.
struct Read<'a> {
    cs: Vec<char>,
    links: &'a HashMap<Rc<str>, Rc<str>>,
    entities: &'a dyn Fn(&str) -> Option<String>,
}

/// `core/md_inline(value, links)`.
pub fn inline(m: &mut Machine, value: &Value, links: &Value) -> R<Value> {
    let wrong = || Failure::ir("md_inline given something that isn't a template");
    let Value::Record(t) = value else { return Err(wrong()) };
    let (Some(Value::List(pieces)), Some(Value::List(holes))) = (t.get("pieces"), t.get("holes")) else { return Err(wrong()) };
    let mut cs: Vec<char> = Vec::new();
    for (k, p) in pieces.items.iter().enumerate() {
        if k > 0 {
            cs.push(HOLE);
        }
        cs.extend(p.value.as_text().ok_or_else(wrong)?.chars());
    }
    let holes: Vec<Value> = holes.items.iter().map(|h| h.value.clone()).collect();
    let links = entries(links);
    let table = m.prog.lets.get("core/entities").cloned();
    let entities = |name: &str| table.as_ref().and_then(|t| entry(t, name));
    let r = Read { cs, links: &links, entities: &entities };
    match r.run(m, &holes)? {
        Some(pieces) => Ok(Value::some(segments(m, pieces)?)),
        None => Ok(Value::none()),
    }
}

/// A `map<text>`'s entries; the first of a key is the one.
fn entries(v: &Value) -> HashMap<Rc<str>, Rc<str>> {
    let mut out = HashMap::new();
    if let Value::Record(r) = v {
        if let Some(Value::List(l)) = r.get("entries") {
            for i in &l.items {
                if let Value::Record(e) = &i.value {
                    if let (Some(Value::Text(k)), Some(Value::Text(v))) = (e.get("key"), e.get("value")) {
                        out.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
            }
        }
    }
    out
}

fn entry(map: &Value, key: &str) -> Option<String> {
    let Value::Record(r) = map else { return None };
    let Some(Value::List(l)) = r.get("entries") else { return None };
    l.items.iter().find_map(|i| match &i.value {
        Value::Record(e) if e.get("key").and_then(Value::as_text) == Some(key) => e.get("value").and_then(Value::as_text).map(str::to_string),
        _ => None,
    })
}

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\u{d}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

fn is_ascii_punct(c: char) -> bool {
    ASCII_PUNCT.contains(c)
}

fn is_punct(c: char) -> bool {
    is_ascii_punct(c) || (!c.is_ascii() && PUNCT.contains(c))
}

fn trim_end(s: &str) -> &str {
    s.trim_end_matches([' ', '\t'])
}

fn flush(pieces: &mut Vec<Piece>, buf: &mut String) {
    if !buf.is_empty() {
        pieces.push(Piece::new(Part::Text(std::mem::take(buf))));
    }
}

/// A label as CommonMark matches them: case folded, spaces collapsed.
fn label_of(s: &str) -> String {
    let flat = s.replace(['\n', '\t'], " ").to_lowercase().replace('ß', "ss");
    flat.split(' ').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

impl Read<'_> {
    fn at(&self, k: usize) -> Option<char> {
        self.cs.get(k).copied()
    }

    fn join(&self, start: usize, end: usize) -> String {
        let end = end.min(self.cs.len());
        if start >= end {
            return String::new();
        }
        self.cs[start..end].iter().collect()
    }

    fn run_of(&self, at: usize, c: char) -> usize {
        self.cs[at.min(self.cs.len())..].iter().take_while(|x| **x == c).count()
    }

    /// The first character from `at` that isn't one `skip` takes; past the end, where that is.
    fn after(&self, at: usize, skip: impl Fn(char) -> bool) -> usize {
        let n = self.cs.len();
        (at..n).find(|k| !skip(self.cs[*k])).unwrap_or(at.max(n))
    }

    fn after_spaces(&self, at: usize) -> usize {
        self.after(at, |c| c == ' ' || c == '\t')
    }

    /// After spaces, tabs and at most one line break.
    fn after_blank(&self, at: usize) -> usize {
        let mut lines = 0;
        let n = self.cs.len();
        for j in at..n {
            match self.cs[j] {
                '\n' if lines == 0 => lines = 1,
                ' ' | '\t' => {}
                _ => return j,
            }
        }
        at.max(n)
    }

    fn run(&self, m: &mut Machine, holes: &[Value]) -> R<Option<Vec<Piece>>> {
        let n = self.cs.len();
        let mut pieces: Vec<Piece> = Vec::new();
        let mut brackets: Vec<Bracket> = Vec::new();
        let mut buf = String::new();
        let mut hole = 0;
        let mut i = 0;
        while i < n {
            let c = self.cs[i];
            let next = self.at(i + 1);
            let mut to = i + 1;
            match c {
                HOLE => {
                    flush(&mut pieces, &mut buf);
                    if let Some(Value::Case(h)) = holes.get(hole) {
                        let part = match (&*h.name, h.values.first().map(|(_, v)| v)) {
                            ("TEXT", Some(Value::Text(t))) => Part::Text(t.to_string()),
                            (_, Some(v)) => Part::View(v.clone()),
                            _ => return Err(Failure::ir("a hole that holds nothing")),
                        };
                        pieces.push(Piece { hole: true, ..Piece::new(part) });
                    }
                    hole += 1;
                }
                '\\' => match next {
                    Some('\n') => {
                        flush(&mut pieces, &mut buf);
                        pieces.push(Piece::new(Part::Break));
                        to = self.after_spaces(i + 2);
                    }
                    Some(p) if is_ascii_punct(p) => {
                        buf.push(p);
                        to = i + 2;
                    }
                    _ => buf.push('\\'),
                },
                '`' => {
                    let run = self.run_of(i, '`');
                    match self.find_ticks(i + run, run) {
                        Some(close) => {
                            flush(&mut pieces, &mut buf);
                            pieces.push(Piece::new(Part::Code(self.code_text(i + run, close))));
                            to = close + run;
                        }
                        None => {
                            buf.push_str(&self.join(i, i + run));
                            to = i + run;
                        }
                    }
                }
                '$' if !matches!(next, None | Some(' ' | '\t' | '\n' | '$')) => match self.maths_close(i + 1) {
                    Some(close) => {
                        let (tex, used) = self.template(m, holes, i + 1, close, hole)?;
                        match m.call("core/latex.from", vec![Binding::Value(tex)])? {
                            Some(Value::Opt(Some(l))) => {
                                flush(&mut pieces, &mut buf);
                                pieces.push(Piece::new(Part::Math((*l).clone())));
                                hole += used;
                                to = close + 1;
                            }
                            _ => return Ok(None),
                        }
                    }
                    None => buf.push('$'),
                },
                '*' | '_' | '~' => {
                    let run = self.run_of(i, c);
                    let before = if i == 0 { ' ' } else { self.cs[i - 1] };
                    let after = self.at(i + run).unwrap_or(' ');
                    let left = !is_space(after) && (!is_punct(after) || is_space(before) || is_punct(before));
                    let right = !is_space(before) && (!is_punct(before) || is_space(after) || is_punct(after));
                    let (mut opens, mut closes) = (left, right);
                    if c == '_' {
                        opens = left && (!right || is_punct(before));
                        closes = right && (!left || is_punct(after));
                    }
                    if c == '~' && run > 2 {
                        opens = false;
                        closes = false;
                    }
                    let text = self.join(i, i + run);
                    if !opens && !closes {
                        buf.push_str(&text);
                    } else {
                        flush(&mut pieces, &mut buf);
                        pieces.push(Piece { run: Some(c), left: run as i64, size: run as i64, opens, closes, live: true, ..Piece::new(Part::Text(text)) });
                    }
                    to = i + run;
                }
                '[' if next == Some('^') && self.note_end(i).is_some() => {
                    let end = self.note_end(i).unwrap_or(i);
                    flush(&mut pieces, &mut buf);
                    pieces.push(Piece::new(Part::Note(self.join(i + 2, end))));
                    to = end + 1;
                }
                '[' | '!' if c == '[' || next == Some('[') => {
                    flush(&mut pieces, &mut buf);
                    let image = c == '!';
                    to = if image { i + 2 } else { i + 1 };
                    brackets.push(Bracket { at: pieces.len(), image, start: to, active: true });
                    pieces.push(Piece::new(Part::Text(if image { "![".into() } else { "[".into() })));
                }
                ']' => {
                    flush(&mut pieces, &mut buf);
                    match self.close_bracket(i, &brackets) {
                        Some(found) => {
                            to = found.end;
                            if let Some(b) = brackets.pop() {
                                make_link(m, &mut pieces, &b, &found)?;
                                if !b.image {
                                    // every bracket before a link can no longer make one
                                    brackets.iter_mut().filter(|b| !b.image).for_each(|b| b.active = false);
                                }
                            }
                        }
                        None => {
                            brackets.pop();
                            pieces.push(Piece::new(Part::Text("]".into())));
                        }
                    }
                }
                '<' => {
                    if let Some(auto) = self.autolink(i) {
                        flush(&mut pieces, &mut buf);
                        pieces.push(Piece::linked(auto.label, auto.dest));
                        to = auto.end;
                    } else if let Some(end) = self.html(i) {
                        // raw HTML is shown as text, never run
                        buf.push_str(&self.join(i, end));
                        to = end;
                    } else {
                        buf.push('<');
                    }
                }
                '&' => match self.entity(i) {
                    Some(e) => {
                        buf.push_str(&e.label);
                        to = e.end;
                    }
                    None => buf.push('&'),
                },
                '\n' => {
                    if buf.ends_with("  ") {
                        buf.truncate(trim_end(&buf).len());
                        flush(&mut pieces, &mut buf);
                        pieces.push(Piece::new(Part::Break));
                    } else {
                        buf.truncate(trim_end(&buf).len());
                        buf.push('\n');
                    }
                    to = self.after_spaces(i + 1);
                }
                'w' | 'h' | 'f' if self.word_start(i) => match self.bare_link(i) {
                    Some(bare) => {
                        flush(&mut pieces, &mut buf);
                        pieces.push(Piece::linked(bare.label, bare.dest));
                        to = bare.end;
                    }
                    None => buf.push(c),
                },
                '@' if self.email_end(i).is_some() => {
                    let end = self.email_end(i).unwrap_or(i);
                    let local = buf.chars().rev().take_while(|c| WORD.contains(*c) || ".+-_".contains(*c)).count();
                    if local > 0 {
                        let cut = buf.char_indices().rev().nth(local - 1).map_or(0, |(k, _)| k);
                        let address = format!("{}@{}", &buf[cut..], self.join(i + 1, end));
                        buf.truncate(cut);
                        flush(&mut pieces, &mut buf);
                        let link = format!("mailto:{}", address);
                        pieces.push(Piece::linked(address, link));
                        to = end;
                    } else {
                        buf.push('@');
                    }
                }
                _ => buf.push(c),
            }
            i = to;
        }
        buf.truncate(trim_end(&buf).len());
        flush(&mut pieces, &mut buf);
        emphasis(&mut pieces, 0);
        Ok(Some(pieces))
    }

    /// Where a closing run of exactly `count` backticks starts, from `at`.
    fn find_ticks(&self, at: usize, count: usize) -> Option<usize> {
        let mut k = at;
        while k < self.cs.len() {
            if self.cs[k] == '`' {
                let run = self.run_of(k, '`');
                if run == count {
                    return Some(k);
                }
                k += run;
            } else {
                k += 1;
            }
        }
        None
    }

    fn code_text(&self, start: usize, end: usize) -> String {
        let raw = self.join(start, end).replace('\n', " ");
        if raw.chars().count() >= 2 && raw.starts_with(' ') && raw.ends_with(' ') && !raw.trim().is_empty() {
            return raw[1..raw.len() - 1].to_string();
        }
        raw
    }

    /// The `$` that closes maths opened before `at`, on the same line.
    fn maths_close(&self, at: usize) -> Option<usize> {
        for k in at..self.cs.len() {
            let c = self.cs[k];
            if c == '\n' {
                return None;
            }
            let before = self.cs[k - 1];
            if c == '$' && before != '\\' {
                let after = self.at(k + 1).unwrap_or(' ');
                return (before != ' ' && before != '\t' && !after.is_ascii_digit()).then_some(k);
            }
        }
        None
    }

    /// The characters from `start` to `end` as a template, its holes taken in order from `first`; and how
    /// many it took.
    fn template(&self, m: &mut Machine, holes: &[Value], start: usize, end: usize, first: usize) -> R<(Value, usize)> {
        let (mut pieces, mut taken) = (Vec::new(), Vec::new());
        let mut piece = String::new();
        let mut at = first;
        let mut k = start;
        while k < end {
            let c = self.cs[k];
            if c == HOLE {
                pieces.push(Value::text(&std::mem::take(&mut piece)));
                taken.extend(holes.get(at).cloned());
                at += 1;
            } else if c == '\\' && self.at(k + 1) == Some('\\') && self.at(k + 2) == Some('{') {
                // in maths, \\{ gives TeX \{
                piece.push_str("\\{");
                k += 2;
            } else {
                piece.push(c);
            }
            k += 1;
        }
        pieces.push(Value::text(&piece));
        let used = taken.len();
        let (pieces, holes) = (m.make_list(pieces)?, m.make_list(taken)?);
        Ok((record("core/template", vec![("pieces", pieces), ("holes", holes)]), used))
    }

    /// The `]` that ends a footnote mark `[^label]` opened at `at`.
    fn note_end(&self, at: usize) -> Option<usize> {
        for k in at + 2..self.cs.len() {
            let c = self.cs[k];
            if c == ']' {
                return (k > at + 2).then_some(k);
            }
            if is_space(c) || c == '[' || c == HOLE {
                return None;
            }
        }
        None
    }

    fn word_start(&self, at: usize) -> bool {
        let before = if at == 0 { ' ' } else { self.cs[at - 1] };
        is_space(before) || matches!(before, '(' | '*' | '_' | '~')
    }

    /// A bare link: www., http://, https:// or ftp://, up to a space or a <, without trailing punctuation.
    fn bare_link(&self, at: usize) -> Option<Link> {
        let head = self.join(at, at + 8).to_lowercase();
        let scheme = if head.starts_with("www.") {
            "http://"
        } else if head.starts_with("http://") || head.starts_with("https://") || head.starts_with("ftp://") {
            ""
        } else {
            return None;
        };
        let end = self.after(at, |c| !(is_space(c) || c == '<' || c == HOLE));
        let count = |upto: usize, c: char| self.cs[at..upto].iter().filter(|x| **x == c).count();
        // trailing punctuation is not part of the link, nor an unmatched )
        let mut cut = end;
        while cut > at {
            match self.cs[cut - 1] {
                '?' | '!' | '.' | ',' | ':' | '*' | '_' | '~' | '\'' | '"' => cut -= 1,
                ')' if count(cut, ')') > count(cut, '(') => cut -= 1,
                _ => break,
            }
        }
        // a trailing &name; looks like an entity, so it is not part of the link
        if cut > 0 && self.cs[cut - 1] == ';' {
            let mut amp = None;
            let mut k = cut as i64 - 2;
            while k >= at as i64 {
                let c = self.cs[k as usize];
                if c == '&' {
                    amp = Some(k as usize);
                    break;
                }
                if !c.is_ascii_alphanumeric() {
                    break;
                }
                k -= 1;
            }
            if let Some(a) = amp.filter(|a| *a > at && *a + 2 < cut) {
                cut = a;
            }
        }
        let label = self.join(at, cut);
        let host: &str = if scheme.is_empty() { label.split_once("://").map_or("", |(_, h)| h) } else { &label };
        let domain = host.split('/').next().unwrap_or("");
        if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') || domain.contains('_') {
            return None;
        }
        Some(Link { end: cut, dest: format!("{}{}", scheme, label), label })
    }

    /// Where a bare email address's domain ends, after the @ at `at`.
    fn email_end(&self, at: usize) -> Option<usize> {
        let mut end = self.after(at + 1, |c| WORD.contains(c) || ".-_".contains(c));
        if self.at(end - 1) == Some('.') {
            end -= 1;
        }
        let domain = self.join(at + 1, end);
        (domain.contains('.') && !domain.ends_with('-') && !domain.ends_with('_') && !domain.starts_with('.')).then_some(end)
    }

    fn autolink(&self, at: usize) -> Option<Link> {
        let end = self.angle_end(at)?;
        let body = self.join(at + 1, end);
        if let Some((scheme, _)) = body.split_once(':') {
            if (2..=32).contains(&scheme.chars().count()) && is_scheme(scheme) {
                return Some(Link { end: end + 1, dest: body.clone(), label: body });
            }
        }
        let parts: Vec<&str> = body.split('@').collect();
        if parts.len() == 2 && is_email(parts[0], parts[1]) {
            return Some(Link { end: end + 1, dest: format!("mailto:{}", body), label: body });
        }
        None
    }

    fn angle_end(&self, at: usize) -> Option<usize> {
        for k in at + 1..self.cs.len() {
            let c = self.cs[k];
            if c == '>' {
                return Some(k);
            }
            if c == '<' || is_space(c) || c == HOLE {
                return None;
            }
        }
        None
    }

    /// Where raw HTML that starts at `at` ends: a tag, a comment, a processing instruction, a declaration
    /// or CDATA.
    fn html(&self, at: usize) -> Option<usize> {
        let rest = self.join(at, at + 4);
        for (open, close) in [("<!--", "-->"), ("<?", "?>"), ("<![", "]]>"), ("<!", ">")] {
            if rest.starts_with(open) {
                return self.html_until(at + open.len(), close);
            }
        }
        let closing = rest.starts_with("</");
        let name_at = if closing { at + 2 } else { at + 1 };
        if !LETTERS.contains(self.at(name_at)?) {
            return None;
        }
        // the tag's name, then attributes, then > or />, with no hole in it
        let name_end = self.after(name_at, |c| WORD.contains(c) || c == '-');
        if !matches!(self.at(name_end), Some(' ' | '\t' | '\n' | '/' | '>')) {
            return None;
        }
        let end = self.tag_end(name_at, closing)?;
        if closing && self.join(name_at, end - 1).trim().contains(' ') {
            return None;
        }
        Some(end)
    }

    fn html_until(&self, from: usize, close: &str) -> Option<usize> {
        let close: Vec<char> = close.chars().collect();
        (from..self.cs.len()).find(|k| self.cs[*k..].starts_with(&close)).map(|k| k + close.len())
    }

    /// Just after the > that ends a tag whose name starts at `at`, past quoted attribute values.
    fn tag_end(&self, at: usize, closing: bool) -> Option<usize> {
        let mut quote = None;
        for k in at..self.cs.len() {
            let c = self.cs[k];
            match quote {
                Some(q) => {
                    if c == q {
                        quote = None;
                    }
                }
                None => match c {
                    '"' | '\'' => {
                        if closing {
                            return None;
                        }
                        quote = Some(c);
                    }
                    '>' => return Some(k + 1),
                    '<' | HOLE | '`' => return None,
                    _ => {}
                },
            }
        }
        None
    }

    fn entity(&self, at: usize) -> Option<Link> {
        entity(&self.cs, at, self.entities)
    }

    /// What a ] at `at` closes: an inline link, a reference, or nothing.
    fn close_bracket(&self, at: usize, brackets: &[Bracket]) -> Option<Link> {
        let b = brackets.last().filter(|b| b.active)?;
        let label = self.join(b.start, at);
        if self.at(at + 1) == Some('(') {
            if let Some(l) = self.inline_link(at + 2) {
                return Some(l);
            }
        }
        // a full reference [text][label], a collapsed one [text][], or a shortcut [text]
        if self.at(at + 1) == Some('[') {
            if let Some(close) = self.label_end(at + 2) {
                let inner = self.join(at + 2, close);
                let key = label_of(if inner.is_empty() { &label } else { &inner });
                if let Some(dest) = self.links.get(&*key) {
                    return Some(Link { end: close + 1, dest: dest.to_string(), label });
                }
                if !inner.is_empty() {
                    return None;
                }
            }
        }
        let dest = self.links.get(&*label_of(&label))?;
        Some(Link { end: at + 1, dest: dest.to_string(), label })
    }

    /// The ] that ends a link label from `at`.
    fn label_end(&self, at: usize) -> Option<usize> {
        for k in at..self.cs.len() {
            let escaped = k >= 1 && self.cs[k - 1] == '\\' && !(k >= 2 && self.cs[k - 2] == '\\');
            if !escaped {
                match self.cs[k] {
                    ']' => return Some(k),
                    '[' => return None,
                    _ => {}
                }
            }
        }
        None
    }

    /// An inline link's (destination "title"), from just after its (.
    fn inline_link(&self, from: usize) -> Option<Link> {
        let mut k = self.after_blank(from);
        let dest;
        if self.at(k) == Some('<') {
            let close = self.dest_close(k + 1)?;
            dest = self.unescape(&self.join(k + 1, close));
            k = close + 1;
        } else {
            let end = self.dest_end(k)?;
            dest = self.unescape(&self.join(k, end));
            k = end;
        }
        let before_title = k;
        k = self.after_blank(k);
        if let Some(q @ ('"' | '\'' | '(')) = self.at(k).filter(|_| k > before_title) {
            let end = self.title_end(k + 1, q, if q == '(' { ')' } else { q })?;
            k = self.after_blank(end + 1);
        }
        (self.at(k) == Some(')')).then(|| Link { end: k + 1, dest, label: String::new() })
    }

    fn dest_close(&self, at: usize) -> Option<usize> {
        let mut escaped = false;
        for j in at..self.cs.len() {
            match self.cs[j] {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '>' => return Some(j),
                '\n' | '<' => return None,
                _ => {}
            }
        }
        None
    }

    fn dest_end(&self, at: usize) -> Option<usize> {
        let mut depth = 0;
        let mut escaped = false;
        for j in at..self.cs.len() {
            match self.cs[j] {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\u{d}' | HOLE => return (depth == 0).then_some(j),
                '(' => depth += 1,
                ')' if depth == 0 => return Some(j),
                ')' => depth -= 1,
                _ => {}
            }
        }
        (depth == 0).then_some(at.max(self.cs.len()))
    }

    fn title_end(&self, at: usize, q: char, close: char) -> Option<usize> {
        for j in at..self.cs.len() {
            let c = self.cs[j];
            if c == close && self.cs[j - 1] != '\\' {
                return Some(j);
            }
            if c == '(' && q == '(' {
                return None;
            }
        }
        None
    }

    /// Backslash escapes and entities in a destination.
    fn unescape(&self, s: &str) -> String {
        let cs: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut k = 0;
        while k < cs.len() {
            match cs[k] {
                '\\' if cs.get(k + 1).map_or(false, |c| is_ascii_punct(*c)) => {
                    out.push(cs[k + 1]);
                    k += 2;
                }
                '&' => match entity(&cs, k, self.entities) {
                    Some(e) => {
                        out.push_str(&e.label);
                        k = e.end;
                    }
                    None => {
                        out.push('&');
                        k += 1;
                    }
                },
                c => {
                    out.push(c);
                    k += 1;
                }
            }
        }
        out
    }
}

fn is_scheme(s: &str) -> bool {
    s.to_lowercase().chars().enumerate().all(|(k, c)| c.is_ascii_lowercase() || (k > 0 && (c.is_ascii_digit() || "+.-".contains(c))))
}

/// An email address's two halves, as CommonMark's autolinks allow them.
fn is_email(local: &str, host: &str) -> bool {
    !local.is_empty()
        && !host.is_empty()
        && local.chars().all(|c| WORD.contains(c) || ".!#$%&'*+/=?^_`{|}~-".contains(c))
        && host.split('.').all(|l| !l.is_empty() && !l.starts_with('-') && !l.ends_with('-') && l.chars().count() <= 63 && l.chars().all(|c| WORD.contains(c) || c == '-'))
}

/// An entity: &name; from HTML5's list, or a numeric one.
fn entity(cs: &[char], at: usize, named: &dyn Fn(&str) -> Option<String>) -> Option<Link> {
    let last = (at + 40).min(cs.len().saturating_sub(1));
    let mut end = None;
    for k in at + 1..=last {
        let c = cs[k];
        if c == ';' {
            end = Some(k);
            break;
        }
        if !(WORD.contains(c) || c == '#') {
            break;
        }
    }
    let end = end?;
    let name: String = cs[at + 1..end].iter().collect();
    let label = match name.strip_prefix('#') {
        Some(rest) => {
            let hex = rest.strip_prefix(['x', 'X']);
            let digits = hex.unwrap_or(rest).to_lowercase();
            if digits.is_empty() || digits.chars().count() > if hex.is_some() { 6 } else { 7 } {
                return None;
            }
            // an invalid or zero code is the replacement character, as CommonMark has it
            let code = u32::from_str_radix(&digits, if hex.is_some() { 16 } else { 10 }).ok()?;
            char::from_u32(code).filter(|_| code != 0).unwrap_or('\u{fffd}').to_string()
        }
        None => named(&name)?,
    };
    Some(Link { end: end + 1, dest: String::new(), label })
}

/// A link: the pieces after its bracket get its link mark; an image: they become its alt text.
fn make_link(m: &mut Machine, pieces: &mut [Piece], b: &Bracket, found: &Link) -> R<()> {
    emphasis(pieces, b.at + 1);
    if b.image {
        let mut alt = String::new();
        for x in pieces[b.at + 1..].iter_mut().filter(|x| !x.gone) {
            // the alt text is plain: a run's characters only where no emphasis used them
            match x.run {
                None => match &x.part {
                    Part::Text(t) | Part::Code(t) => alt.push_str(t),
                    Part::Math(l) => {
                        if let Some(Value::Text(t)) = m.call("core/latex.show", vec![Binding::Value(l.clone())])? {
                            alt.push_str(&t);
                        }
                    }
                    Part::Image(_, a) => alt.push_str(a),
                    Part::Break => alt.push(' '),
                    Part::Note(_) | Part::View(_) => {}
                },
                Some(c) => (0..x.left).for_each(|_| alt.push(c)),
            }
            x.gone = true;
        }
        pieces[b.at].part = Part::Image(found.dest.clone(), alt);
        pieces[b.at].live = false;
        return Ok(());
    }
    for x in pieces[b.at + 1..].iter_mut() {
        if !x.gone && x.marks.link.is_none() {
            x.marks.link = Some(found.dest.clone());
        }
        x.live = false;
    }
    pieces[b.at].gone = true;
    Ok(())
}

/// CommonMark's emphasis: each closing run, in order, finds the nearest run that can open it.
fn emphasis(pieces: &mut [Piece], bottom: usize) {
    for c in bottom..pieces.len() {
        // only a run that can close starts a search; it may close several openers, two characters at a time
        if pieces[c].run.is_none() || !pieces[c].closes {
            continue;
        }
        for _ in 0..33 {
            let closer = &pieces[c];
            if !(closer.live && closer.closes && closer.left > 0) {
                break;
            }
            let Some(o) = opener(pieces, bottom, c) else {
                if !pieces[c].opens {
                    pieces[c].live = false;
                }
                break;
            };
            let run = pieces[c].run;
            let used = match run {
                Some('~') => pieces[c].left,
                _ if pieces[c].left >= 2 && pieces[o].left >= 2 => 2,
                _ => 1,
            };
            for x in pieces[o + 1..c].iter_mut() {
                // a run inside keeps its characters, marked like the text around it
                if !x.gone && (x.run.is_none() || x.left > 0) {
                    match run {
                        Some('~') => x.marks.strike = true,
                        _ if used == 2 => x.marks.bold = true,
                        _ => x.marks.italic = true,
                    }
                }
                x.live = false;
            }
            for k in [o, c] {
                let x = &mut pieces[k];
                x.left = (x.left - used).clamp(0, 64);
                x.live = x.left > 0;
            }
        }
    }
}

/// The nearest run before `c` that can open it.
fn opener(pieces: &[Piece], bottom: usize, c: usize) -> Option<usize> {
    let closer = &pieces[c];
    (bottom..c).rev().find(|k| {
        let x = &pieces[*k];
        if !(x.live && x.opens && x.run == closer.run && x.left > 0) {
            return false;
        }
        let both = x.closes || closer.opens;
        let three = (x.size + closer.size) % 3 == 0 && !(x.size % 3 == 0 && closer.size % 3 == 0);
        let tildes = closer.run != Some('~') || x.left == closer.left;
        !(both && three) && tildes
    })
}

fn record(ty: &str, fields: Vec<(&str, Value)>) -> Value {
    Value::Record(Rc::new(Record { ty: Rc::from(ty), fields: fields.into_iter().map(|(n, v)| (Rc::from(n), v)).collect() }))
}

/// The pieces as segments: a run's characters that nothing used are text, and neighbours with the same
/// marks join.
fn segments(m: &mut Machine, pieces: Vec<Piece>) -> R<Value> {
    let mut out: Vec<(Part, Marks)> = Vec::new();
    let mut after_hole = true;
    for p in pieces.into_iter().filter(|p| !p.gone) {
        let part = match p.run {
            Some(c) => Part::Text((0..p.left).map(|_| c).collect()),
            None => p.part,
        };
        if let (Part::Text(v), Some((Part::Text(w), marks)), false, false) = (&part, out.last_mut(), p.hole, after_hole) {
            if *marks == p.marks {
                w.push_str(v);
                continue;
            }
        }
        if p.run.is_some() && matches!(&part, Part::Text(t) if t.is_empty()) {
            continue;
        }
        out.push((part, p.marks));
        after_hole = p.hole;
    }
    let case = |name: &str, values: Vec<(&str, Value)>| Value::Case(Rc::new(Case { ty: Rc::from("core/md_part"), name: Rc::from(name), values: values.into_iter().map(|(n, v)| (Rc::from(n), v)).collect() }));
    let values = out
        .into_iter()
        .map(|(part, marks)| {
            let part = match part {
                Part::Text(t) => case("TEXT", vec![("value", Value::text(&t))]),
                Part::View(v) => case("VIEW", vec![("child", v)]),
                Part::Code(t) => case("CODE", vec![("value", Value::text(&t))]),
                Part::Math(l) => case("MATH", vec![("tex", l)]),
                Part::Image(src, alt) => case("IMAGE", vec![("src", Value::text(&src)), ("alt", Value::text(&alt))]),
                Part::Note(t) => case("NOTE", vec![("mark", Value::text(&t))]),
                Part::Break => case("BREAK", Vec::new()),
            };
            let link = marks.link.as_deref().map_or(Value::none(), |l| Value::some(Value::text(l)));
            let marks = record("core/md_marks", vec![("bold", Value::Bool(marks.bold)), ("italic", Value::Bool(marks.italic)), ("strike", Value::Bool(marks.strike)), ("link", link)]);
            record("core/md_segment", vec![("part", part), ("marks", marks)])
        })
        .collect();
    m.make_list(values)
}
