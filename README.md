<picture>
  <source media="(prefers-color-scheme: dark)" srcset="brand/logo-dark.svg">
  <img src="brand/logo.svg" alt="Extended Mark" height="52">
</picture>

# Extended Mark 0.3

Extended Mark (called MKP, Markdown Markup, up to 0.2) is a statically typed language for widgets in Markdown documents. A document is its definitions, a line that holds only `@@@`, and Markdown with `@widget` calls in it. Extended Mark lowers a page onto extern widgets, and whoever supplies those widgets decides how the page looks.

## Install

`xtmd` is the command: it checks, formats and builds documents, writes their Markdown, migrates them
and runs the language server.

```sh
npm i -g extended-mark        # the xtmd command, a prebuilt binary for this machine; or npx extended-mark …
cargo install xtmd            # built from source

# no Node or Rust: the binary from the latest release, checked against its SHA256SUMS
curl -fsSL https://raw.githubusercontent.com/extended-mark/extended-mark/main/install.sh | sh     # Linux, macOS
irm https://raw.githubusercontent.com/extended-mark/extended-mark/main/install.ps1 | iex          # Windows (PowerShell)
```

Each [release](https://github.com/extended-mark/extended-mark/releases) also has the binaries to
download: `xtmd-<version>-<platform>.tar.gz` for `linux-x64`, `linux-arm64`, `darwin-x64`,
`darwin-arm64` and `win32-x64`. The one-line installers put `xtmd` in `~/.local/bin` (on Windows,
`%LOCALAPPDATA%\xtmd\bin`, added to your PATH); `XTMD_VERSION` picks a release and `XTMD_INSTALL_DIR`
another folder.

0.3 is a new version of the language: `xtmd migrate` moves a 0.2 document to it. MKP 0.2 is in the
branch [`legacy-0.2.x`](https://github.com/extended-mark/extended-mark/tree/legacy-0.2.x), with its releases.

## In this repository

- `docs/specs`: the language, below. Every rule in it is held to a case in `checks/`.
- `compiler/`: `xtmd`, the compiler, in Rust, and its npm packages (`extended-mark`, with the `xtmd` command, and `@extended-mark/wasm`).
- `engines/published`: the engines on npm, for the web, React, Vue, Svelte, Solid and Angular. Each comes in two kinds: run time, which draws an IR built ahead, and real time, which compiles in place.
- `engines/examples`: engines that show how one is made (egui, Android's Compose, a native runtime). They are never published.
- `ide/`: `xtmd lsp`, the language server, and the VS Code extension.
- `apps/playground`: the playground. `apps/example-host`: a host written from the spec alone. `brand/`: the logo and icons.

## The syntax

| Page | What it covers |
|---|---|
| [1 Overview](docs/specs/01-overview.md) | The rules, the words, the two kinds of file, and how a document is read |
| [2 Values and types](docs/specs/02-values-and-types.md) | Records, enums, constants and `mut`, optional values, numbers, contracts and proofs, and what a fn may change |
| [3 Widgets](docs/specs/03-widgets.md) | Declaring and calling widgets, slots, events, views, loops and where variables live |
| [4 Interfaces](docs/specs/04-interfaces.md) | Structural interfaces, the standard ones, literals for your own types, and generics |
| [5 Standard library](docs/specs/05-standard-library.md) | What is built in, and all of `core`: types, text, dates, maths, Markdown, tokens and widgets |
| [6 Hosts, packs and implementers](docs/specs/06-hosts-packs-implementers.md) | Packs and imports, tokens, talking to the host, levels, failures and limits |
| [7 Examples](docs/specs/07-examples.md) | Whole widgets and a whole document |
| [8 Open questions](docs/specs/08-open-questions.md) | What this version decided, and what was left for later |
| [9 Grammar](docs/specs/09-grammar.md) | The grammar of files, documents and prose |

## Guides

- [Writing a host](docs/guides/host.md): drawing a page with code of your own, from its tree.
- [Writing an engine](docs/guides/engine.md): running the IR on another platform, or rendering it to a file.

## Releasing a new version of a pack

A pack's version is a promise: what compiles against one release of `kit@1` compiles against every later one.
A change that could break a document makes `kit@2`, which ships a **migration**, checked by the compiler.

```sh
xtmd surface kit              # at each release: writes surfaces/kit@1.xtmdpack. Commit it
# … change the pack, write `pack kit@2`, and migrations/kit@2.xtmdpack:
#     migration kit@1 to kit@2 {
#         widget meter {
#             rename(to = gauge);
#             rename(input = colour, to = tone);
#         }
#     }
xtmd check kit                # the migration type-checks from kit@1's surface to kit@2, and leaves nothing out
xtmd surface kit              # kit@2's surface
xtmd migrate docs --dry-run   # whoever uses it: each change and why, then --yes
```

[examples/pack-migration](examples/pack-migration) is one worked through, with a document before and after.

`notes/` has what each release changed.
