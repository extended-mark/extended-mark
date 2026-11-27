# Engines

An engine turns a page into files ahead of time: it reads the page's IR and writes code for a platform, which then holds
no runtime of ours. `xtmd.toml` names it, and `xtmd export page.xtmd --engine web --out dist` runs it.

- **web/**: `@extended-mark/web`, HTML, CSS and scripts.
- React, Next, Vue, Svelte, Solid and Angular: a component of the framework's. Each is in its renderer's package
  (`renderers/react` and so on), beside the renderer that draws a page from its IR, and uses `@extended-mark/web`.

What draws a page from its IR while a program runs is a runtime (`runtime/`) with a renderer (`renderers/`) and a widget
set (`widgets/`); with the compiler beside them it is a live document (`live/`). A native program holds the runtime
once, for every page, and needs no engine.
