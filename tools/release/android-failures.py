"""Which of the Android tests failed, as a workflow's annotations.

    android-failures.py <the repository>

Ends well only when tests failed and every one of them never started, because Robolectric's Android couldn't be
fetched: those may be tried again."""

import glob
import sys
import xml.etree.ElementTree as tree

UNFETCHED = "Failed to fetch maven artifact"


def failures(repo):
    for f in sorted(glob.glob(f"{repo}/**/build/test-results/testDebugUnitTest/*.xml", recursive=True)):
        for t in tree.parse(f).getroot().iter("testcase"):
            for bad in t.findall("failure") + t.findall("error"):
                yield f"{t.get('classname')}.{t.get('name')}", (bad.get("message") or "no reason given").replace("\n", " ")[:400]


if __name__ == "__main__":
    failed = list(failures(sys.argv[1]))
    for name, why in failed:
        print(f"::error::{name}: {why}")
    sys.exit(0 if failed and all(UNFETCHED in why for _, why in failed) else 1)
