#!/bin/sh
# Publishes Extended Mark's crates to crates.io, each after the ones it depends on (cargo orders a workspace
# publish). A published crate holds only its own folder, so the repository's files a crate embeds are
# copied into it first: xtmd and xtmd-live embed all of core, xtmd-syntax core's entities, and
# xtmd-lsp checks/rules.json (their build scripts look in the crate before the repository); they are
# taken out after.
#
#   tools/release/publish-crates.sh [--dry-run]
#
# Publishing needs CARGO_REGISTRY_TOKEN in the environment; --dry-run needs nothing. A crate whose version
# is on crates.io already is left out, so a release run again after a failure publishes only the rest, and
# crates.io's limit on new crates (5 at once, then one every 10 minutes) is waited out. The ten published:
# xtmd, xtmd-live, xtmd-lsp, xtmd-syntax, xtmd-check, xtmd-ir, xtmd-lower, xtmd-eval, xtmd-markdown and
# xtmd-migrate. The workspace's other members have `publish = false` and cargo leaves them out:
# xtmd-schemagen (a tool here), xtmd-wasm (xtmd.wasm, in @extended-mark/wasm and on the GitHub
# release) and the example engines in engines/examples (xtmd-runtime, xtmd-planes, xtmd-egui-widgets, xtmd-egui,
# xtmd-egui-live, xtmd-android, xtmd-android-live).
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
log=$(mktemp)
trap 'cleanup; rm -f "$log"' EXIT
# crates.io takes a few new crates at once and then one every few minutes, and answers the next with
# 429 and a time to come back; the run waits until then (up to 15 minutes at a time) and goes on with
# the crates left, so a release with new crates takes longer instead of failing
for round in 1 2 3 4 5 6 7 8 9 10 11 12; do
  # the publishable members (publish isn't false) at their version, and the ones crates.io has already
  skip=""
  left=0
  for nv in $(cargo metadata --no-deps --format-version 1 | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{for(const p of JSON.parse(s).packages)if(!(Array.isArray(p.publish)&&p.publish.length===0))console.log(p.name+"@"+p.version)})'); do
    name=${nv%@*}; version=${nv#*@}
    if [ "$(curl -s -o /dev/null -w '%{http_code}' -A 'extended-mark release (https://github.com/extended-mark/extended-mark)' "https://crates.io/api/v1/crates/$name/$version")" = 200 ]; then
      echo "$name $version is on crates.io already: left out"
      skip="$skip --exclude $name"
    else
      left=$((left + 1))
    fi
  done
  if [ "$left" = 0 ]; then
    echo "every crate is on crates.io at its version already: nothing to publish"
    exit 0
  fi
  # shellcheck disable=SC2086
  if cargo publish --workspace --allow-dirty $skip "$@" 2>"$log"; then cat "$log" >&2; exit 0; fi
  cat "$log" >&2
  # only crates.io's limit is waited out; any other failure is the release's
  when=$(sed -n 's/.*429 Too Many Requests.*try again after \(.* GMT\).*/\1/p' "$log" | head -n 1)
  [ -n "$when" ] || exit 1
  wait=$(( $(date -d "$when" +%s) - $(date +%s) + 30 ))
  [ "$wait" -gt 0 ] || wait=30
  [ "$wait" -le 900 ] || { echo "crates.io asks to wait until $when: more than 15 minutes, so the release stops here" >&2; exit 1; }
  echo "crates.io's limit on new crates: waiting until $when, then publishing what's left (round $round)" >&2
  sleep "$wait"
done
echo "crates.io's limit outlasted twelve rounds: run the release again to publish what's left" >&2
exit 1
