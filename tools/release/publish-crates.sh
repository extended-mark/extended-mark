#!/bin/sh
# Publishes Extended Mark's crates to crates.io, each after the ones it depends on (cargo orders a workspace
# publish). A published crate holds only its own folder, so the repository's files a crate embeds are
# copied into it first: xtmd and xtmd-live embed all of core, xtmd-syntax core's entities, and
# xtmd-lsp checks/rules.json (their build scripts look in the crate before the repository); they are
# taken out after.
#
#   tools/release/publish-crates.sh [--dry-run]
#
# Publishing needs CARGO_REGISTRY_TOKEN in the environment; --dry-run needs nothing. The ten published:
# xtmd, xtmd-live, xtmd-lsp, xtmd-syntax, xtmd-check, xtmd-ir, xtmd-lower, xtmd-eval, xtmd-markdown and
# xtmd-migrate. The workspace's other members have `publish = false` and cargo leaves them out:
# xtmd-schemagen (a tool here), xtmd-wasm (xtmd.wasm, in @extended-mark/wasm and on the GitHub
# release) and the example targets in targets/examples (xtmd-runtime, xtmd-egui, xtmd-egui-live,
# xtmd-android, xtmd-android-live).
set -eu
repo=$(cd "$(dirname "$0")/../.." && pwd)
rust="$repo/compiler"
export PATH="$HOME/.cargo/bin:$PATH"
cleanup() { rm -rf "$rust/cli/core" "$rust/live/core" "$rust/syntax/core" "$repo/ide/lsp/checks"; }
trap cleanup EXIT
for c in cli live; do
  mkdir -p "$rust/$c/core"
  cp "$repo"/core/*.xtmdpack "$rust/$c/core/"
done
mkdir -p "$rust/syntax/core"
cp "$repo/core/entities.xtmdpack" "$rust/syntax/core/"
mkdir -p "$repo/ide/lsp/checks"
cp "$repo/checks/rules.json" "$repo/ide/lsp/checks/"
cd "$rust"
cargo publish --workspace --allow-dirty "$@"
