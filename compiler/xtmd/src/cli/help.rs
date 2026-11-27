//! What `xtmd`, `xtmd --help` and `xtmd <command> --help` print: the commands, and each one's flags,
//! what it prints where, its exit codes and an example.

/// `xtmd` with no command, `xtmd --help` and `xtmd help`.
pub(crate) const TOP: &str = "xtmd, the Extended Mark compiler.

usage: xtmd <command> [arguments] [flags]
  xtmd fmt <file|folder>…        rewrite each file in the canonical layout, in place (the formatter)
  xtmd check <file|folder>…      report every error and warning, then a line for each file
  xtmd parse <file>              print a file's syntax tree, as JSON
  xtmd compile <file|folder>…    write each document's IR, for a runtime (--rir: the resolved IR, for tools)
  xtmd decompile <file.rir.json> write a resolved IR's files back as source, in the canonical layout
  xtmd md <file|folder>…         write each document's Markdown, as it reads when it first loads
  xtmd build … --engine e        the files an engine makes of the documents: HTML, Rust, whatever it is for
  xtmd surface <pack>            the release step: record what this version of the pack provides
  xtmd migrate [<file|folder>…]  bring documents and packs up to the packs they are built with
  xtmd lsp                       the language server, for an editor: the protocol over stdin and stdout
  xtmd help <command>, or xtmd <command> --help: that command's flags, exit codes and an example
  xtmd --version: this xtmd's release

Every command takes:
  --format json  JSON instead of text. A command that writes a document (compile, md, parse) writes
                 it as JSON, with its diagnostics as JSON lines on stderr. One that reports (check,
                 fmt, surface, migrate) prints JSON lines: one object a line, each with
                 its \"type\", and a \"summary\" last.
  --out <file>   what it prints goes to the file, its folder made if need be (compile, build, md and
                 surface write their document there); a run that stops at a usage error leaves it alone.
  --help         its own usage.
And check, compile, build, md, surface and migrate take:
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
  It takes no flags but --help. Editors: ide/README.md.
  exit: 0 after the editor's shutdown and exit; 1 when the input ends first
  e.g.  xtmd lsp";

const COMPILE: &str = "usage: xtmd compile <files|folders>… [--pack a,b] [--config xtmd.toml] [--rir [--core]] [--pretty] [--out out] [--format json]
  Writes a document's IR, what a runtime draws: the document, and only what it uses of core and
  its packs. The IR's top-level \"levels\" are what the page needs.
  --rir          the resolved IR instead: every name and type resolved, every comment kept and
                 nothing lowered, for tools (schemas/rir.schema.json); its \"levels\" are the document's too
  --pretty       JSON over several lines, for a person to read; without it, as small as it goes
  --core         with --rir: core's own files first, resolved too, for an editor that completes
                 core's widgets and goes to their declarations
  --out f        one file: writes it to f, its folder made if need be; without it, it goes to stdout
  --out dir      several files, or a folder: each document as <dir>/<its place>/<name>.ir.json
                 (.rir.json with --rir), core compiled once; one that doesn't compile is left out
  --format json  its errors as JSON lines on stderr, and what was written on stdout
  exit: 0 written; 1 one doesn't compile; 2 a usage error, or a file that can't be read
  e.g.  xtmd compile page.xtmd --out dist/page.ir.json
        xtmd compile docs --out dist
        xtmd compile page.xtmd --rir --pretty";

const DECOMPILE: &str = "usage: xtmd decompile <file.rir.json> [--out dir | --stdout | --diff | --check] [--format json]
  The opposite of xtmd compile --rir: each file of a resolved IR written back as source, every name
  as its file knows it and every comment in its place, in xtmd fmt's layout. It reads nothing of the
  source the resolved IR was made from.
  --out dir      writes each file under dir, at the path the resolved IR names; without it, at that path
  --stdout       prints the files instead, one after the other
  --diff         writes nothing: shows how each file there would change
  --check        writes nothing: exit 1 when a file there would change
  --format json  a JSON line for each file: written, same or would change
  exit: 0 written, or nothing would change; 1 a part it can't write back, or with --check a file that would
        change; 2 a usage error, or a file that can't be read
  e.g.  xtmd compile page.xtmd --rir --out page.rir.json && xtmd decompile page.rir.json --stdout
        xtmd decompile build.rir.json --out migrated";

const BUILD: &str = "usage: xtmd build <files|folders>… [--pack a,b] [--config xtmd.toml] [--format json] --out dir --engine <name|path> [the engine's own arguments]
  Writes the files an engine makes of the documents, into --out's folder: HTML, Rust, whatever the
  engine is for. Each document is compiled as xtmd compile does, and its IR given to the engine.
  Without --engine there is nothing to build: xtmd compile writes the IR.
  --engine name  one of xtmd.toml's [engines], a command run in xtmd.toml's folder:
                   [engines]
                   web = [\"node\", \"engine.mjs\"]
  --engine path  a program, by a path with a / in it, run in the current folder
                 It reads the documents' IR on its standard input and gives the files on its standard
                 output (schemas/build.schema.json), its errors on standard error. Everything after its
                 name goes to it, untouched, so xtmd's own flags go before --engine
  --format json  its errors as JSON lines on stderr, and what was written on stdout
  exit: 0 written; 1 one doesn't compile, or what the engine gave can't be written; 2 a usage error, a
        file that can't be read, or an engine that can't be run; the engine's own when that isn't 0
  e.g.  xtmd build docs --out dist --engine web --theme plain";

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
        "compile" => COMPILE,
        "decompile" => DECOMPILE,
        "build" => BUILD,
        "md" => MD,
        "surface" => SURFACE,
        "migrate" => crate::cli::migrate::USAGE,
        "lsp" => LSP,
        _ => return None,
    })
}
