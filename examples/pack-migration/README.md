# A pack's new version, and the documents that use it

`kit@2` renames a widget and an input, changes a default, turns a text into a slot, drops an input and
renames a case. Each would break a document written for `kit@1`, so `kit@2` ships a migration, and
`xtmd migrate` moves the documents.

| File | What it is |
|---|---|
| `kit/kit.xtmdpack` | The pack, as `kit@2` |
| `kit/surfaces/kit@1.xtmdpack` | What `kit@1` provided, written by `xtmd surface` when it was released. Nobody edits it |
| `kit/surfaces/kit@2.xtmdpack` | What `kit@2` provides: a later `kit@2` that breaks it is a compile error |
| `kit/migrations/kit@2.xtmdpack` | How each use of `kit@1` becomes a use of `kit@2` |
| `docs/release.xtmd` | A document written for `kit@1`, as `xtmd.toml`'s `[uses]` says |
| `moved/release.xtmd` | The same, after `xtmd migrate docs` |

In a copy of this folder:

```sh
xtmd check kit                # the pack, its migration, and the example inside it
xtmd surface kit --check      # the surface is up to date, and nothing released is broken
xtmd migrate docs --dry-run   # each change, with the statement that makes it and why
xtmd migrate docs --yes       # written: docs/release.xtmd, and kit = 2 in xtmd.toml
```

`compiler/cli/tests/pack_migration.rs` runs these in CI. The rules are in
[page 6](../../docs/specs/06-hosts-packs-implementers.md), "Versions and migrations".
