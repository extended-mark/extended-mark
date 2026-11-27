#!/bin/sh
# Installs xtmd, Extended Mark's command, from the latest release: the binary for this machine
# (Linux on x64 or arm64, macOS on Intel or Apple silicon), checked against the release's
# SHA256SUMS. Needs nothing but curl (or wget), tar and a shell.
#
#   curl -fsSL https://raw.githubusercontent.com/extended-mark/extended-mark/main/install.sh | sh
#
#   XTMD_VERSION=0.3.0      that release, not the latest
#   XTMD_INSTALL_DIR=dir    where xtmd goes (default ~/.local/bin)
#   XTMD_BASE_URL=url       where the releases are (default the repository's on GitHub)
set -eu

base="${XTMD_BASE_URL:-https://github.com/extended-mark/extended-mark/releases}"
dir="${XTMD_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*"; }
die() { printf 'xtmd install: %s\n' "$*" >&2; exit 1; }

if command -v curl > /dev/null 2>&1; then
  fetch() { curl -fsSL -o "$2" "$1"; }
  final_url() { curl -fsSLI -o /dev/null -w '%{url_effective}' "$1"; }
elif command -v wget > /dev/null 2>&1; then
  fetch() { wget -q -O "$2" "$1"; }
  final_url() { wget -S --spider --max-redirect=20 "$1" 2>&1 | sed -n 's/^ *Location: *\([^ ]*\).*/\1/p' | tail -n 1 | tr -d '\r'; }
else
  die "needs curl or wget"
fi

case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=darwin ;;
  *) die "this script is for Linux and macOS; on Windows, run install.ps1 (see the README)" ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) cpu=x64 ;;
  aarch64 | arm64) cpu=arm64 ;;
  *) die "no xtmd for $(uname -m): there are builds for x64 and arm64 ('npm i -g extended-mark' also falls back to WebAssembly)" ;;
esac
# an x64 shell under Rosetta on Apple silicon gets the native build
if [ "$os" = darwin ] && [ "$cpu" = x64 ] && [ "$(sysctl -n sysctl.proc_translated 2> /dev/null || echo 0)" = 1 ]; then
  cpu=arm64
fi
platform="$os-$cpu"

version="${XTMD_VERSION:-}"
if [ -z "$version" ]; then
  # the latest release's page is its tag's: …/releases/tag/v0.3.0
  tag="$(final_url "$base/latest" | sed -n 's#.*/tag/##p')"
  [ -n "$tag" ] || die "couldn't find the latest release at $base/latest (set XTMD_VERSION to choose one)"
  version="${tag#v}"
fi
version="${version#v}"

asset="xtmd-$version-$platform.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

say "Installing xtmd $version for $platform…"
fetch "$base/download/v$version/$asset" "$tmp/$asset" || die "couldn't download $base/download/v$version/$asset"
fetch "$base/download/v$version/SHA256SUMS" "$tmp/SHA256SUMS" || die "couldn't download the release's SHA256SUMS"

want="$(awk -v f="$asset" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")"
[ -n "$want" ] || die "SHA256SUMS has no line for $asset"
if command -v sha256sum > /dev/null 2>&1; then
  got="$(sha256sum "$tmp/$asset" | awk '{ print $1 }')"
else
  got="$(shasum -a 256 "$tmp/$asset" | awk '{ print $1 }')"
fi
[ "$got" = "$want" ] || die "$asset doesn't match the release's SHA256SUMS: nothing installed"

tar -xzf "$tmp/$asset" -C "$tmp"
[ -f "$tmp/xtmd" ] || die "$asset has no xtmd in it"
mkdir -p "$dir"
cp "$tmp/xtmd" "$dir/xtmd.new"
chmod 755 "$dir/xtmd.new"
mv -f "$dir/xtmd.new" "$dir/xtmd"

say "Installed $("$dir/xtmd" --version 2> /dev/null || echo "xtmd $version") at $dir/xtmd"
case ":$PATH:" in
  *":$dir:"*) ;;
  *)
    say ""
    say "$dir isn't on your PATH yet. Add it, then open a new terminal:"
    say "  echo 'export PATH=\"$dir:\$PATH\"' >> ~/.profile    # or ~/.zshrc, ~/.bashrc"
    ;;
esac
