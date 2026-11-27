// Written by xtmd-schemagen from schemas/web.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The format's version.
pub const VERSION: i64 = 1;

fn is_false(b: &bool) -> bool {
    !*b
}

/// What a widget draws.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Drawn {
    /// Words, as they are: nothing in them is read as HTML.
    #[serde(rename = "words")]
    Words(String),
    #[serde(rename = "element")]
    Element(Element),
}

/// One element.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Element {
    /// Its attributes, as HTML writes them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attrs: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Drawn>,
    /// The widget a host's own component draws in it, by full name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    /// Only the live element can do the rest: a canvas painted, a plane measured. The widget's code is told as it enters the page, after each change, and as it leaves.
    #[serde(default, skip_serializing_if = "is_false")]
    pub hook: bool,
    /// Who it is among the ones beside it, across redraws: a widget's is its node's id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// The DOM events it listens for. What it does then is the widget's code.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub listens: Vec<String>,
    /// What the live element holds that no attribute does: a field's `value`, a box's `checked`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, Prop>,
    /// CSS properties, in CSS's spelling.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub style: BTreeMap<String, String>,
    /// It is in SVG's namespace.
    #[serde(default, skip_serializing_if = "is_false")]
    pub svg: bool,
    pub tag: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Prop {
    #[serde(rename = "text")]
    Text(String),
    #[serde(rename = "bool")]
    Bool(bool),
}
