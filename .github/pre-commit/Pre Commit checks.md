# Pre Commit checks

What to look at before a commit, and what each release owes the one before it. An agent goes through
this before committing; a person before a release.

## Every commit

`node .github/pre-commit/check.mjs` makes the checks a program can make, and fails naming each one that
fails. CI runs it too.

- The notes for agents still name what is there (`node tools/llm/check.mjs`).
- Every version is VERSION's (`node tools/release/bump.mjs --check`).
- core is formatted (`xtmd fmt --check core`, with the xtmd of `compiler/target/release`, or
  `XTMD_BINARY`).
- The table below is the code's, and nothing in it is past its "Remove in".

What is yours, since no program can tell:

- The tests of what changed, and the cargo suite when the compiler changed (`.knowledge/testing/`).
- `xtmd fmt --check` over the files you touched.
- A change to the language, the IR or a pack's surface comes with how what exists moves to it.

## Moved commands and flags

A command or flag a release moves still answers in that release, only to say where it went (exit 2,
nothing run). The next release removes it completely, its message too. The moved commands are `MOVED` in
`compiler/xtmd/src/cli/mod.rs`, the moved flags `MOVED_FLAGS` in `compiler/xtmd/src/cli/flags.rs`.

The table is written from them (`node .github/pre-commit/check.mjs --write`), so a move is made in the
code and the table follows. Once VERSION is raised to a row's "Remove in", the check fails until its
entry, its message and its tests are taken out, and that release's notes say so.

<!-- the table is written by: node .github/pre-commit/check.mjs --write -->
| Moved | In | Where it went | Remove in |
|---|---|---|---|
| `xtmd build` | 0.4.0 | The files an engine makes: `xtmd export <files> --out dir --engine <name>`. The IR a runtime runs: `xtmd compile <files>` | 0.5.0 |
| `xtmd export --rir` | 0.4.0 | xtmd compile writes the resolved IR: `xtmd compile page.xtmd --rir` | 0.5.0 |
| `xtmd export --core` | 0.4.0 | xtmd compile writes the resolved IR, core's files in it: `xtmd compile page.xtmd --rir --core` | 0.5.0 |
| `xtmd export --pretty` | 0.4.0 | xtmd compile writes the IR: `xtmd compile page.xtmd --pretty` | 0.5.0 |
<!-- the table ends -->
