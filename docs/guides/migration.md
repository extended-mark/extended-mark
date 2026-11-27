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
2. This `xtmd` moves the resolved IR one step, to its own: what the language changed first, then core's
   migration (core is a pack with versions, like any other), then each of the tree's packs' migrations,
   applied to what each use resolves to.
3. Every file is written again in `xtmd fmt`'s layout, its comments kept, and compiled back to its
   resolved IR, which must be what the step made.
4. Each document's Markdown is compared with what the release before wrote. A difference is a warning:
   the Markdown misses what a page does, and a step may change it on purpose.

It says what it read (`read as written for 0.4.0 (rir 2), moved to 0.5.0 (rir 3)`), each file that
changes, each TODO, each warning, each question, and why anything is refused. A refusal, or a question
with no answer, writes nothing. `--format json` gives a line for each, then a summary with `ready`,
`todos` and `warnings`.

**A tree further back.** `python3 tools/migrate/chain.py docs --to 0.6.0` fetches the `xtmd` of each
release in between, patch releases too, from its GitHub release (checked against the release's
`SHA256SUMS`), and runs `xtmd migrate docs --yes` with each in turn. It stops at the first run that
leaves something for you (exit 3), so that you settle it in the release it belongs to; run it again
afterwards and it goes on from there. What follows `--` is given to each run, such as
`-- --answers answers.toml`. A tree whose `xtmd.toml` says no release is taken as written for the
release before by `xtmd migrate`; chain.py asks you to say which.

**Questions.** A step asks what only you know, by an id. Answer them in a file:

```toml
# answers.toml
[steps]
"docs/shop.xtmd: first" = "keep"
```

```sh
xtmd migrate docs --answers answers.toml --yes
```

**TODOs.** What no step can move is left as it was, with `// TODO(xtmd migrate): why` above it (`@//`
in prose). The file is written even when it doesn't compile yet, and what it doesn't compile with is
said beside its TODOs; the TODO is yours to finish. A run that writes one exits with 3, so a script
can't miss it.

## From 0.3.2 to 0.4.0

`xtmd = "0.3.2"` in the tree's `xtmd.toml`, then 0.4.0's `xtmd migrate`. A tree of 0.3.2:

```xtmd
let intro: md = "# Hi";

widget card(title: text, warn: text) {
    slot body: view;
    view {
        @label(value = warn);
        @body;
    }
    markdown {
        \{title}
    }
}
@@@
@blocks(value = intro);

@card(title = "Deploys", warn = "late");
```

becomes:

```xtmd
let intro: rich_text = "# Hi";

widget card(title: text, warn_: text) {
    slot body: view;

    view {
        @label(value = warn_);
        @body;
    }
    markdown {
        \{title}
    }
}
@@@
\{intro}

@card(title = "Deploys", warn_ = "late") {
    @slot body {
    }
}
```

What it moves, each as page 1 to 6 now say:

- `md` is the built-in `rich_text`, and `@blocks(value = x)` places `x` where it stands: `\{x}` in prose,
  `@x;` in a view (a value with no name is given one first, `let held_2: rich_text = …;`).
- A slot a call leaves out is given an empty block where it may be empty; a plain block given to a
  widget with several slots fills its first, by name.
- A name that is a keyword now (`warn`) is `warn_`, everywhere it is read.
- A `let` at the top of a file that reads the page becomes a fn, and each read a call; a field the tree
  writes is made `mut`; an extern widget that said no Markdown says `markdown { }`.
- A pack's `uses core@1` is `uses core@2`.

What it leaves for you, with a TODO saying why: code that reads a `rich_text`'s parts (its segments,
blocks or marks) or compares two, a slot that must hold something and that a call left out, a field of
another pack's record the tree writes, a field of `$state` or an exposed fn that was renamed (a host
reads it by its name), and anything a pack's migration can't decide.

A pack the tree only uses (given with `--pack`) that 0.4.0 doesn't take as it is stops the run: migrate
the pack first, then what uses it.

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
   by the release before, the step, the files printed and compiled back to what the step made. And on
   every file the release before shipped: `checks/migrate-<x.y>/` holds them, and
   `compiler/xtmd/tests/migrate_032.rs` migrates each and holds the result to `expected.tsv`.
5. **core's version.** A release that changes what core provides raises core's version
   (`pack core@N` in `core/pack.xtmdpack`), keeps the older version's surface in `core/surfaces/`, and
   writes `core/migrations/core@N.xtmdpack`, which `xtmd check` holds to every difference, as any pack's.
6. **On.** `rir::READY` says the step is whole. The release check (`tools/release/preflight.mjs`, in the
   release dry run and the pre-commit checks) refuses a release without the release before's front
   end, its step on, and its corpus.

A change to a command or a flag works the same way, one release at a time: the release that moves it
keeps the old name only to say where it went, and the next one removes it. The table in
`.github/pre-commit/Pre Commit checks.md` lists each one and when it goes, and its check fails once
`VERSION` reaches that release.
