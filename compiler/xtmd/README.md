# xtmd

Extended Mark's command: it checks, formats and builds Extended Mark documents (`.xtmd`) and packs
(`.xtmdpack`), writes their Markdown, migrates them between versions and runs the language server.

## Install

```sh
cargo install xtmd            # this crate, built from source
npm i -g extended-mark        # the same command, a prebuilt binary for this machine; or npx extended-mark …

# no Node or Rust: the binary from the latest release, checked against its SHA256SUMS
curl -fsSL https://raw.githubusercontent.com/extended-mark/extended-mark/main/install.sh | sh     # Linux, macOS
irm https://raw.githubusercontent.com/extended-mark/extended-mark/main/install.ps1 | iex          # Windows (PowerShell)
```

The [releases](https://github.com/extended-mark/extended-mark/releases) have the binaries too.

## Use

```sh
xtmd check docs/              # every error and warning
xtmd fmt docs/                # the canonical layout, in place
xtmd compile page.xtmd          # the IR a page runs from
xtmd compile docs --out dist    # every document's, core compiled once
xtmd md page.xtmd             # the page as Markdown
xtmd build docs --out dist --engine ./web   # the files an engine makes: IR in, files out
xtmd migrate docs/            # documents brought up to the packs they use
xtmd lsp                      # the language server, for an editor
xtmd help <command>           # each command's flags and an example
```

## As a library

```rust
use xtmd::live::Compiler;

let compiler = Compiler::new(); // compiles core, the slow part
let built = compiler.build("page.xtmd", "# Hello\n", &[]);
assert!(built.ir.is_some());
```

`xtmd::live` is what a live document embeds: `Live` follows a text as it is edited, and keeps the last IR that
compiled while a draft has errors. A runtime, or an engine that is given IR, takes the crate with
`default-features = false`: the formats' types, the evaluator and the Markdown writer, with no compiler in it.

The language, its spec and what runs, draws and builds a page are at
[extended-mark/extended-mark](https://github.com/extended-mark/extended-mark).
