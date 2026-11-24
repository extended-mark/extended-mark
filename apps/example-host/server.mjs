// The server's side: a page's tree as HTML, every element with its node's id, so a browser takes the same
// elements over. `node server.mjs` serves the example.

import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { fileURLToPath } from "node:url";
import { Page } from "@extended-mark/runtime";
import { nodes, shape, supplies } from "./draw.mjs";

const here = (f) => fileURLToPath(new URL(f, import.meta.url));
const escape = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const VOID = new Set(["br", "hr", "input"]);

function element(n) {
  const { tag, attrs = {}, words, holds } = shape(n);
  const all = { "data-id": n.id, ...attrs };
  const open = Object.entries(all).flatMap(([k, v]) => (v === undefined ? [] : [` ${k}="${escape(v)}"`]));
  if (VOID.has(tag)) return `<${tag}${open.join("")}>`;
  return `<${tag}${open.join("")}>${holds ? nodes(n.inputs[holds]).map(element).join("") : escape(words ?? "")}</${tag}>`;
}

/** What both sides give a page: the host's values, and what it draws. */
export const given = { externs: { "doc/clock": { today: { text: "Tuesday" } } }, supplies };

/** The page for an IR, with the data a reader had, as an HTML document that works before any script. */
export function render(ir, data) {
  const page = new Page(ir, { ...given, state: data?.state });
  if (data?.own) page.restoreOwn(data.own);
  const json = (x) => JSON.stringify(x).replace(/</g, "\\u003c");
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Tasks</title><style>:root{--muted:#6b7280;font-family:system-ui}body{margin:2rem auto;max-width:40rem}.row{display:flex;gap:.5rem;align-items:center}.column{display:flex;flex-direction:column;gap:.5rem}</style></head>
<body><main id="page">${page.tree().nodes.map(element).join("")}</main>
<script type="application/json" id="ir">${json(ir)}</script><script type="application/json" id="data">${json(page.data())}</script>
<script type="module" src="/client.js"></script></body></html>`;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const ir = JSON.parse(readFileSync(here("dist/page.ir.json"), "utf8"));
  createServer((req, res) => {
    const js = req.url === "/client.js";
    res.writeHead(200, { "content-type": js ? "text/javascript" : "text/html; charset=utf-8" });
    res.end(js ? readFileSync(here("dist/client.js")) : render(ir));
  }).listen(Number(process.env.PORT ?? 8080), () => console.log(`http://localhost:${process.env.PORT ?? 8080}`));
}
