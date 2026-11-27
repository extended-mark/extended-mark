# Moving a tree to a new release

A release of Extended Mark may change the language. What was written for the release before it moves
by what each release understands of it, never by searching its text, and every file of the tree moves
or none does. Spec page 6 ("A release that changes the language") says what it means; this guide says
how to run it, and how a release that changes something makes its step.

## Running it

You need the `xtmd` of the release the tree is written for, beside this one. Its `xtmd.toml` says
which (`xtmd = "0.4.0"`).

```sh
xtmd migrate docs --reader ~/xtmd-0.4.0/xtmd --dry-run   # what would change, file by file
xtmd migrate docs --reader ~/xtmd-0.4.0/xtmd --diff      # the change as a diff
xtmd migrate docs --reader ~/xtmd-0.4.0/xtmd --yes       # written: every file, or none
```

What happens:

1. The old `xtmd` reads each file, documents and packs' files: `xtmd compile --rir` gives its resolved
   IR and `xtmd md` each document's Markdown.
2. This `xtmd` brings the resolved IR to its own version, a version at a time.
3. Every file is written again in `xtmd fmt`'s layout, its comments kept, and compiled.
4. Each document's Markdown must be what the old release wrote. A migration changes how a page is
   written, never what it says.

It says what it read (`read by 0.4.0's xtmd as rir 2, moved to 0.5.0 (rir 3)`), each file that
changes, each TODO, each question, and why anything is refused. A refusal, or a question with no
answer, writes nothing. `--format json` gives a line for each, then a summary with `ready`.

**Questions.** A step asks what only you know, by an id. Answer them in a file:

```toml
# answers.toml
[steps]
"docs/shop.xtmd: first" = "keep"
```

```sh
xtmd migrate docs --reader ~/xtmd-0.4.0/xtmd --answers answers.toml --yes
```

**TODOs.** What no step can move is left as it was, with `// TODO(xtmd migrate): why` above it. The
file is written and compiles; the TODO is yours to finish.

A tree written for 0.3 can't be moved this way yet: 0.3's `xtmd` has no `compile --rir`, and its
reader is still to come. `xtmd migrate docs` without `--reader` moves it as before.

## Making a release that changes something

A change to what the resolved IR holds raises its version (`rir` in `schemas/rir.schema.json`), and
the release ships the step from the version before:

1. Keep the old schema: `schemas/rir-history/<old version>/rir.schema.json`, and a copy in
   `compiler/xtmd/src/migrate/rir/`. The test in `compiler/xtmd/tests/migrate_pipeline.rs` holds the
   copy of this version's to `schemas/`.
2. Write the step in `compiler/xtmd/src/migrate/rir/`: a function over the JSON, from the old
   version's to the new one's, by names and types. It is given `Asked`:
   - `asked.ask(id, question, options)` gives the answer, or keeps the question open, and then
     nothing is written;
   - `asked.todo(node, file, why)` leaves a TODO above a node.
3. Add it to `STEPS`, with the old schema. What it is given is checked against that schema, and what
   it gives against the new one, so a step that writes the wrong shape is refused, not written.
4. Test it the way `migrate_pipeline.rs` tests a made-up release: a tree read at the old version, the
   step, the files printed and compiled, the Markdown the same.

A change to a command or a flag works the same way, one release at a time: the release that moves it
keeps the old name only to say where it went, and the next one removes it. The table in
`.github/pre-commit/Pre Commit checks.md` lists each one and when it goes, and its check fails once
`VERSION` reaches that release.

A reader for a release whose `xtmd` can't give a resolved IR (0.3) is a `Reader`
(`compiler/xtmd/src/migrate/readers/`): anything that gives the tree's resolved IR and each document's
Markdown, as that release understood them.
