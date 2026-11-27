# Rulesets

`main.json` protects the default branch (`main`). GitHub doesn't read this folder by itself:
import it once under **Settings → Rules → Rulesets → New ruleset → Import a ruleset**, and import it
again after changing it.

- **No deleting or force-pushing** `main`.
- **Changes land through a pull request** (no approvals required).
- **Required checks, on a branch that is up to date with `main`:**
  - **From staging**: the pull request's head is this repository's `staging`, so only what `staging` has tested reaches `main` (`.github/workflows/notes.yml`).
  - **Release notes**: `VERSION` is raised (past the last release, while `main`'s has none) and `notes/<version>.md` is well formed (`tools/release/check-notes.mjs`, `.github/workflows/notes.yml`).
  - **Types, tests and conformance**: the CI (`.github/workflows/ci.yml`): Rust's tests, the npm packages' and the playground's.

  "Up to date" stops two pull requests from both claiming the same next version.

A merge to `main` then publishes `v<VERSION>` (`.github/workflows/release.yml`).
