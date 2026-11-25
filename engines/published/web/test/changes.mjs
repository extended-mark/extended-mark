// Random changes to a page: what a reader or a host could do to it now.

import { everyNode } from "@extended-mark/runtime";

export function random(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const pick = (xs, rnd) => xs[Math.floor(rnd() * xs.length)];

/** A value like another, and sometimes one that isn't: a host gives what it likes. */
export function like(x, rnd, deep = 0) {
  if (rnd() < 0.06) return pick([{ num: "1.5" }, { text: "?" }, { bool: true }, { list: [] }, { none: {} }], rnd);
  if ("num" in x) return { num: pick(["0", "1", "2", "3", "7", "-1", "10", "0.5", "100"], rnd) };
  if ("text" in x) return { text: pick(["", "a", "lemon", "a *b* <i>", "Ω≈ç"], rnd) };
  if ("bool" in x) return { bool: rnd() < 0.5 };
  if ("list" in x && deep < 4) {
    const of = x.list.map((y) => (rnd() < 0.3 ? like(y, rnd, deep + 1) : y));
    const r = rnd();
    if (r < 0.2) of.pop();
    else if (r < 0.4 && of.length) of.push(like(of[0], rnd, deep + 1));
    else if (r < 0.6) of.reverse();
    return { list: of };
  }
  if ("record" in x && deep < 4) return { record: { type: x.record.type, fields: Object.fromEntries(Object.entries(x.record.fields).map(([k, y]) => [k, rnd() < 0.4 ? like(y, rnd, deep + 1) : y])) } };
  if ("some" in x) return rnd() < 0.3 ? { none: {} } : { some: like(x.some, rnd, deep + 1) };
  return x;
}

/** Something a reader or a host could do now: a node's place among the ids on the page, and the change. */
export function changeOf(rnd, ir, { state, nodes, ids, past = [] }) {
  const acts = [() => ({ page: { values: { theme: { case: { enum: "core/theme", name: pick(["DARK", "LIGHT"], rnd) } }, width: { case: { enum: "core/width", name: pick(["NARROW", "MEDIUM", "WIDE"], rnd) } } } } })];
  for (const k of Object.keys(state)) acts.push(() => ({ set: { fields: { [k]: like(state[k], rnd) } } }));
  if (Object.keys(state).length > 1) acts.push(() => ({ set: { fields: Object.fromEntries(Object.entries(state).map(([k, x]) => [k, like(x, rnd)])) } }));
  if (past.length) acts.push(() => ({ restore: { data: pick(past, rnd) } }));
  for (const d of ir.declarations) {
    const x = d.declare;
    if ("expose" in x) for (const f of x.expose.fns) acts.push(() => ({ expose: { channel: x.expose.channel, name: f.name, args: f.params.map((p) => like({ "core/text": { text: "" }, "core/bool": { bool: true } }[p.type.name] ?? { num: "1" }, rnd)) } }));
    if ("extern" in x && "channel" in x.extern) for (const v of x.extern.channel.values ?? []) if (v.default && !x.extern.channel.name.startsWith("core/")) acts.push(() => ({ extern: { channel: x.extern.channel.name, values: { [v.name]: like(v.default, rnd) } } }));
  }
  for (const node of everyNode(nodes)) {
    const at = ids.indexOf(node.id);
    if (at < 0) continue;
    const decl = ir.declarations.map((d) => d.declare.extern?.widget).find((w) => w?.name === node.widget);
    for (const e of node.events ?? []) {
      const params = decl?.events?.find((x) => x.name === e)?.params ?? [];
      const arg = (p) => ({ "core/text": { text: "x" }, "core/point": { record: { type: "core/point", fields: { x: { num: "1" }, y: { num: "2" } } } } })[p.type.name] ?? { num: "1" };
      for (let k = 0; k < 2; k++) acts.push(() => ({ event: { at, name: e, args: params.map(arg) } }));
    }
    for (const w of Object.keys(node.writes ?? {})) {
      const now = node.inputs[w];
      if (now && w !== "box" && w !== "drawn") acts.push(() => ({ write: { at, input: w, value: like(now, rnd) } }));
    }
  }
  return pick(acts, rnd)();
}
