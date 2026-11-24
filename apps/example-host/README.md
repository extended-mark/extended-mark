# A host of its own

A host written from the spec alone (docs/specs/06, The host's side): it takes a page's IR, draws the tree
itself and tells the page what the reader does. It uses no widgets of ours. `@extended-mark/runtime` is
the engine's inside, and any engine that gives the same tree would do.

- [draw.mjs](draw.mjs): each widget it draws, as one element. A widget that isn't there shows its Markdown.
- [server.mjs](server.mjs): the tree as HTML, each element with its node's id, for a reader with no script.
- [client.mjs](client.mjs): the same elements taken over by their ids, and each change made to the elements
  it is about.

```sh
npm install && npm run build && npm start     # http://localhost:8080
npm test
```
