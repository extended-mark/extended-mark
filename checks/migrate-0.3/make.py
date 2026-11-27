#!/usr/bin/env python3
"""The 0.3.2 corpus that `xtmd migrate` is tested on (compiler/xtmd/tests/migrate_032.rs), made again.

Every document and pack's file at tag v0.3.2, but core's (0.4.0 carries 0.3.2's core in
compiler/previous/), is copied to v0.3.2/ as it was, and the probes stay as they are in probes/. Each is
put in a unit: a folder with an xtmd.toml, a pack's folder, a folder holding documents and their packs,
or a file alone with its `<name>.packs/` folder. The released 0.3.2 xtmd says of each unit whether it
is valid, with its errors, and each valid document's Markdown: units.json.

    python3 checks/migrate-0.3/make.py            # fetches xtmd 0.3.2 from npm
    python3 checks/migrate-0.3/make.py --xtmd ~/xtmd-0.3.2/xtmd
"""
import argparse, json, os, shutil, subprocess, sys, tarfile, tempfile, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "../.."))
TAG = "v0.3.2"
EXT = (".xtmd", ".xtmdpack")
# a document that imports a pack kept elsewhere in the tag, as the tag's own tests built it
PACKS_ELSEWHERE = {"apps/playground/examples/release-flow.xtmd": ["examples/graphs/graphs.xtmdpack"]}


def fetch_xtmd(into):
    """xtmd 0.3.2 for this machine, from npm: @extended-mark/cli-linux-x64@0.3.2's bin/xtmd."""
    url = "https://registry.npmjs.org/@extended-mark/cli-linux-x64/-/cli-linux-x64-0.3.2.tgz"
    tgz = os.path.join(into, "cli.tgz")
    urllib.request.urlretrieve(url, tgz)
    with tarfile.open(tgz) as t:
        t.extractall(into, filter="data")
    x = os.path.join(into, "package", "bin", "xtmd")
    os.chmod(x, 0o755)
    return x


def tag_files():
    out = subprocess.run(["git", "-C", REPO, "ls-tree", "-r", "--name-only", TAG], capture_output=True, text=True, check=True).stdout
    return [p for p in out.splitlines() if p.endswith(EXT) and not p.startswith("core/")]


def units_of(paths):
    """Each unit: (id, the paths it moves, the paths given as --pack)."""
    tomls = set(subprocess.run(["git", "-C", REPO, "ls-tree", "-r", "--name-only", TAG], capture_output=True, text=True, check=True).stdout.split())
    trees = sorted({os.path.dirname(t) for t in tomls if t.endswith("/xtmd.toml") or t == "xtmd.toml"} - {""})
    units, taken = [], set()
    for t in trees:
        mine = [p for p in paths if p.startswith(t + "/")]
        if mine:
            units.append((t, mine, []))
            taken.update(mine)
    rest = [p for p in paths if p not in taken]
    folders = {}
    for p in rest:
        if ".packs/" in p:
            continue
        folders.setdefault(os.path.dirname(p), []).append(p)
    for d, files in sorted(folders.items()):
        if d.startswith("checks/cases") or d == "checks/core":
            for f in files:
                packs = os.path.splitext(f)[0] + ".packs"
                extra = sorted(p for p in rest if p.startswith(packs + "/"))
                units.append((f, [f], extra))
        else:
            # a folder's documents with its packs, a pack's folder, or files that stand alone
            docs = [f for f in files if f.endswith(".xtmd")]
            packs = [f for f in files if f.endswith(".xtmdpack")]
            if packs:
                units.append((d, files, packs))
            else:
                units.extend((f, [f], PACKS_ELSEWHERE.get(f, [])) for f in docs)
    # a pack used by documents further down (android compose's tokens): the folder above holds both
    return units


def run(xtmd, args, cwd):
    r = subprocess.run([xtmd] + args, capture_output=True, text=True, cwd=cwd)
    return r.returncode, r.stdout, r.stderr


def judge(xtmd, root, unit):
    uid, files, packs = unit
    pack_arg = ["--pack", ",".join(packs)] if packs else []
    code, out, err = run(xtmd, ["check"] + files + pack_arg + ["--format", "json"], root)
    errors = []
    for line in out.splitlines():
        try:
            j = json.loads(line)
        except ValueError:
            continue
        if j.get("type") == "diagnostic" and j.get("severity") == "error":
            errors.append("%s:%s:%s %s" % (j.get("file", ""), j.get("line"), j.get("column"), j.get("code")))
    valid = code == 0
    markdown = {}
    if valid:
        for f in files:
            if f.endswith(".xtmd"):
                c, md, e = run(xtmd, ["md", f] + pack_arg, root)
                markdown[f] = md if c == 0 else None
    return {"id": uid, "files": files, "packs": packs, "valid": valid, "errors": errors if not valid else [], "markdown": markdown}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--xtmd", help="the released xtmd 0.3.2; left out, it is fetched from npm")
    a = ap.parse_args()
    work = tempfile.mkdtemp(prefix="migrate-0.3-")
    xtmd = a.xtmd or fetch_xtmd(work)
    v = run(xtmd, ["--version"], work)[1].strip()
    if not v.startswith("xtmd 0.3.2"):
        sys.exit("%s is %r, not xtmd 0.3.2" % (xtmd, v))
    dest = os.path.join(HERE, TAG)
    shutil.rmtree(dest, ignore_errors=True)
    paths = tag_files()
    for p in paths + [p for p in subprocess.run(["git", "-C", REPO, "ls-tree", "-r", "--name-only", TAG], capture_output=True, text=True).stdout.split() if p.endswith("xtmd.toml")]:
        blob = subprocess.run(["git", "-C", REPO, "show", "%s:%s" % (TAG, p)], capture_output=True, check=True).stdout
        os.makedirs(os.path.dirname(os.path.join(dest, p)), exist_ok=True)
        with open(os.path.join(dest, p), "wb") as f:
            f.write(blob)
    units = [judge(xtmd, dest, u) for u in units_of(paths)]
    # the probes: each a tree of its own
    pdir = os.path.join(HERE, "probes")
    for name in sorted(os.listdir(pdir)):
        root = os.path.join(pdir, name)
        files = sorted(os.path.relpath(os.path.join(d, f), root) for d, _, fs in os.walk(root) for f in fs if f.endswith(EXT) and "/surfaces/" not in os.path.join(d, f) and "/migrations/" not in os.path.join(d, f))
        u = judge(xtmd, root, ("probes/" + name, files, []))
        units.append(u)
    with open(os.path.join(HERE, "units.json"), "w") as f:
        json.dump({"made_by": v, "tag": TAG, "units": units}, f, indent=1, sort_keys=True)
        f.write("\n")
    n = len(units)
    ok = sum(u["valid"] for u in units)
    print("%d units, %d valid in 0.3.2, %d refused by it; %d files" % (n, ok, n - ok, sum(len(u["files"]) for u in units)))
    shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
