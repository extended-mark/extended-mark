# Releasing Extended Mark

Work goes into `staging` first, and `main` takes it only from there: a merge request into main
whose head isn't this repository's `staging` fails the **From staging** check, which main's ruleset
requires. A merge into main that raises `VERSION` starts the release: `.github/workflows/release.yml` builds
everything on Linux, tests it, tags the merge commit `v<VERSION>` and publishes it under that tag. The merge request raises the
version everywhere with `node tools/release/bump.mjs <version>`, which also starts `notes/<VERSION>.md` (notes/README.md); the **Release notes** check holds it to that, and
the **Types, tests and conformance** check (ci.yml) runs the tests, both required by main's ruleset
(.github/rulesets/). A merge that leaves `VERSION` as it was releases nothing, and the workflow can
be run by hand. Only `main` releases: run by hand on any other branch, the workflow does nothing,
and `staging` has the tests (ci.yml, on its pull requests and pushes) and nothing that publishes.

To hold a release, disable the Release workflow (Actions → Release → Disable workflow) before the
merge; enable it and run it by hand when the release should go out. While `main`'s `VERSION` has no
release, a merge request may keep it and add to `notes/<VERSION>.md`: the check holds it only above
the last release.

## Before a release: the checklist

What a release owes the one before it, gone through on the merge request that raises `VERSION`:

1. `node tools/release/bump.mjs <version>` raises it everywhere and starts `notes/<version>.md`.
2. `node .github/pre-commit/check.mjs` passes: the notes for agents, every version the same, core
   formatted, and the table of moved commands and flags. A row whose "Remove in" is this release fails
   it until its entry, its message and its tests are taken out, and the notes say so.
3. **The IR.** A change to what a runtime reads raises its version: the old schema kept in
   `schemas/ir-history/<old>/` and `compiler/xtmd/src/migrate/ir/`, the step in `migrate/ir/`, and the
   table in spec page 6 ("The IR").
4. **The resolved IR.** A change to what it holds raises its version: the old schema kept in
   `schemas/rir-history/<old>/` and `compiler/xtmd/src/migrate/rir/`, and the table in spec page 6
   (docs/guides/migration.md says how).
5. **The step and the front end.** This release's one step from the release before (`STEP`, in
   `compiler/xtmd/src/migrate/rir/`), with its tests, and the release before's front end in
   `compiler/xtmd/previous/`, copied from its tag (`node tools/release/previous.mjs <its tag>`). Every
   change to what a file means comes with how what exists moves to it: in the step, or in a pack's
   migration, core's included (`core/surfaces/`, `core/migrations/`), each in the notes. Every file the
   release before shipped is in `checks/migrate-<x.y>/`, which a test migrates, and `rir::READY` is on:
   the release dry run (`preflight.mjs`) and the pre-commit checks refuse a release without these.
6. **Read by the release before.** The repository's documents move from the last release:
   `xtmd migrate <folder> --dry-run`, on a copy whose xtmd.toml says the last release, is ready,
   nothing refused and no question open, for each folder of examples/ and apps/ (the playground with
   `--pack examples/graphs`). examples/pack-migration is refused, as it should be: its documents are
   written for kit@1, for `xtmd migrate` to move by the pack's migration.

What it does, in order:

1. **Binaries.** `xtmd` for Linux on x64 and arm64 (static, musl), macOS on Intel and Apple
   silicon, and Windows on x64, each cross-built with `cargo zigbuild` and put in its
   `@extended-mark/cli-<os>-<cpu>` package (`compiler/npm/extended-mark/scripts/copy-binary.mjs --target <triple>`).
2. **xtmd.wasm**, for `@extended-mark/wasm` and the live engines (`compiler/wasm/build.sh`).
3. **Tests:** the same checks every pull request runs (ci.yml): the Rust workspace, every npm
   package's and the playground's tests.
4. **Publishing.** The GitHub release `v<VERSION>`, titled and described by its note; then the npm
   packages (`publish-npm.mjs`), the VS Code extension and the crate (`publish-crates.sh`). Each
   registry is tried even when one before it failed, and a version a registry has already is left
   alone, so `Re-run failed jobs` finishes what's left.

## Tested once per tree

The checks (ci.yml) test a set of files once. A run that passes leaves a small artifact named after the
commit's tree hash, `tested-<tree>` (and `dryrun-<tree>` for the release dry run's packaging); a later run
on the same tree finds it and skips that work. So the merge into `staging`, the pull request into `main`,
the merge into `main` and the release's own test job, which test the files the pull request into `staging`
already tested, each take about a minute. The tree includes `.github/`, so a change to the workflows is
tested afresh, and only a run of this repository counts, never a fork's. The registry check of the release
dry run always runs, since a registry can change while the files don't. npm's downloads are cached
between runs (setup-node's npm cache, keyed on the lock files), as are Rust's builds and Gradle's.

## Before the merge: the release dry run

A release that fails on main can't be taken back: a version a registry has is its for good. So on every
pull request into main, and on `staging` after each merge, the **Release dry run** job (ci.yml) asks
whether the release would go through, and the required **Types, tests and conformance** check fails
when it wouldn't. `preflight.mjs` checks what only the registries know:

- npm: every unscoped name is on npm already. npm refuses a name too like another's (it refused
  `xtmd` for `xtend`, `xml` and others) and says so only when the name is first published, so a new
  unscoped package is published by hand once. `NPM_TOKEN` signs in, may write every package and
  doesn't run out within two days; under trusted publishing no package is new.
- crates.io: under trusted publishing the crate isn't new; its version must be the release's.
- Open VSX: the namespace exists, and `OVSX_PAT` may publish to it.
- each registry: whether it publishes, stages, passes over a version it has, or is skipped for want
  of a credential, in the job's summary.

Then each publish is dry-run: every npm package packed and checked (`publish-npm.mjs --stage
--dry-run`), the crate built as crates.io would (`publish-crates.sh --dry-run`), and the `.vsix`
packed.

## What it publishes

Exactly these. Nothing private (Android, egui, the native runtime, the playground) is published: CI builds and checks them, and no release puts them out.

- **The GitHub release** `v<VERSION>`: each platform's binary, `xtmd-<VERSION>-<platform>.tar.gz`
  for `linux-x64`, `linux-arm64`, `darwin-x64`, `darwin-arm64` and `win32-x64` (what `install.sh`
  and `install.ps1` fetch); `SHA256SUMS`, the SHA-256 of each file; `xtmd-<VERSION>.wasm`; and the
  VS Code extension, `xtmd-<VERSION>.vsix`.
- **npm**, 23 packages:
  - from compiler/npm: `extended-mark` (the `xtmd` command, its API and Vite plugin),
    `@extended-mark/wasm`, and the binary for each platform,
    `@extended-mark/cli-linux-x64`, `@extended-mark/cli-linux-arm64`, `@extended-mark/cli-darwin-x64`,
    `@extended-mark/cli-darwin-arm64` and `@extended-mark/cli-win32-x64`;
  - from runtime, widgets, renderers, engines and live: `@extended-mark/runtime`, `@extended-mark/widgets`, `@extended-mark/dom`, `@extended-mark/web`,
    `@extended-mark/live`,
    `@extended-mark/react`, `@extended-mark/react-live`, `@extended-mark/next`, `@extended-mark/vue`,
    `@extended-mark/vue-live`, `@extended-mark/svelte`, `@extended-mark/svelte-live`,
    `@extended-mark/solid`, `@extended-mark/solid-live`, `@extended-mark/angular` and
    `@extended-mark/angular-live`.
- **crates.io**, one crate: `xtmd`, the compiler as a library and the command. Its parts are modules, and it
  uses no other crate of ours. (Up to 0.3.2 there were ten; the other nine end there.)
- **The VS Code Marketplace and Open VSX**: the extension (ide/vscode), the same `.vsix`.

## What it needs

The npm packages are `extended-mark` (the `xtmd` command, its API and Vite plugin; npm refuses the
name `xtmd` itself, as too like `xtend` and others) and `@extended-mark/*`; the crate is `xtmd`. Each
registry is published to by trusted publishing, with the run's own OIDC token, once it is set up;
until then, and for a package's first release (a registry sets up trusted publishing only for a
package it already has), with a token. On npm the token only stages: the first release's versions
wait on npmjs.com until a maintainer approves them, and then each package gets its Trusted Publisher
and `NPM_TRUSTED_PUBLISHING` is set to `true`, so every release after it publishes by itself.

| Repository variable or secret | For | Without it |
|---|---|---|
| `NPM_TRUSTED_PUBLISHING` = `true` (variable) | npm: each package's Trusted Publisher on npmjs.com names this repository and `release.yml`; set once the first release is approved | the token below stages the versions instead |
| `NPM_TOKEN` (secret) | a granular access token with Read and write on **all packages** (not just the `@extended-mark` scope: `extended-mark` is unscoped) (`npm stage publish` needs no 2FA, so the token needs no bypass): the release stages each version (`publish-npm.mjs --stage`, npm 11.15.0 or later) and it waits in npmjs.com's **Staged Packages** until a maintainer approves it (up to 100 at once, or `npm stage approve <id>`); after the first release each package gets its Trusted Publisher and `NPM_TRUSTED_PUBLISHING` = `true` | with no trusted publishing either, the npm packages aren't published; the step says so |
| `CRATES_TRUSTED_PUBLISHING` = `true` (variable) | crates.io: each crate's Trusted Publishing names this repository and `release.yml` | the token below is used |
| `CARGO_REGISTRY_TOKEN` (secret) | a crates.io token with publish-new and publish-update for `xtmd*` | with no trusted publishing either, the crates aren't published; the step says so |
| `VSCE_TRUSTED_PUBLISHING` = `true` (variable) | the VS Code Marketplace: the publisher `extended-mark`'s Trusted Publishing policy names this repository and `release.yml` (Azure DevOps tokens for vsce end on 1 December 2026) | the extension isn't published to the VS Code Marketplace (it is still on the GitHub release, as a .vsix) |
| `OVSX_TRUSTED_PUBLISHING` = `true` (variable) | Open VSX, which VSCodium, Cursor and the like install from: the namespace `extended-mark`'s trusted publisher names this repository and `release.yml`, and `ovsx publish --trusted-publishing` takes the run's OIDC token | the token below is used |
| `OVSX_PAT` (secret) | only until Open VSX's trusted publishing is set up: an Open VSX token whose account owns the namespace `extended-mark` | with no trusted publishing either, the extension isn't published to Open VSX; the step says so |

Trusted publishing checks each package's `repository` against the repository the run is in, so
they are moved together when the repository moves.

The GitHub release needs only the workflow's own token (Settings → Actions → Workflow
permissions: read and write).

## The scripts

- `build-ts.mjs [--test | --test-only] [--only <folders> | --except <folders>] [--list]`: installs
  and builds every npm package in order, the playground last, each after what it uses (npm links a
  `file:` dependency, so it is built first), then tests them all, or those under `--only`'s folders,
  or all but `--except`'s; `--list` says which and does nothing. CI's npm job tests on four
  machines at once, its last part taking every package the others don't name.
- `check-notes.mjs [--base-version x.y.z] | --notes-for x.y.z`: the release-notes check, and a
  note's title and body for its release.
- `publish-npm.mjs --version x.y.z [--stage] [--dry-run]`: publishes each package after its
  dependencies, its `file:` dependencies named by the version for the publish; a platform package
  without its binary is left out, and a version already staged or published is passed over, so a run
  can be repeated. `--stage` stages each one instead (`npm stage publish`) and ends with how many it
  staged and where to approve them.
- `publish-crates.sh [--dry-run]`: copies core and checks/rules.json into the crate, then `cargo publish -p xtmd`,
  unless crates.io has its version already.
- `bump.mjs <version> | --check`: sets the release's version everywhere it is written (`VERSION`, every
  crate and its path dependencies, `Cargo.lock`'s own crates, every package.json of ours and its
  `@extended-mark` pins, the lock files' own entries, the Android example's Gradle versions, the schemas'
  examples, the runtime snapshots' release, and the docs that show it), and starts the version's note.
  `--check` names each place that doesn't say `VERSION`; the Release notes check and the release run it.
- `preflight.mjs`: the release dry run's checks of each registry (above); its tests are in `test/`
  (`node --test tools/release/test/*.test.mjs`).
- `packages.mjs`: the npm packages and their order, for both.

Every package and crate carries the release's version (0.4.0); `publish-npm.mjs` refuses one that
doesn't.
