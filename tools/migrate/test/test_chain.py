"""chain.py against stand-in releases: each release's "xtmd" is a script that notes its run in the tree
and moves xtmd.toml to its own release, or ends with the exit code the test gives it.

    python3 -m unittest discover tools/migrate/test
"""
import hashlib, io, json, os, shutil, sys, tarfile, tempfile, unittest
from contextlib import redirect_stderr, redirect_stdout

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import chain  # noqa: E402

PLATFORM = "linux-x64"
SCRIPT = """#!/bin/sh
# a stand-in xtmd {v}: `xtmd migrate <tree> --yes [args]`
tree="$2"
echo "{v} $*" >> "$tree/chain.log"
code=$(cat "$tree/exit-{v}" 2>/dev/null)
if [ -n "$code" ]; then exit "$code"; fi
printf 'xtmd = "{v}"\\n' > "$tree/xtmd.toml"
"""


class Chain(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="chain-")
        self.addCleanup(shutil.rmtree, self.dir, True)
        self.assets = os.path.join(self.dir, "assets")
        self.tree = os.path.join(self.dir, "tree")
        os.makedirs(self.tree)
        self.release("0.3.2", "0.4.0", "0.4.1", "0.4.2", "0.5.0")
        with open(os.path.join(self.tree, "xtmd.toml"), "w") as f:
            f.write('xtmd = "0.3.2"\n')

    def release(self, *versions):
        """Each release's assets as GitHub has them, and the list of releases."""
        for v in versions:
            home = os.path.join(self.assets, f"v{v}")
            os.makedirs(home)
            name = f"xtmd-{v}-{PLATFORM}.tar.gz"
            data = SCRIPT.format(v=v).encode()
            with tarfile.open(os.path.join(home, name), "w:gz") as t:
                info = tarfile.TarInfo("xtmd")
                info.size, info.mode = len(data), 0o755
                t.addfile(info, io.BytesIO(data))
            digest = hashlib.sha256(open(os.path.join(home, name), "rb").read()).hexdigest()
            with open(os.path.join(home, "SHA256SUMS"), "w") as f:
                f.write(f"{digest}  {name}\n")
        # GitHub's list, newest first, with a draft and a pre-release that are never run
        listed = [{"tag_name": f"v{v}", "draft": False, "prerelease": False} for v in reversed(versions)]
        listed += [{"tag_name": "v0.4.3", "draft": True, "prerelease": False}, {"tag_name": "v0.4.4", "draft": False, "prerelease": True}]
        with open(os.path.join(self.dir, "releases.json"), "w") as f:
            json.dump(listed, f)

    def run_chain(self, *args):
        out, err = io.StringIO(), io.StringIO()
        argv = [self.tree, "--index", "file://" + os.path.join(self.dir, "releases.json"), "--assets", "file://" + self.assets, "--platform", PLATFORM, "--cache", os.path.join(self.dir, "cache"), *args]
        with redirect_stdout(out), redirect_stderr(err):
            code = chain.main(argv)
        return code, out.getvalue() + err.getvalue()

    def runs(self):
        path = os.path.join(self.tree, "chain.log")
        return [l.split()[0] for l in open(path)] if os.path.exists(path) else []

    def written_for(self):
        return chain.tree_release(self.tree)

    def test_each_release_in_between_runs_in_order_patch_releases_too(self):
        code, said = self.run_chain()
        self.assertEqual(code, 0, said)
        self.assertEqual(self.runs(), ["0.4.0", "0.4.1", "0.4.2", "0.5.0"])
        self.assertEqual(self.written_for(), (0, 5, 0))

    def test_to_stops_at_the_release_asked_for(self):
        code, said = self.run_chain("--to", "0.4.1")
        self.assertEqual(code, 0, said)
        self.assertEqual(self.runs(), ["0.4.0", "0.4.1"])
        self.assertEqual(self.written_for(), (0, 4, 1))

    def test_a_run_that_leaves_something_for_a_person_stops_the_chain_there(self):
        open(os.path.join(self.tree, "exit-0.4.1"), "w").write("3")
        code, said = self.run_chain()
        self.assertEqual(code, 3, said)
        self.assertEqual(self.runs(), ["0.4.0", "0.4.1"])
        self.assertIn("stopped at 0.4.1", said)
        # once the person has settled it in 0.4.1's terms, the chain goes on from there
        os.unlink(os.path.join(self.tree, "exit-0.4.1"))
        with open(os.path.join(self.tree, "xtmd.toml"), "w") as f:
            f.write('xtmd = "0.4.1"\n')
        code, said = self.run_chain()
        self.assertEqual(code, 0, said)
        self.assertEqual(self.runs(), ["0.4.0", "0.4.1", "0.4.2", "0.5.0"])

    def test_a_run_that_fails_stops_the_chain_with_its_code(self):
        open(os.path.join(self.tree, "exit-0.4.2"), "w").write("1")
        code, said = self.run_chain()
        self.assertEqual(code, 1, said)
        self.assertIn("exit code 1", said)
        self.assertEqual(self.written_for(), (0, 4, 1))

    def test_an_asset_whose_hash_differs_is_never_run(self):
        with open(os.path.join(self.assets, "v0.4.1", "SHA256SUMS"), "w") as f:
            f.write(f"{'0' * 64}  xtmd-0.4.1-{PLATFORM}.tar.gz\n")
        code, said = self.run_chain()
        self.assertEqual(code, 1, said)
        self.assertIn("isn't what release 0.4.1's SHA256SUMS says", said)
        self.assertEqual(self.runs(), ["0.4.0"])

    def test_what_follows_two_dashes_is_given_to_each_run(self):
        code, said = self.run_chain("--to", "0.4.0", "--", "--answers", "answers.toml")
        self.assertEqual(code, 0, said)
        self.assertIn("--answers answers.toml", open(os.path.join(self.tree, "chain.log")).read())

    def test_a_tree_that_says_no_release_or_one_that_isnt_one_is_a_usage_error(self):
        with open(os.path.join(self.tree, "xtmd.toml"), "w") as f:
            f.write('packs = ["kit"]\n')
        self.assertEqual(self.run_chain()[0], 2)
        with open(os.path.join(self.tree, "xtmd.toml"), "w") as f:
            f.write('xtmd = "0.3.9"\n')
        self.assertEqual(self.run_chain()[0], 2)


if __name__ == "__main__":
    unittest.main()
