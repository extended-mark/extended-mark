# Moving a tree to a new release

A release of Extended Mark may change the language. A release moves what was written for the release
right before it, and nothing older: a tree further back is moved by each release in between, in turn,
patch releases too. It moves it by what each release understands of it, never by searching its text,
and every file of the tree moves or none does. Spec page 6 ("A release moves what was written for the
release right before it") says what it means; this guide says how to run it, and how a release that
changes something makes its step.

A migration moves what is plain. It never promises the result: what it can't move it leaves for you,
with a TODO at its place, and says so.

## Running it

The tree's `xtmd.toml` says the release it is written for (`xtmd = "0.4.0"`). With the `xtmd` of the
release after that one:

```sh
xtmd migrate docs --dry-run   # what would change, file by file
xtmd migrate docs --diff      # the change as a diff
xtmd migrate docs --yes       # written: every file, or none
```

What happens:

1. The release before reads each file, documents and packs' files, with its own front end, which this
   `xtmd` carries: each file's resolved IR, and each document's Markdown.
2. This `xtmd` moves the resolved IR one step, to its own.
3. Every file is written again in `xtmd fmt`'s layout, its comments kept, and compiled back to its
   resolved IR, which must be what the step made.
4. Each document's Markdown is compared with what the release before wrote. A difference is a warning:
   the Markdown misses what a page does, and a step may change it on purpose.

It says what it read (`read as written for 0.4.0 (rir 2), moved to 0.5.0 (rir 3)`), each file that
changes, each TODO, each warning, each question, and why anything is refused. A refusal, or a question
with no answer, writes nothing. `--format json` gives a line for each, then a summary with `ready`,
`todos` and `warnings`.

**A tree further back.** `tools/migrate/chain.py docs --to 0.6.0` fetches the `xtmd` of each release
in between and runs each in turn, stopping at the first that leaves something for you, so that you
settle it in the release it belongs to. A tree whose `xtmd.toml` says no release is taken as written
for the release before, and you are told so.

**Questions.** A step asks what only you know, by an id. Answer them in a file:

```toml
# answers.toml
[steps]
"docs/shop.xtmd: first" = "keep"
```

```sh
xtmd migrate docs --answers answers.toml --yes
```

**TODOs.** What no step can move is left as it was, with `// TODO(xtmd migrate): why` above it. The
file is written and compiles; the TODO is yours to finish. A run that writes one exits with 3, so a
script can't miss it.

## Making a release that changes something

Each release carries the front end of the release before it, and one step from it:

1. **The front end.** At the release, the release tool copies the release before's front end into
   `compiler/xtmd/previous/`, replacing the one there: the repository always holds exactly one older
   front end, built into the command line only.
2. **The schema.** A change to what the resolved IR holds raises its version (`rir` in
   `schemas/rir.schema.json`). Keep the old schema: `schemas/rir-history/<old version>/`, and a copy in
   `compiler/xtmd/src/migrate/rir/`.
3. **The step**, in `compiler/xtmd/src/migrate/rir/`: a function over the JSON, from the release
   before's resolved IR to this one's, by names and types. A release whose files need moving has one
   even when the version stays. It is given `Asked`:
   - `asked.ask(id, question, options)` gives the answer, or keeps the question open, and then
     nothing is written;
   - `asked.todo(node, file, why)` leaves a TODO above a node.
4. **Test it** the way `compiler/xtmd/tests/migrate_pipeline.rs` tests a made-up release: a tree read
   by the release before, the step, the files printed and compiled back to what the step made.

A change to a command or a flag works the same way, one release at a time: the release that moves it
keeps the old name only to say where it went, and the next one removes it. The table in
`.github/pre-commit/Pre Commit checks.md` lists each one and when it goes, and its check fails once
`VERSION` reaches that release.
