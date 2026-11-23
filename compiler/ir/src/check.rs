// Written by xtmd-schemagen from schemas/check.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::ir::Empty;

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockHeading {
    pub inline: Vec<Inline>,
    pub level: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockBulletsItems {
    pub blocks: Vec<Block>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub done: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockBullets {
    pub items: Vec<BlockBulletsItems>,
    pub ordered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockCode {
    pub lang: String,
    pub src: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BlockTableAlign {
    #[serde(rename = "START")]
    Start,
    #[serde(rename = "CENTER")]
    Center,
    #[serde(rename = "END")]
    End,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockTable {
    pub align: Vec<BlockTableAlign>,
    pub head: Vec<Vec<Inline>>,
    pub rows: Vec<Vec<Vec<Inline>>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockFootnote {
    pub blocks: Vec<Block>,
    pub mark: String,
}

/// A Markdown block, as the runner reads the page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Block {
    #[serde(rename = "paragraph")]
    Paragraph(Vec<Inline>),
    #[serde(rename = "heading")]
    Heading(BlockHeading),
    #[serde(rename = "bullets")]
    Bullets(BlockBullets),
    #[serde(rename = "quote")]
    Quote(Vec<Block>),
    #[serde(rename = "code")]
    Code(BlockCode),
    /// A maths block's TeX.
    #[serde(rename = "math")]
    Math(String),
    #[serde(rename = "table")]
    Table(BlockTable),
    #[serde(rename = "footnote")]
    Footnote(BlockFootnote),
    #[serde(rename = "divider")]
    Divider(Empty),
}

/// A rule that failed: a `require`'s message, or a contract's code, and where.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Broken {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallCall {
    pub args: Vec<crate::ir::Expr>,
    pub name: String,
}

/// An extern fn the step called, with its arguments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub call: CallCall,
}

/// What the compiler gives: its diagnostics, and the page's Markdown with its starting values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseExpect {
    pub diagnostics: Vec<Diagnostic>,
    /// The page's Markdown with its starting values, as `xtmd md` writes it, before any step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<MarkdownExpected>,
}

/// A compiler case: a source, the diagnostics it gives, and the page's Markdown with its starting values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    /// What the compiler gives: its diagnostics, and the page's Markdown with its starting values.
    pub expect: CaseExpect,
    /// `group.name`, unique.
    pub id: String,
    /// Packs to build with, beside the case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packs: Vec<String>,
    /// The .xtmd or .xtmdpack file, beside the case.
    pub source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "warning")]
    Warning,
}

/// A diagnostic, by its code and where it starts. Messages are never compared.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub code: String,
    /// Counted in characters, from 1.
    pub column: i64,
    pub line: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<DiagnosticSeverity>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InlineText {
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub strike: bool,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InlineImage {
    pub alt: String,
    pub src: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Inline {
    #[serde(rename = "text")]
    Text(InlineText),
    #[serde(rename = "code")]
    Code(String),
    #[serde(rename = "math")]
    Math(String),
    #[serde(rename = "image")]
    Image(InlineImage),
    #[serde(rename = "note")]
    Note(String),
    #[serde(rename = "break")]
    Break(Empty),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MarkdownExpected {
    /// The whole page, as blocks.
    #[serde(rename = "blocks")]
    Blocks(Vec<Block>),
    /// Blocks somewhere on the page.
    #[serde(rename = "has")]
    Has(Vec<Block>),
    /// The whole page, from a Markdown file beside the case, read as blocks.
    #[serde(rename = "file")]
    File(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mounted {
    pub props: BTreeMap<String, crate::ir::Expr>,
    /// Each slot's children, by site, in order.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, Vec<String>>,
    pub widget: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<Broken>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refused: Vec<Broken>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PageTheme {
    #[serde(rename = "LIGHT")]
    Light,
    #[serde(rename = "DARK")]
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PageWidth {
    #[serde(rename = "NARROW")]
    Narrow,
    #[serde(rename = "MEDIUM")]
    Medium,
    #[serde(rename = "WIDE")]
    Wide,
}

/// What the app tells the page about where it is read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<PageTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub today: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<PageWidth>,
}
