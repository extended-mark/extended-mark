# Extended Mark

A statically typed language for widgets in Markdown documents. One compiler (Rust) turns a document
into IR; engines draw that IR on a platform.

## Where to look

| Need | Go to |
|---|---|
| "Where does X live?" | [.index/](.index/) (start with `00-OVERVIEW.txt`) |
| Architecture & conventions | [.knowledge/](.knowledge/) (`ls -R .knowledge`, then read the note you need) |
| Ground rules | [CLAUDE.md](CLAUDE.md) (below) |
| The language itself | [docs/specs/](docs/specs/), page by page in [README.md](README.md) |

`.knowledge/` works like memories: one small note a subject, named so that `ls` is enough to choose.
Read the one or two that fit, not all of them.

## Ground rules

1. **The spec decides.** `docs/specs` says what the language means. When code and spec disagree, find
   out which is wrong before changing either. A change of meaning is written into the spec as its
   text, never as a dated note.
2. **One compiler.** Nothing else parses or checks Extended Mark. A module that has to work out what
   a page means is a sign the IR should have said it.
3. **No part of ours is special.** core is a pack like any other, and our engines and hosts use only
   what anyone's may.
4. **Every rule has a case.** A compile error or warning is a code, a row in `checks/rules.json` and a
   case in `checks/cases`. Cases compare code and place, never the message.
5. **Two runtimes, one behaviour.** A change to what a page does is made in TypeScript and in Rust, and
   tested in both.
6. **Generated files aren't edited.** Change the schema in `schemas/`, then run `xtmd-schemagen`.
7. **A bug is shown before it is fixed**, with a test that fails, and checked against the spec first:
   what looks wrong may be a rule.
8. **Nothing skipped.** A test that can't run is reported; it is never turned off to pass.
9. **What is released stays.** A change to the language, the IR or a pack's surface comes with how what
   exists moves to it. An IR version that hasn't shipped is changed in place.
10. **Write like the code around you**: full sentences in comments, saying why, in the words of the
    spec (`.knowledge/conventions/words.txt`).

## Keeping these notes true

A change that moves, adds or removes something they name changes the note in the same commit.
`node tools/llm/check.mjs` fails when a path they name is gone, a part of the repository is in no index
file, or a note has grown past its size. CI runs it.
