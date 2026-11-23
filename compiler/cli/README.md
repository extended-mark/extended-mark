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
xtmd build page.xtmd          # the IR a runtime draws
xtmd md page.xtmd             # the page as Markdown
xtmd migrate docs/            # documents brought up to the packs they use
xtmd lsp                      # the language server, for an editor
xtmd help <command>           # each command's flags and an example
```

The language, its spec and the engines that draw a page are at
[extended-mark/extended-mark](https://github.com/extended-mark/extended-mark).
