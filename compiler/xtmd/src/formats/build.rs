// Written by xtmd-schemagen from schemas/build.schema.json. Don't edit: change the schema, then run
// `cargo run -p xtmd-schemagen`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};

/// The format's version.
pub const VERSION: i64 = 1;

/// What an engine built.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Built {
    pub files: Vec<File>,
    /// The format's version.
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub ir: crate::formats::ir::Module,
    /// Its place among what is built, with no ending: `sprint`, `docs/sprint`.
    pub name: String,
}

/// A file to write: its text, or its bytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
    /// Where, under the folder built into: `sprint.html`, `assets/page.js`. Parts between slashes, none of them `.` or `..`.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// What is built, all of it at once, so an engine can share between documents what is the same in them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub documents: Vec<Document>,
    /// The format's version.
    pub version: i64,
}
