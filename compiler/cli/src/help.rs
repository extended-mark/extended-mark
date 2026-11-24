//! What `xtmd`, `xtmd --help` and `xtmd <command> --help` print: the commands, and each one's flags,
//! what it prints where, its exit codes and an example.

/// `xtmd` with no command, `xtmd --help` and `xtmd help`.
pub(crate) const TOP: &str = "xtmd, the Extended Mark compiler.

usage: xtmd <command> [arguments] [flags]
  xtmd fmt <file|folder>…        rewrite each file in the canonical layout, in place (the formatter)
  xtmd check <file|folder>…      report every error and warning, then a line for each file
  xtmd parse <file>              print a file's syntax tree, as JSON
  xtmd build <file|folder>…      write each document's IR, for an engine (--rir: the resolved IR, for tools)
  xtmd md <file|folder>…         write each document's Markdown, as it reads when it first loads
  xtmd build … --engine e        …or the files an engine makes of them: HTML, Rust, whatever it is for
  xtmd surface <pack>            the release step: record what this version of the pack provides
  xtmd migrate [<file|folder>…]  bring documents and packs up to the packs they are built with
  xtmd lsp                       the language server, for an editor: the protocol over stdin and stdout
  xtmd help <command>, or xtmd <command> --help: that command's flags, exit codes and an example
  xtmd --version: this xtmd's release

Every command takes:
  --format json  JSON instead of text. A command that writes a document (build, md, parse) writes
                 it as JSON, with its diagnostics as JSON lines on stderr. One that reports (check,
                 fmt, surface, migrate) prints JSON lines: one object a line, each with
                 its \"type\", and a \"summary\" last.
  --out <file>   what it prints goes to the file, its folder made if need be (build, md and
                 surface write their document there); a run that stops at a usage error leaves it alone.
  --help         its own usage.
And check, build, md, surface and migrate take:
  --pack a,b     the packs: folders and .xtmdpack files.
  --config f     an xtmd.toml; left out, xtmd looks for one in the file's folder and the folders above.
                 It says packs = [\"kit\", \"extra.xtmdpack\"] (as --pack does), xtmd = \"0.4.0\" (the release
                 of xtmd the tree is written for), under [uses], kit = 2 (each pack's version) and, under
                 [engines], web = [\"node\", \"engine.mjs\"] (what xtmd build --engine web runs).

Exit codes: 0 done, nothing wrong; 1 something is wrong (an error, a file that would change, a
check that failed); 2 a usage error, or a file that can't be read.
A flag a command doesn't know is warned about and ignored, but a run that would write files then
writes nothing.";

const FMT: &str = "usage: xtmd fmt <file|folder>… [--check | --diff | --stdout] [--out file] [--format json]
       xtmd fmt --stdin <file> [--check | --diff] [--format json]
  Rewrites each file in the canonical layout, in place, and says what it did: the standard
  formatter. A folder means every .xtmdpack and .xtmd file under it.
  --check        writes nothing: names each file that isn't in the canonical layout, and fails
  --diff         writes nothing: prints each change as a unified diff (with --check, it fails too)
  --stdout       writes nothing: prints one file in the canonical layout
  --stdin        formats the text on stdin instead, for an editor whose text isn't saved: the file
                 named says what it is (.xtmd or .xtmdpack) and is never read or written; the text
                 goes to stdout, or with --check and --diff, whether and how it would change
  --out f        alone: writes one file's canonical text to f, and leaves the file as it was;
                 with --check or --diff, what they print goes to f
  --format json  a JSON line for each file ({\"type\":\"file\",\"result\":\"canonical\"|\"formatted\"|
                 \"would change\"|…}), and a summary last
  The verdicts go to stderr, so --stdout and --diff print only the text.
  exit: 0 done, or nothing to change; 1 a file would change (--check) or doesn't read;
        2 a usage error, or a file that can't be read
  e.g.  xtmd fmt page.xtmd
        xtmd fmt --check src/
        xtmd fmt page.xtmd --out tidy.xtmd
        xtmd fmt --stdin page.xtmd < draft.xtmd";

const CHECK: &str = "usage: xtmd check <file|folder>… [--pack a,b] [--config xtmd.toml] [--out file] [--format json]
  Reports every error and warning in each file, then a line for each file and, for several, a
  total. A document's line names the levels it needs to run (core.draw, core.live, core.gfx).
  A folder means every Extended Mark file under it. A pack's file is checked with the rest of its pack,
  against the surfaces it has released (surfaces/name@N.xtmdpack), and its migrations' examples are run.
  --pack a,b     the packs the files use (or packs = [...] in xtmd.toml)
  --out f        the report goes to f instead of stderr
  --format json  JSON lines on stdout: one for each diagnostic ({\"type\":\"diagnostic\",\"code\",
                 \"severity\",\"line\",\"column\",\"message\",\"fix\"}, and \"edits\" where the fix is
                 exactly one: [{\"line\",\"column\",\"end\",\"text\"}], what an editor writes), one for each
                 file ({\"type\":\"file\",\"result\":\"clean\"|\"warnings\"|\"errors\"|\"not read\",\"levels\"}),
                 and a summary last
  exit: 0 no errors (warnings are fine); 1 errors; 2 a usage error, or a file that can't be read
  e.g.  xtmd check docs/
        xtmd check --pack graphs flow.xtmd --format json --out problems.jsonl";

const PARSE: &str = "usage: xtmd parse <file> [--out tree.json] [--format json]
  Prints one file's syntax tree, as JSON, to stdout (or --out); its syntax errors go to stderr,
  as JSON lines with --format json.
  exit: 0 it parses; 1 it has syntax errors; 2 a usage error, or a file that can't be read
  e.g.  xtmd parse page.xtmd --out tree.json";

const LSP: &str = "usage: xtmd lsp
  The language server, for an editor: it speaks the Language Server Protocol over stdin and
  stdout, and an editor starts it. It gives each open document's errors as it is edited, hover,
  go to definition, references, the outline, completion and formatting, worked out from the
  same checker xtmd check runs, with core's own declarations. A document's packs are the ones its
  xtmd.toml names, read from disk, unless the editor gives them (xtmd/packs).
  It takes no flags but --help. Editors: ide/vscode, and ide/lsp's README for the rest.
  exit: 0 after the editor's shutdown and exit; 1 when the input ends first
  e.g.  xtmd lsp";

const BUILD: &str = "usage: xtmd build <files|folders>… [--pack a,b] [--config xtmd.toml] [--rir [--core]] [--pretty] [--out out] [--format json]
       xtmd build <files|folders>… [--pack a,b] [--config xtmd.toml] [--format json] --out dir --engine <name|path> [the engine's own arguments]
  Writes a document's IR, what a runtime draws: the document, and only what it uses of core and
  its packs. The IR's top-level \"levels\" are what the page needs.
  --rir          the resolved IR instead: every name and type resolved and nothing lowered, for
                 editor tools (schemas/rir.schema.json); its \"levels\" are the document's too
  --pretty       JSON over several lines, for a person to read; without it, as small as it goes
  --core         with --rir: core's own files first, resolved too, for an editor that completes
                 core's widgets and goes to their declarations
  --out f        one file: writes it to f, its folder made if need be; without it, it goes to stdout
  --out dir      several files, or a folder: each document as <dir>/<its place>/<name>.ir.json
                 (.rir.json with --rir), core compiled once; one that doesn't compile is left out
  --format json  its errors as JSON lines on stderr, and what was written on stdout
  --engine name  writes the files an engine makes of the documents instead, into --out's folder. The
                 engine is one of xtmd.toml's [engines], a command run in xtmd.toml's folder:
                   [engines]
                   web = [\"node\", \"engine.mjs\"]
  --engine path  a program, by a path with a / in it, run in the current folder
                 It reads the documents' IR on its standard input and gives the files on its standard
                 output (schemas/build.schema.json), its errors on standard error. Everything after its
                 name goes to it, untouched, so xtmd's own flags go before --engine
  exit: 0 written; 1 one doesn't compile, or what the engine gave can't be written; 2 a usage error, a
        file that can't be read, or an engine that can't be run; the engine's own when that isn't 0
  e.g.  xtmd build page.xtmd --out dist/page.ir.json
        xtmd build docs --out dist
        xtmd build docs --out dist --engine web --theme plain";

const MD: &str = "usage: xtmd md <files|folders>… [--pack a,b] [--config xtmd.toml] [--out out] [--format json]
  Writes a document's Markdown, as the page reads when it first loads: each value at its start,
  each widget as its pack writes it in Markdown (a button as its label; a widget whose pack writes
  no Markdown is left out).
  --out f        one file: writes it to f, its folder made if need be; without it, it goes to stdout
  --out dir      several files, or a folder: each document as <dir>/<its place>/<name>.md
                 (.blocks.json with --format json), core compiled once
  --format json  the same page as a JSON list of blocks, each named by its kind ({\"heading\": …},
                 {\"paragraph\": …}), as the checks read it
  exit: 0 written; 1 one doesn't compile; 2 a usage error, or a file that can't be read
  e.g.  xtmd md page.xtmd --out README.md
        xtmd md docs --out markdown";

const SURFACE: &str = "usage: xtmd surface <pack folder|file> [--pack a,b] [--check | --diff | --stdout] [--out file] [--force] [--format json]
  The release step. Writes what this version of the pack provides, its surface, to
  <pack folder>/surfaces/<name>@<version>.xtmdpack (the version is the pack block's: pack kit@2).
  Commit it: a later version is held to it, and its migration starts from it.
  --check        writes nothing: fails if the surface is missing or out of date, or the pack
                 breaks one it has released
  --diff         writes nothing: shows how the surface would change; --stdout prints it
  --out f        writes the surface to f instead of into the pack folder
  --force        replaces a recorded surface that differs (only for a version not yet released)
  --pack a,b     the packs this pack uses
  exit: 0 written, or up to date; 1 missing, out of date, broken or doesn't compile; 2 a usage error
  e.g.  xtmd surface kit
        xtmd surface kit --check";

/// A command's own usage, or none for a name that isn't one.
pub(crate) fn of(cmd: &str) -> Option<&'static str> {
    Some(match cmd {
        "fmt" => FMT,
        "check" => CHECK,
        "parse" => PARSE,
        "build" => BUILD,
        "md" => MD,
        "surface" => SURFACE,
        "migrate" => crate::migrate::USAGE,
        "lsp" => LSP,
        _ => return None,
    })
}
