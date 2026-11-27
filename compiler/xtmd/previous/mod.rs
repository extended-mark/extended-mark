//! 0.3.2's front end, copied from tag v0.3.2 by tools/release/previous.mjs: the release before this
//! one, which reads a tree written for it (src/migrate/readers/previous.rs). Not edited by hand.

/// The release it is.
pub const RELEASE: &str = "0.3.2";
/// The tag it was copied from.
pub const TAG: &str = "v0.3.2";

pub mod core_files;
pub mod syntax;
pub mod ir;
pub mod eval;
pub mod check;
pub mod lower;
pub mod markdown;
