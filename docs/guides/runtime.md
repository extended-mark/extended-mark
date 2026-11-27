# Writing a runtime

A runtime runs a page: it takes the IR and gives a host the drawn tree, which a renderer draws. This guide is
the order to build one in. The contract is [page 6](../specs/06-hosts-packs-implementers.md), and evaluation is
[page 2](../specs/02-values-and-types.md). Two are here to read: [TypeScript](../../runtime/typescript)
and [Rust](../../runtime/rust).

Two things you may want instead:

- **To draw differently**, write a [host](host.md): new widgets, another toolkit or framework and another look
  need no runtime of yours, only the tree.
- **To build pages into files**, ahead of time, write an [engine](engine.md).

## What you never write

A parser, a checker, or anything of the language's. There is one compiler, and the IR it writes has every
default filled in, every name resolved, every call chosen and no Markdown left. If your runtime has to work out
what something means, the IR should have said it: tell us.

## 1. Read the IR

[schemas/ir.schema.json](../../schemas/ir.schema.json) is all of it, and generates types
(`cargo run -p xtmd-schemagen` writes ours, Rust and TypeScript, from it). Its parts:

| Part | What it is |
|---|---|
| `version` | The IR's version, a whole number. Yours reads one, and refuses another by name: "this page is IR 3, this runtime reads IR 4: run xtmd migrate" |
| `needs` | What the page takes from others. Refuse a page that needs a core fn you don't have; tell the host about the rest (`lacking`) |
| `declarations` | Records, enums, tokens, fns, externs, `$state`, constants and what it exposes |
| `templates` | Extended Mark's own widgets, as nodes; `doc` is the page |
| `handlers` | What events run |
| `reads`, `writes` | What each place of a view reads that may change, and what each handler and fn may write. A place with none never changes |
| `whole` | An operator proved to work on whole numbers any platform's own hold |

Read it once. It is static: a page's values are yours, beside it.

## 2. Evaluate

Values, exact decimal numbers, statements, fns, and the natives on numbers, text and lists. A literal, a
`try`, a `match` and a loop each do what page 2 says and nothing else. Count steps, items and depth
(page 6, Limits). Every native is in `needs.core` when a page uses it, so you can start with a few.

The runtimes' own tests hold hundreds of small pages, each with what it observes: `runtime/typescript/test/cases`
and, the same ones in Rust, `runtime/rust/tests`.

## 3. Draw the tree

Walk `doc`: an extern call is a node, a widget's template is walked in its place with its inputs and its own
variables, a loop's rows are told apart by their items, a slot's content is worked out where it is placed. The
result is [the drawn tree](../../schemas/tree.schema.json):

- **Ids**: a node's own on the page, and the same for as long as it is there. A row's follow its item.
- **Inputs**: all of them, worked out, defaults in, in the IR's form. A fn is a number that moves when what it
  reads does.
- **Failures** are contained: the widget's node says `failed`, and the rest is drawn.
- A widget the host doesn't draw is its Markdown, as nodes.

Two trees to compare with, byte for byte: `schemas/examples/counter.tree.json` and
`apps/example-host/test/page.tree.json`.

## 4. Change it

Events, writes and the host's own changes run one at a time, each a change that is done whole or undone:
contracts, then `require`s. Then draw again, and work out only what reads what changed: keep what each
expression, loop, branch and rule read, with a version for each thing that can be read. Say what changed, as
page 6 does. Count the work in a test: one label's change on a page of 80 rows costs what it does on one of 8.

## 5. The rest

| | |
|---|---|
| Markdown | The page's, with the values it has now: each extern's `markdown` nodes, each template's own where it isn't its view |
| Planes | Lay spots out from the sizes the host measured, in passes, at most eight; one that never settles is a failure |
| Data | `state`, and `own` by place, out and back in |
| Levels | `levels` say what the page needs (`core.draw`, `core.live`, `core.gfx`). A call needing one you don't have shows its Markdown |

## A static document, and a live one

A **static** document is given compiled IR and carries no compiler. A **live** one is given text: it compiles with
the one compiler, built in, and does with the IR exactly what its static twin does. In a browser the compiler is
`@extended-mark/wasm`, and in a program the Rust crate `xtmd` (its module `live`). A live document keeps the last page that
compiled while a draft has errors.

## Checking it

How well a runtime keeps to the spec is its implementer's to check; xtmd doesn't test them. What helps:

- the schemas, on everything you read and give;
- the two trees above, and the runtimes' tests as a list of what to try;
- `xtmd md`, for the Markdown a page must write with its starting values;
- running one page in yours and in another, with the same events, and comparing the trees.
