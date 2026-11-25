#!/bin/sh
# Publishes Extended Mark's crate, xtmd, to crates.io. A published crate holds only its own folder, so the repository's
# files it embeds, core and checks/rules.json, are copied into it first (its build script looks in the crate before the
# repository), and taken out after.
#
#   tools/release/publish-crates.sh [--dry-run]
#
# Publishing needs CARGO_REGISTRY_TOKEN in the environment; --dry-run needs nothing. A version crates.io has already is
# left alone, so a release can be run again after a failure. The workspace's other members have `publish = false`:
# xtmd-schemagen (a tool here), xtmd-wasm (xtmd.wasm, in @extended-mark/wasm and on the GitHub release) and the example
# parts are never published.
set -eu
repo=$(cd "$(dirname "$0")/../.." && pwd)
crate="$repo/compiler/xtmd"
export PATH="$HOME/.cargo/bin:$PATH"
trap 'rm -rf "$crate/core" "$crate/checks"' EXIT
mkdir -p "$crate/core" "$crate/checks"
cp "$repo"/core/*.xtmdpack "$crate/core/"
cp "$repo/checks/rules.json" "$crate/checks/"
cd "$repo/compiler"
version=$(cargo metadata --no-deps --format-version 1 | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>console.log(JSON.parse(s).packages.find(p=>p.name==="xtmd").version))')
if [ "$(curl -s -o /dev/null -w '%{http_code}' -A 'extended-mark release (https://github.com/extended-mark/extended-mark)' "https://crates.io/api/v1/crates/xtmd/$version")" = 200 ]; then
  echo "xtmd $version is on crates.io already: nothing to publish"
  exit 0
fi
cargo publish -p xtmd --allow-dirty "$@"
