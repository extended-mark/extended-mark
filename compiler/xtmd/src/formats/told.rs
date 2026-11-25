// Written by xtmd-schemagen from schemas/told.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};

/// The format's version.
pub const VERSION: i64 = 1;

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
    pub args: Vec<crate::formats::tree::Input>,
    pub name: String,
}

/// A fn of the host's the page called, with its arguments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub call: CallCall,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<Broken>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refused: Vec<Broken>,
}

/// What a page tells its host, since it was last taken.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Told {
    /// The host's fns the page called, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<Call>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<Broken>,
    /// Writes that were refused: nothing changed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refused: Vec<Broken>,
    /// The `warn`s found false.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Broken>,
}
