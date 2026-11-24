// Written by xtmd-schemagen from schemas/state.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The format's version.
pub const VERSION: i64 = 1;

/// The page's data: what a host keeps to put a page back as it was. Plain values only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Data {
    /// Each widget's own variables, by the place it is drawn at.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub own: BTreeMap<String, BTreeMap<String, crate::tree::Input>>,
    /// `$state`, by field.
    pub state: BTreeMap<String, crate::tree::Input>,
}
