// Written by xtmd-schemagen from schemas/tree.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::ir::Empty;

/// What changed in one node. What isn't here is as it was. A widget that failed, or draws again, is another node to its parent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    /// The inputs whose widgets are others now, or in another order.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub children: BTreeMap<String, Vec<Child>>,
    /// The events now, when they changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<String>>,
    /// The node, or the page itself, whose id is empty and whose one input is `nodes`.
    pub id: String,
    /// The inputs that changed, whole.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, Input>,
    /// The `mut` inputs now, when what a write has to fit changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writes: Option<BTreeMap<String, Contract>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Child {
    #[serde(rename = "keep")]
    Keep(String),
    /// A node drawn anew.
    #[serde(rename = "node")]
    Node(Node),
}

/// What a value has to fit. Every part that is given has to hold.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<Ends>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub only: Vec<Input>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<Ends>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<Input>,
}

/// The page's data: what a host keeps to put a page back as it was. Plain values only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Data {
    /// Each widget's own variables, by the place it is drawn at.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub own: BTreeMap<String, BTreeMap<String, Input>>,
    /// `$state`, by field.
    pub state: BTreeMap<String, Input>,
}

/// Both ends are included; a missing one is open.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ends {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<Input>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<Input>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputRecord {
    pub fields: BTreeMap<String, Input>,
    #[serde(rename = "type")]
    pub ty: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputCase {
    #[serde(rename = "enum")]
    pub enum_: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, Input>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputRange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<Input>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<Input>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<Input>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputTokval {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub value: Input,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputFn {
    /// Moves when the fn may answer otherwise: what it was given, or what it read, changed.
    pub version: i64,
}

/// An input's value, as the IR writes one; and what only a drawn page holds: widgets, and a fn to call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Input {
    /// A number, exact: `0.1`.
    #[serde(rename = "num")]
    Num(String),
    #[serde(rename = "text")]
    Text(String),
    #[serde(rename = "bool")]
    Bool(bool),
    #[serde(rename = "duration")]
    Duration(String),
    #[serde(rename = "list")]
    List(Vec<Input>),
    #[serde(rename = "record")]
    Record(InputRecord),
    /// An enum's case.
    #[serde(rename = "case")]
    Case(InputCase),
    #[serde(rename = "some")]
    Some(Box<Input>),
    #[serde(rename = "none")]
    None(Empty),
    #[serde(rename = "range")]
    Range(Box<InputRange>),
    /// A token, by its name: `core/color.OK`. What it stands for is the IR's, or the host's own.
    #[serde(rename = "token")]
    Token(String),
    /// A token made from a plain value: `stroke.PX(2.5)`.
    #[serde(rename = "tokval")]
    Tokval(Box<InputTokval>),
    /// Widgets: a slot's, a rich_text's or a view's. A rich_text's are drawn among words.
    #[serde(rename = "nodes")]
    Nodes(Vec<Node>),
    /// A fn the host may call, by the node's id and the input's name.
    #[serde(rename = "fn")]
    Fn(InputFn),
}

/// A widget to draw.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// The events the page does something with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<String>,
    /// It couldn't be drawn, and why. Its inputs are empty, and it is drawn again once it can be.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
    /// Who it is: its own on the page, and the same for as long as it is there. A loop's widget follows its item, so a row moved keeps its id.
    pub id: String,
    /// Every input and slot by its name, worked out, each with its default when the call gave none. A slot's is the widgets in it.
    pub inputs: BTreeMap<String, Input>,
    /// The extern widget it calls, by its full name: `core/press`. Of a widget that failed, the one that did.
    pub widget: String,
    /// The `mut` inputs, which the host may write, with what a write has to fit: the variable's the call gave, or the input's own.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub writes: BTreeMap<String, Contract>,
}

/// The page now: its extern widgets in order, each with everything in it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tree {
    pub nodes: Vec<Node>,
}
