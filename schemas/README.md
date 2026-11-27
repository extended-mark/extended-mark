# schemas

The contracts between the modules, as JSON Schema (draft 2020-12). They are the source: the Rust and TypeScript types are generated from them.

| File | What it describes |
|---|---|
| `ir.schema.json` | IR 4: the one checked, lowered IR the `xtmd` compiler writes and every runtime runs. Declarations, templates (widgets written in Extended Mark, lowered to nodes) and handlers, only what the page can run. No Markdown is left in it (docs/specs/06, The IR). |
| `rir.schema.json` | The resolved IR, `rir 2`: what the compiler's checker understood of a build, for tools, which no page runs from. Every name resolved to what it names, every value typed, nothing lowered: prose is still Markdown and a call still has its arguments by name. Every node keeps its span in its source and every comment: its own (`doc`, `leading`, `trailing`, `block_start`, `block_end`, `free`) and those that belong to no node, as comment items; a document keeps its front matter's text. So a migration, an editor or a linter can change a build and print it back: `xtmd decompile` does. Its types are the IR's. `xtmd compile --rir` writes it. |
| `tree.schema.json` | The drawn tree every runtime gives, and what changed in it (docs/specs/06, The host's side). |
| `state.schema.json` | A page's data: `$state` and widgets' own variables, as a host keeps them and gives them back. |
| `actions.schema.json` | What a reader, a widget or a host does to a page, each one change. |
| `told.schema.json` | What a page tells its host: the fns it called, what was refused or failed, its warnings. |
| `blocks.schema.json` | A page's Markdown as blocks, before it is text, as `xtmd md --format json` writes them. |
| `build.schema.json` | What `xtmd compile --engine` gives an engine, and the files it gives back. |
| `web.schema.json` | What a web widget set gives for a widget: plain elements, for any renderer or engine. |
| `check.schema.json` | A compiler case: a source, its diagnostics and its Markdown; and what a case's steps observe. |
| `rir-history/<version>/` | An earlier resolved IR's schema, kept as it was shipped: what a migration reads from the release that wrote it. |
| `ir-history/<version>/` | An earlier IR's schema, kept as it was, and pages as the xtmd of its time wrote them, each with the Markdown it must keep or why it is refused: `xtmd migrate` is held to them by `cargo test -p xtmd --test migrate_ir_history`. |
| `examples/*.rir.json` | A resolved IR beside the source it was made from (`examples/counter.xtmd`), held to the schema, its spans inside that source. |
| `examples/*.ir.json` | Whole IR files: the counter is what `xtmd compile --pretty` writes, and the sprint page is written by hand; held to the schema and to the Rust types by `cargo test -p xtmd --test formats_schemas`. |

Every one of these but the IR and the cases has `x-version`, its version, which the generated types hold as `VERSION`.

The IR follows five rules: one key per node where a value can be of several kinds; every part named; one key, one meaning (`get` for every read, `declare` for every declaration); linked parts together (an `if` holds all its branches); and Extended Mark's own words (`requires`, `default`, `$state`, operators by their symbol).

## The generated types

```sh
cd compiler
cargo run -p xtmd-schemagen            # writes the types
cargo run -p xtmd-schemagen -- --check # only says whether they are current
```

| Generated | From |
|---|---|
| `compiler/xtmd/src/formats/ir.rs`, `tree.rs`, `state.rs`, `actions.rs`, `told.rs`, `blocks.rs`, `build.rs`, `web.rs`, `check.rs` | the crate `xtmd`, its module `formats` |
| `runtime/typescript/src/generated/`, the same names | `@extended-mark/runtime` |

`cargo test -p xtmd --test formats_schemas` fails when a generated file is not current, when an example or a check case doesn't fit its schema, or when the Rust types don't read and write it back unchanged.
