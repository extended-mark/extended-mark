# schemas

The contracts between the compiler and every engine, as JSON Schema (draft 2020-12). They are the source: the Rust and TypeScript types are generated from them.

| File | What it describes |
|---|---|
| `ir.schema.json` | IR 4: the one checked, lowered IR the `xtmd` compiler writes and every engine runs. Declarations, templates (widgets written in Extended Mark, lowered to nodes) and handlers, only what the page can run. No Markdown is left in it (docs/specs/06, The IR). |
| `rir.schema.json` | The resolved IR, `rir 1`: what the compiler's checker understood of a build, for tools, never run by an engine. Every name resolved to what it names, every value typed, nothing lowered: prose is still Markdown and a call still has its arguments by name. Every node keeps its span in its source and the comments that sit with it, so `xtmd migrate`, an editor or a linter can edit the source exactly or print it back. Its types are the IR's. `xtmd build --rir` writes it. |
| `tree.schema.json` | The host's side (docs/specs/06, The host's side): the drawn tree every engine gives, what changed in it, and the page's data. |
| `check.schema.json` | A compiler case (a source, its diagnostics and its Markdown), the Markdown blocks `xtmd md --format json` writes, and what a runtime says about a page. |
| `ir-history/<version>/` | An earlier IR's schema, kept as it was, and pages as the xtmd of its time wrote them, each with the Markdown it must keep or why it is refused: `xtmd migrate` is held to them by `cargo test -p xtmd-migrate`. |
| `examples/*.rir.json` | A resolved IR beside the source it was made from (`examples/counter.xtmd`), held to the schema, its spans inside that source. |
| `examples/*.ir.json` | Whole IR files: the counter is what `xtmd build --pretty` writes, and the sprint page is written by hand; held to the schema and to the Rust types by `cargo test -p xtmd-ir`. |

The IR follows five rules: one key per node where a value can be of several kinds; every part named; one key, one meaning (`get` for every read, `declare` for every declaration); linked parts together (an `if` holds all its branches); and Extended Mark's own words (`requires`, `default`, `$state`, operators by their symbol).

## The generated types

```sh
cd compiler
cargo run -p xtmd-schemagen            # writes the types
cargo run -p xtmd-schemagen -- --check # only says whether they are current
```

| Generated | From |
|---|---|
| `compiler/ir/src/ir.rs`, `check.rs`, `tree.rs` | the crate `xtmd-ir` |
| `engines/published/runtime/src/generated/ir.ts`, `check.ts`, `tree.ts` | `@extended-mark/runtime` |

`cargo test -p xtmd-ir` fails when a generated file is not current, when an example or a check case doesn't fit its schema, or when the Rust types don't read and write it back unchanged.
