#![doc = include_str!("../README.md")]

pub mod host;
pub mod layout;
mod markdown;
mod page;

pub use layout::{Measure, Pt, Region, Size, MAX_LAYOUT_PASSES};
pub use xtmd_eval::{Binding, FailKind, Failure, Machine, Num, Value};
pub use xtmd_ir::check::{PageTheme, PageWidth};
pub use xtmd_ir::{actions, blocks, ir, state, told, tree};
pub use page::{failure_code, Broken, Call, CallCall, Drawn, DrawnNode, Env, Handle, LayoutWrite, Outcome, Page, PageData, PageOptions, PageValues, Shaded, Supplies, ValueFn, WriteTo};
