# The badge pack (for the checks)

A tiny pack with externs, for the renderers' tests (plan tab 15, "A pack with externs, for the
checks"): `badge/badge` is supplied by the pack on every platform it supports, and `badge/seal` by
none, so every renderer shows its Markdown instead and reports it as missing (page 6).

- `badge.xtmdpack`: the pack.
- `web.mjs`: the widget for @extended-mark/dom, which the web renderer's check harness registers.
- `view.mjs`: the widget as an element, for the renderers drawn from the widget set (React, Vue, Svelte,
  Solid and Angular, and their live documents), whose check harnesses register it.

Cases using it: checks/cases/packs/.
