// Written by xtmd-schemagen from schemas/blocks.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};

use crate::formats::ir::Empty;

/// The format's version.
pub const VERSION: i64 = 1;

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

/// A Markdown block.
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

/// A page as Markdown blocks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blocks {
    pub blocks: Vec<Block>,
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
