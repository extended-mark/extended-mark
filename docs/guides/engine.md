# Writing an engine

An engine turns pages into files ahead of time: it takes their IR and gives HTML and scripts, components of a
framework, or whatever its platform runs. What it builds does what a [runtime](runtime.md) and a renderer do with the
page ([page 6](../specs/06-hosts-packs-implementers.md)), and holds only what the page uses.

It is for platforms whose reader would otherwise fetch a runtime with the page, as a browser does. A native program,
such as one with egui or Compose, has its runtime built in, once, for every page: it needs a renderer and no engine.

## A program

`xtmd export --engine` runs it:

```toml
# xtmd.toml
[engines]
web = ["node", "engine.mjs"]
```

```sh
xtmd export docs --out dist --engine web --theme plain
```

Every document's IR is on its standard input, all at once, and the files it makes go to its standard output
([build.schema.json](../../schemas/build.schema.json)). Its errors go to standard error, and a failure is its exit
code. Everything after its name is its own arguments. It writes nothing itself: xtmd does, once it has ended well.
It can be in any language. Ours for the web, [@extended-mark/web](../../engines/web), compiles: each expression and
rule becomes code, and what never changes is HTML alone. A small one draws each page as it starts:

```js
// engine.mjs: each page as HTML
import { Page } from "@extended-mark/runtime";
import { draw } from "./draw.mjs";

let text = "";
for await (const part of process.stdin) text += part;
const { documents } = JSON.parse(text);
const files = documents.map((d) => ({ path: `${d.name}.html`, text: draw(new Page(d.ir).tree()) }));
process.stdout.write(JSON.stringify({ version: 1, files }));
```

What the IR says of reads, writes, constants and whole numbers ([ir.schema.json](../../schemas/ir.schema.json)) is
there so that an engine prints and has nothing to work out.

## One of your own, from ours

Ours have a few named places to plug into, and each is a format: a widget set, token values, the page around a page
and the host's fns, for the web engine; a widget set for a renderer. Nothing is inherited from, and what is inside is
no promise. A widget is replaced by its full name (`core/press`), and a whole set is copied as a starting point: the
web's, egui's and Compose's copies run our own tests in CI. For more than that, copy the module. The formats
([schemas](../../schemas/README.md)) are all the copy has to keep.

## Checking it

How well an engine keeps to the spec is its implementer's to check; xtmd doesn't test them. What helps is to run a page
twice, as the engine built it and on a runtime, with the same events, and to compare: ours does so for every document in
this repository and for random runs (`engines/web/test`).
