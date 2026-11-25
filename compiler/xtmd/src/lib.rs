#![doc = include_str!("../README.md")]
//!
//! # The parts
//!
//! Text goes through them in this order, and each uses only the ones before it (`tests/order.rs`):
//!
//! | Module | What it does |
//! |---|---|
//! | [`formats`] | The formats' types, made from `schemas/`: the IR, the drawn tree, the rest |
//! | [`eval`] | Numbers, values, the native fns and the limits |
//! | [`markdown`] | Blocks to text, and a page's starting blocks |
//! | `syntax` | Text to a tree and back: the reader and `xtmd fmt` |
//! | `lower` | What a page can run, as the IR an engine runs |
//! | `check` | Names, types, rules; gives the IR |
//! | `live` | Text to IR in one call, core compiled once; a text followed as it is edited |
//! | `migrate` | `xtmd migrate`, on texts |
//! | `lsp` | The language server |
//! | `cli` | The command |
//!
//! All but the first three are the compiler, behind the feature `compiler` (on by default). A runtime takes
//! `xtmd = { version = "…", default-features = false }`, and has none of it.

pub mod eval;
pub mod formats;
pub mod markdown;

#[cfg(feature = "compiler")]
pub mod check;
#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "compiler")]
pub mod live;
#[cfg(feature = "compiler")]
pub mod lower;
#[cfg(feature = "compiler")]
pub mod lsp;
#[cfg(feature = "compiler")]
pub mod migrate;
#[cfg(feature = "compiler")]
pub mod syntax;
