// Written by xtmd-schemagen from schemas/actions.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The format's version.
pub const VERSION: i64 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionEvent {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<crate::tree::Input>,
    /// The node, by its id.
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionWrite {
    /// The node, by its id.
    pub id: String,
    /// One of its `mut` inputs.
    pub input: String,
    pub value: crate::tree::Input,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionSet {
    /// `$state`'s fields, by name.
    pub fields: BTreeMap<String, crate::tree::Input>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionExpose {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<crate::tree::Input>,
    pub channel: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionExtern {
    pub channel: String,
    /// The channel's values, by name.
    pub values: BTreeMap<String, crate::tree::Input>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionPage {
    /// core's page's values, by name: `width`, `theme`, `today`, `now`.
    pub values: BTreeMap<String, crate::tree::Input>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRestore {
    pub data: crate::state::Data,
}

/// One change from outside.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    /// The reader's event on a widget.
    #[serde(rename = "event")]
    Event(ActionEvent),
    /// A widget writing a `mut` input: checked, then done or refused.
    #[serde(rename = "write")]
    Write(ActionWrite),
    /// The host's write to `$state`, as one change.
    #[serde(rename = "set")]
    Set(ActionSet),
    /// A fn the document exposes, called.
    #[serde(rename = "expose")]
    Expose(ActionExpose),
    /// A channel's values, given.
    #[serde(rename = "extern")]
    Extern(ActionExtern),
    /// What the host tells the page about where it is read.
    #[serde(rename = "page")]
    Page(ActionPage),
    /// The page's data put back as one change, `$state` whole or not at all.
    #[serde(rename = "restore")]
    Restore(ActionRestore),
}
