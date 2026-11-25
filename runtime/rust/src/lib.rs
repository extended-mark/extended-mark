#![doc = include_str!("../README.md")]

pub mod host;
pub mod layout;
mod markdown;
mod page;

pub use layout::{Pt, Region, Size};
pub use xtmd::eval::{Binding, FailKind, Failure, Machine, Num, Value};
pub use xtmd::formats::check::{PageTheme, PageWidth};
pub use xtmd::formats::{actions, blocks, ir, state, told, tree};
pub use page::{failure_code, Broken, Call, CallCall, Drawn, DrawnNode, Env, Handle, Outcome, Page, PageData, PageOptions, PageValues, Supplies, ValueFn, Write, WriteTo, MAX_PASSES};
