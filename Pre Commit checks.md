# Pre Commit checks

What to look at before a commit, and what each release owes the one before it. An agent reads this
before committing; a person reads it before a release.

## Every commit

- `node tools/llm/check.mjs`: CLAUDE.md, `.index/` and `.knowledge/` still name what is there.
- The tests of what changed, and the cargo suite when the compiler changed
  (`.knowledge/testing/`).
- `xtmd fmt --check` over core and the files you touched.
- A change to the language, the IR or a pack's surface comes with how what exists moves to it.

## Moved commands and flags

A command or flag a release moves still answers in that release, only to say where it went
(exit 2, nothing run). The next release removes it completely, its message too. The moved commands
are in `MOVED` in `compiler/xtmd/src/cli/mod.rs`, the dropped flags in `ASKED_FOR` in
`compiler/xtmd/src/cli/flags.rs`.

| Moved | In | Where it went | Remove in |
|---|---|---|---|
| `xtmd build` | 0.4.0 | `xtmd export` runs an engine; `xtmd compile` writes the IR | 0.5.0 |
| `xtmd build --rir`, `--core`, `--pretty` | 0.4.0 | `xtmd compile --rir`, `--core`, `--pretty` | 0.5.0 |

Once the version is raised to the release in "Remove in", take out the entry, its message, its row
here and its tests, and say so in that release's notes.
