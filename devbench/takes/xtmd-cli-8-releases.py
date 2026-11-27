"""xtmd CLI, part 8: from one release to the next. compile --rir, decompile (from a resolved IR and
from an IR), and xtmd migrate on a tree written for 0.3.2: moved by what each release understands of
it, what it leaves for a person said out loud, and a tree further back refused by name.

Every command and every line of output is real: devbench/takes/xtmd-cli/capture.py ran them with
this repo's xtmd and kept what they printed, and the player only lays that out. 0.3.2's front end is
built into 0.4.0's xtmd, so moving a tree needs no other xtmd.
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
    r.run("old", "A tree written for 0.3.2: its xtmd.toml says so.")
    r.file("old", "doc.xtmd", "before")
    t.beat(r.line('let intro: md = "# Hi";'), "0.3.2's md, its @blocks, and an input named warn, a keyword in 0.4.0.", 3.6)
    r.run("dry", "xtmd migrate: 0.3.2's own front end, built into this xtmd, reads every file; one step moves it.")
    t.beat(r.out("dry", "read as written for 0.3.2 (rir 1), moved to 0.4.0 (rir 2)"), "Read as 0.3.2 understood it, moved to 0.4.0's resolved IR.", 3.4)
    r.run("diff", "--diff shows how.")
    t.beat(r.out("diff", "+let intro: rich_text"), "md is the built-in rich_text…", 2.6)
    t.beat(r.out("diff", "+\\{intro}"), "…@blocks places its value where it stands…", 2.8)
    t.beat(r.out("diff", "+widget card(title: text, warn_: text) {"), "…and warn is warn_, everywhere it is read.", 3.0)
    r.run("json", "--format json gives a line a file, and a summary a program can read.")
    t.beat(r.out("json", '"ready":true'), "Ready: nothing refused, no question open, no TODO.", 2.8)
    r.run("yes", "--yes writes it, xtmd.toml too.")
    t.beat(r.out("yes", "written: 2 files"), "Written: every file, or none.", 2.4)
    r.run("check", "Run again, there is nothing left to move, and it compiles in 0.4.0.")
    t.beat(r.out("check", "1 up to date"), "--check passes, and so does xtmd check.", 2.8)

    r.chapter(3)
    r.run("todo", "Code that reads a rich_text's parts has no answer in 0.4.0.")
    t.beat(r.out("todo", "nothing reads its parts"), "So it is written with a TODO above it, saying why…", 3.0)
    t.beat(r.out("todo", "exit 3"), "…every TODO is said, and the run ends with exit 3.", 2.8)
    r.run("older", "A tree written for 0.2.9 is two releases back.")
    t.beat(r.out("older", "tools/migrate/chain.py"), "Refused by name: each release moves it in turn, and chain.py runs them.", 3.6)

    r.end("Spec page 6 and docs/guides/migration.md say how a release's own step is written.", 3.4)
