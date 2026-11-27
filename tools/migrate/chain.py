#!/usr/bin/env python3
"""A tree moved across several releases of Extended Mark, one release at a time, as a person would.

Each release's `xtmd migrate` moves only a tree written for the release right before it (spec page 6),
so a tree further back is moved by every release in between, in turn, patch releases too. This reads
the release the tree is written for (its xtmd.toml), fetches the command line of each release after it
up to --to (the GitHub release's xtmd-<release>-<os>-<cpu>.tar.gz, checked against the release's
SHA256SUMS), and runs `xtmd migrate <tree> --yes` with each. It stops at the first run that doesn't end
cleanly: exit 3 means a person must look (TODOs written, or a question to answer, in that release's
terms), and they run this again once they have; anything else is an error, said as it is.

    python3 tools/migrate/chain.py path/to/tree                 # up to the latest release
    python3 tools/migrate/chain.py path/to/tree --to 0.6.0
    python3 tools/migrate/chain.py path/to/tree -- --answers answers.toml

Exit codes: 0 when the tree is written for --to, 3 when a release's run left something for a person,
1 when a run failed or an asset can't be trusted, 2 for a usage error.
"""
import argparse, hashlib, json, os, platform, re, subprocess, sys, tarfile, tempfile, urllib.request

REPO = "extended-mark/extended-mark"
INDEX = f"https://api.github.com/repos/{REPO}/releases?per_page=100"
ASSETS = f"https://github.com/{REPO}/releases/download"
FOR_A_PERSON = 3


def version(v):
    m = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", v.strip())
    if not m:
        raise ValueError(f"{v!r} isn't a release, such as 0.4.0")
    return tuple(int(x) for x in m.groups())


def show(v):
    return ".".join(map(str, v))


def tree_release(tree):
    """The release xtmd.toml says the tree is written for, or None when it says none."""
    path = os.path.join(tree, "xtmd.toml")
    if not os.path.isfile(path):
        return None
    for line in open(path, encoding="utf-8"):
        m = re.match(r'\s*xtmd\s*=\s*"([^"]*)"', line)
        if m:
            return version(m.group(1))
    return None


def fetch(url):
    with urllib.request.urlopen(url) as r:
        return r.read()


def releases(index):
    """Every release, oldest first: from GitHub's list of releases (drafts and pre-releases left out),
    or from a JSON list of versions."""
    data = json.loads(fetch(index))
    out = set()
    for r in data:
        if isinstance(r, str):
            out.add(version(r))
        elif not r.get("draft") and not r.get("prerelease"):
            try:
                out.add(version(r["tag_name"]))
            except ValueError:
                pass
    return sorted(out)


def this_platform():
    os_name = {"linux": "linux", "darwin": "darwin", "win32": "win32"}.get(sys.platform, sys.platform)
    cpu = {"x86_64": "x64", "amd64": "x64", "aarch64": "arm64", "arm64": "arm64"}.get(platform.machine().lower(), platform.machine())
    return f"{os_name}-{cpu}"


def command_of(release, assets, plat, cache):
    """The release's xtmd, fetched once into the cache, its archive checked against SHA256SUMS."""
    v = show(release)
    home = os.path.join(cache, v, plat)
    exe = os.path.join(home, "xtmd.exe" if plat.startswith("win32") else "xtmd")
    if os.path.isfile(exe):
        return exe
    name = f"xtmd-{v}-{plat}.tar.gz"
    sums = fetch(f"{assets}/v{v}/SHA256SUMS").decode()
    want = next((l.split()[0] for l in sums.splitlines() if l.split()[1:2] == [name] or l.split()[1:2] == ["*" + name]), None)
    if want is None:
        raise RuntimeError(f"release {v}'s SHA256SUMS has no {name}: there is no command line of {v} for {plat}")
    archive = fetch(f"{assets}/v{v}/{name}")
    got = hashlib.sha256(archive).hexdigest()
    if got != want:
        raise RuntimeError(f"{name} isn't what release {v}'s SHA256SUMS says (sha256 {got}, not {want}): nothing is run with it")
    os.makedirs(home, exist_ok=True)
    with tempfile.NamedTemporaryFile(suffix=".tar.gz", delete=False) as t:
        t.write(archive)
    try:
        with tarfile.open(t.name) as tar:
            tar.extractall(home, filter="data")
    finally:
        os.unlink(t.name)
    os.chmod(exe, 0o755)
    return exe


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("tree", help="the tree to move: the folder with its xtmd.toml")
    ap.add_argument("--to", help="the release to move it to; the latest when left out")
    ap.add_argument("--index", default=INDEX, help="where the list of releases is (GitHub's, or a JSON list of versions)")
    ap.add_argument("--assets", default=ASSETS, help="where each release's assets are, by v<release>/")
    ap.add_argument("--platform", default=None, help="os-cpu of the command lines to fetch, such as linux-x64")
    ap.add_argument("--cache", default=os.path.join(os.path.expanduser("~"), ".cache", "xtmd", "releases"))
    argv = sys.argv[1:] if argv is None else list(argv)
    # what follows `--` is given to each xtmd migrate as it is
    rest = argv[argv.index("--") + 1:] if "--" in argv else []
    a = ap.parse_args(argv[:argv.index("--")] if "--" in argv else argv)
    try:
        all_releases = releases(a.index)
        start = tree_release(a.tree)
        to = version(a.to) if a.to else all_releases[-1]
    except (ValueError, IndexError, OSError) as e:
        print(f"chain.py: {e}", file=sys.stderr)
        return 2
    if start is None:
        print(f"{a.tree}/xtmd.toml says no release (xtmd = \"…\"): say which release the tree is written for, then run this again", file=sys.stderr)
        return 2
    if start not in all_releases:
        print(f"the tree says it is written for {show(start)}, which isn't a release", file=sys.stderr)
        return 2
    steps = [r for r in all_releases if start < r <= to]
    if not steps:
        print(f"the tree is written for {show(start)}: nothing to move to {show(to)}")
        return 0
    print(f"{show(start)} to {show(to)}: " + ", ".join(show(r) for r in steps))
    plat = a.platform or this_platform()
    for r in steps:
        try:
            exe = command_of(r, a.assets, plat, a.cache)
        except (RuntimeError, OSError) as e:
            print(f"stopped before {show(r)}: {e}", file=sys.stderr)
            return 1
        print(f"== xtmd {show(r)} migrate {a.tree}", flush=True)
        code = subprocess.run([exe, "migrate", a.tree, "--yes", *rest]).returncode
        if code == FOR_A_PERSON:
            print(f"stopped at {show(r)}: its run left something for a person (TODOs written, or a question to answer). Settle it in {show(r)}'s terms, then run this again: it goes on from there.")
            return FOR_A_PERSON
        if code != 0:
            print(f"stopped at {show(r)}: its xtmd migrate ended with exit code {code}, and the tree is as {show(r)}'s run left it", file=sys.stderr)
            return 1
        now = tree_release(a.tree)
        if now != r:
            print(f"stopped at {show(r)}: its run ended cleanly, but xtmd.toml says {show(now) if now else 'no release'}, not {show(r)}", file=sys.stderr)
            return 1
    print(f"the tree is written for {show(to)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
