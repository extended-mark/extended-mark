// Written by xtmd-schemagen from schemas/check.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
pub enum MarkdownExpected {
    /// The whole page, as blocks.
    #[serde(rename = "blocks")]
    Blocks(Vec<crate::blocks::Block>),
    /// Blocks somewhere on the page.
    #[serde(rename = "has")]
    Has(Vec<crate::blocks::Block>),
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

/// What the host tells the page about where it is read.
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
