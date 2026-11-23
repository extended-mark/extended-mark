# The badge pack (for the checks)

A tiny pack with externs, for the engines' tests (plan tab 15, "A pack with externs, for the
checks"): `badge/badge` is supplied by the pack on every engine it supports, and `badge/seal` by
none, so every engine shows its Markdown instead and reports it as missing (page 6).

- `badge.xtmdpack`: the pack.
- `supplies.json`: what the pack supplies on each engine. Headless engines (test, native) draw
  nothing, so for them the list is the implementation.
- `web.mjs`: the widget for @extended-mark/dom, which the web engine's check harness registers.
- `view.mjs`: the widget as an element, for the engines drawn from @extended-mark/view (React, Vue, Svelte,
  Solid and Angular, and their live engines), whose check harnesses register it.

Cases using it: checks/cases/packs/.
