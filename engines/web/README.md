# @extended-mark/web

Extended Mark's web engine: documents built ahead into HTML, a stylesheet, and a script for what of a page changes.
There is no runtime, no interpreter and no IR in what it makes.

```toml
# xtmd.toml
[engines]
web = ["xtmd-web"]
```

```sh
xtmd export docs --out dist --engine web
```

A page is whole before any script runs, and one that never changes has none. A script takes over the HTML that was
written: its elements stay the ones they were. Pages built together share what is the same in them.

## Plugging in

Everything after the engine's name is the engine's own:

| | |
|---|---|
| `--widgets FILE` | A module whose `widgets` are yours, by full name (`core/press`, `acme/chart`): each replaces ours, or is a pack's. A widget is a fn of plain elements, as in [@extended-mark/widgets](../../widgets/web). |
| `--only-widgets` | Draw with those, and none of ours. |
| `--tokens FILE` | Token values in place of the document's, as JSON. |
| `--shell FILE` | A module whose default is the page around a page: a fn of `{ name, title, body, styles, script, frontMatter }` to HTML. |
| `--fns FILE` | A module whose `fns` are value fns, by full name. |
| `--readable` | Scripts that can be read. |

From JavaScript, `build(job, options)` takes and gives what `schemas/build.schema.json` says.

## A page's script

It starts the page whose element (`data-xtmd-page`) is in the document it is loaded in, as `page`. A host that starts
its own gives the element `data-xtmd-held`, and calls `mount(element, { state, data, externs, fns, page, onChange })`.
Either gives a handle: `act(action)`, `data()`, `told()`, `left` and `destroy()`, in the formats of `schemas/`. It
stores nothing: what a host keeps is `data()`.
