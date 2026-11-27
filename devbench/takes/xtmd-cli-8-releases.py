"""xtmd CLI, part 8: from one release to the next. compile --rir, decompile (from a resolved IR and
from an IR), and xtmd migrate --reader: a tree moved by what each release understands of it, every
file or none.

Every command and every line of output is real: devbench/takes/xtmd-cli/capture.py ran them with
this repo's xtmd and kept what they printed, and the player only lays that out. The old release's
xtmd is this one, 0.4.0, which the tree is written for: until 0.5.0 ships, what moves is the layout.
To record:

    python3 devbench/takes/xtmd-cli/capture.py --work /tmp/xtmdv releases
    python3 -m http.server -d /tmp/xtmdv/site 8765 &
    devbench video record xtmd-cli-8-releases --base http://127.0.0.1:8765
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "xtmd-cli"))
from reel import Reel  # noqa: E402

TITLE = "xtmd CLI 8: from one release to the next"


def take(t):
    r = Reel(t, "releases")
    r.card(3.4)

    r.chapter(0)
    r.layout("split")
    r.run("tree", "A tree written for a release: two documents, a pack of their own, and xtmd.toml.")
    r.file("tree", "books.xtmd", "before")
    t.beat(r.line("let first: text   =  \"Read\";"), "Not in fmt's layout, and with a comment above its let.", 3.2)
    r.layout("term")
    r.run("rir", "xtmd compile --rir writes the resolved IR: what the checker understood, for tools.")
    t.beat(r.out("rir", '"rir":2'), "Its own version, 2, the release that wrote it, and every file: the pack's and the document.", 3.8)
    r.run("comments", "Every comment is in it, on the node it sits with.")
    t.beat(r.out("comments", "// what is on the shelf"), "The let's leading comment, kept.", 2.8)

    r.chapter(1)
    r.run("back", "xtmd decompile writes a resolved IR back as source.")
    t.beat(r.out("back", "package shelf;"), "The pack first, as the resolved IR holds it…", 2.8)
    t.beat(r.out("back", "// what is on the shelf"), "…then the document, in fmt's layout, its comment where it was.", 3.4)
    r.run("from-ir", "It reads the IR a runtime runs too, which keeps far less.")
    t.beat(r.out("from-ir", "comments: an IR keeps none"), "So it says what it writes anew: one warning for each kind of loss.", 3.6)
    t.beat(r.out("from-ir", '@row_of_books(title = "Read");'), "The page comes back from what it runs: the let worked out into its value.", 3.6)

    r.chapter(2)
    r.run("old", "To move a tree, the xtmd of the release it is written for reads it: here 0.4.0's.")
    t.beat(r.out("old", "resolved IR 2"), "xtmd 0.4.0, IR 4, resolved IR 2.", 2.8)
    r.run("dry", "xtmd migrate --reader: the old release reads every file, this one prints them again, and compiles them.")
    t.beat(r.out("dry", "read by 0.4.0's xtmd as rir 2"), "Read by 0.4.0's xtmd as rir 2, and brought to this xtmd's version.", 3.4)
    t.beat(r.out("dry", "./books.xtmd: changes"), "One file changes. --dry-run writes nothing.", 2.8)
    r.run("diff", "--diff shows how.")
    t.beat(r.out("diff", '+let first: text = "Read";'), "Its layout is fmt's now; the comment stays.", 3.0)
    r.run("json", "--format json gives a line a file, and a summary a program can read.")
    t.beat(r.out("json", '"ready":true'), "Ready: nothing refused, no question open.", 2.8)
    r.run("yes", "--yes writes it.")
    t.beat(r.out("yes", "written: 1 file"), "Written.", 2.0)
    r.run("check", "And run again, there is nothing left to change.")
    t.beat(r.out("check", "nothing to change"), "--check passes: exit 0.", 2.6)

    r.chapter(3)
    r.run("broken-old", "Another tree, where one document no longer compiles.")
    t.beat(r.out("broken-old", "has no field `count`"), "The old release can't read notes.xtmd…", 3.0)
    t.beat(r.out("broken-old", "nothing written"), "…so nothing is written: not even books.xtmd, which it could move.", 3.4)
    r.run("untouched", "git agrees.")
    t.beat(r.out("untouched", "every file is as it was"), "Every file, or none.", 2.6)
    r.run("no-reader", "And a reader that can't be run is said once.")
    t.beat(r.out("no-reader", "can't be run"), "Nothing written, exit 1.", 2.6)

    r.end("Spec page 6 and docs/guides/migration.md say how a release's own step is written.", 3.4)
