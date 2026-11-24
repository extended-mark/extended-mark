//! The formats that pass between modules, as Rust types: the IR every engine runs, the tree a host
//! draws, a page's state, what is done to it, what it tells, its Markdown blocks, what an engine builds; and the compiler's
//! cases.
//!
//! The modules are generated from the JSON Schemas in `schemas/`, which are the source:
//! change a schema, then run `cargo run -p xtmd-schemagen`.

pub mod actions;
pub mod blocks;
pub mod build;
pub mod check;
pub mod ir;
pub mod state;
pub mod told;
pub mod tree;

impl ir::Type {
    /// A type by its name, with nothing else: `core/number`, `doc/task`.
    pub fn named(name: &str) -> ir::Type {
        ir::Type { name: name.to_string(), args: Vec::new(), contract: None, params: Vec::new(), returns: None, fields: Vec::new(), interface: None }
    }

    /// A type with its type arguments: `core/list` of `doc/task`.
    pub fn of(name: &str, args: Vec<ir::Type>) -> ir::Type {
        ir::Type { args, ..ir::Type::named(name) }
    }
}

impl ir::Template {
    /// What it writes as Markdown: its markdown block, or its view.
    pub fn markdown(&self) -> &[ir::Node] {
        self.markdown.as_deref().unwrap_or(&self.view)
    }
}
