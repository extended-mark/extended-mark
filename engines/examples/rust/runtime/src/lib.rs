#![doc = include_str!("../README.md")]

pub mod host;
pub mod layout;
mod markdown;
mod page;

pub use layout::{Measure, Pt, Region, Size, MAX_LAYOUT_PASSES};
pub use xtmd_eval::{Binding, FailKind, Failure, Machine, Num, Value};
pub use xtmd_ir::check::{Call, PageTheme, PageWidth};
pub use xtmd_ir::{ir, tree};
pub use page::{failure_code, Broken, Drawn, DrawnNode, Env, Handle, LayoutWrite, Outcome, Page, PageData, PageOptions, PageValues, Shaded, Supplies, ValueFn, WriteTo};
