# Extended Mark

A statically typed language for widgets in Markdown documents. One compiler (Rust) turns a document
into IR. Around it are modules anyone can replace:

| Folder | What it is |
|---|---|
| `compiler/` | The one compiler: the crate `xtmd`, its npm packages and WebAssembly |
| `core/` | The standard pack, in Extended Mark |
| `schemas/` | The JSON formats the modules meet in |
| `runtime/` | Runs a page from its IR: TypeScript and Rust, and the Rust one in Kotlin |
| `widgets/` | A widget set for each platform: web, egui, Compose |
| `renderers/` | Draw a running page: the plain web, the frameworks, egui, Compose |
| `engines/` | Turn a page into files ahead of time: the web, and through it the frameworks |
| `live/` | Compiler, runtime and renderer together, for a document compiled where it is read |

**Native platforms (egui, Compose) have no engine, by design**: the program has the runtime built in, once,
for every page. Kotlin holds the same Rust compiler and runtime as native libraries; nothing of the
language is written in Kotlin. See `.knowledge/architecture/modules.txt` and
`.knowledge/architecture/android-libraries.txt`.

## The notes: what they are, and how to use them

Two folders of short text files are written for you, an agent, so that you needn't search the repository
to find your way. They are part of it: versioned, reviewed, and checked by CI.

- **`.index/` says where things are.** A file an area (compiler, runtime, web drawing, Android, editors,
  tests, release, ...), and in it each folder and file that matters, with what it does in a few words.
  `.index/00-OVERVIEW.txt` lists the files and the tree. `.index/20-TASKS.txt` goes the other way: "I want
  to change X", and every place to touch, in order.
- **`.knowledge/` says why and how.** A small note a subject, at most 30 lines, in folders by theme:
  `architecture`, `compiler`, `runtime`, `drawing`, `migration`, `building`, `testing`, `conventions`. What
  isn't clear from one file: how parts fit, what must be changed together, what was learned the hard way.
- **`docs/specs/` says what the language means**, and neither folder repeats it.

How to use them:

1. Start of a task: read `.index/00-OVERVIEW.txt`, then the one area's file, and the task in
   `.index/20-TASKS.txt` if it is there.
2. `ls -R .knowledge`: the names are chosen so that this is enough to pick from. Read the one or two notes
   that fit, not all of them.
3. Then open the code they name. When a note and the code disagree, the code is what happens; find out
   which is wrong, and correct the note in the same commit.
4. Learned something that took trial and error, or that the next agent would have to work out again? Add it
   to the note it belongs in, or a new one named for its subject.

| Need | Go to |
|---|---|
| "Where does X live?" | [.index/](.index/) |
| "Why is X so, and what goes with it?" | [.knowledge/](.knowledge/) |
| "How do I run or test X?" | `.knowledge/testing/`, `.knowledge/building/` |
| What a word means here, how to write | `.knowledge/conventions/` |
| The language itself | [docs/specs/](docs/specs/), page by page in [README.md](README.md) |

## Ground rules

1. **The spec decides.** `docs/specs` says what the language means. When code and spec disagree, find
   out which is wrong before changing either. A change of meaning is written into the spec as its
   text, never as a dated note.
2. **One compiler.** Nothing else parses or checks Extended Mark. A module that has to work out what
   a page means is a sign the IR should have said it.
3. **No part of ours is special.** core is a pack like any other, and our modules and hosts use only
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

## Keeping the notes true

A change that moves, adds or removes something they name changes the note in the same commit.
`node tools/llm/check.mjs` fails when a path they name is gone, a part of the repository is in no file of
`.index/`, a line is over 110 characters, or a note has grown past its size. CI runs it. It can't tell whether
what a note says is still so: that is yours, for what you change.
