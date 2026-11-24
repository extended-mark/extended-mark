// The kinds of part a view is made of, each drawn by a fn of its own: a page holds the ones it has.

import { Failure, type Place } from "@extended-mark/runtime/ops";
import { asNum, none, num, type Value } from "@extended-mark/runtime/values";
import { NONE, STOP, join, type Bound, type Contracts, type Drawn, type Env, type Handler, type Inst, type Kit, type P, type Part, type Template, type Widget, type X } from "./kit.js";

type Part_<N> = (m: Kit, n: N, e: Env, out: Drawn[]) => Env | void;

// ---- what is kept

/** What a loop, a branch, a place or a local gives: as it was, when it is the same one with the same locals
 * around it and nothing it read has changed. */
function hold(m: Kit, n: Part, e: Env, work: () => { drawn: Drawn[]; value?: Value }): { drawn: Drawn[]; value?: Value } {
  const key = `n|${e.k}|${n.s}`;
  const was = m.memo.get(key);
  // the same content placed twice in one redraw is two
  const twice = m.nodeSeen.has(key) || was?.frame.seen === m.draws;
  if (was && !twice && was.n === n && was.epoch === m.epoch && was.p === e.p && was.o === e.o && m.unchanged(was.frame) && m.sameBound(was.b, e.b)) {
    m.replay(was.frame);
    return was;
  }
  m.nodeSeen.add(key);
  const frame = m.enter();
  let made: { drawn: Drawn[]; value?: Value };
  try {
    made = work();
  } finally {
    m.exit(frame);
  }
  if (!frame.dirty && !twice) m.memo.set(key, { frame, n, p: e.p, o: e.o, b: e.b, epoch: m.epoch, ...made });
  else if (!twice) m.memo.delete(key);
  return made;
}

/** A part held, whose failure in the document's own code is shown where it would be. */
function heldPart(m: Kit, n: Part, e: Env, out: Drawn[], work: () => Drawn[]): void {
  try {
    out.push(...hold(m, n, e, () => ({ drawn: work() })).drawn);
  } catch (f) {
    if (!m.contains(e)) throw f;
    out.push(m.failed("doc", join(e.p, n.s), join(e.k, n.s), f));
  }
}

// ---- a `let`

export interface Let extends Part {
  x: X;
}

/** Its name is what its place is called: `let total`. What fails stops what comes after, which may read it. */
export const local: Part_<Let> = (m, n, e, out) => {
  try {
    const { value } = hold(m, n, e, () => ({ drawn: [], value: n.x(m, e.b, e.o) }));
    return { ...e, b: [...e.b, value!] };
  } catch (f) {
    if (!m.contains(e)) throw f;
    out.push(m.failed("doc", join(e.p, n.s), join(e.k, n.s), f));
    return STOP;
  }
};

// ---- loops

export interface Row {
  k: number | string;
  b: Bound[];
}

export interface Loop extends Part {
  r: (m: Kit, b: Bound[], o?: Inst) => Row[];
  v: Part[];
}

export const loop: Part_<Loop> = (m, n, e, out) =>
  heldPart(m, n, e, out, () => {
    const drawn: Drawn[] = [];
    n.r(m, e.b, e.o).forEach((row, k) => m.parts(n.v, { o: e.o, b: [...e.b, ...row.b], p: join(e.p, `${n.s}[${k}]`), k: join(e.k, `${n.s}[${row.k}]`) }, drawn));
    return drawn;
  });

/** The rows a list or a range gives; `at` says whether a row has its position too. */
export function rows(m: Kit, over: Value, at: boolean): Row[] {
  const row = (k: number, key: number | string, v: Value): Row => {
    m.step();
    return { k: key, b: at ? [num(k), v] : [v] };
  };
  if (over.kind === "list") return over.items.map((i, k) => row(k, i.id, i.value));
  if (over.kind === "range") return m.walkRange(over).map((v, k) => row(k, asNum(v)?.show() ?? String(k), v));
  throw Failure.ir("`for` over something that isn't a list or a range");
}

/** The rows of a `mut for`: each a reference to its item. */
export function mutRows(m: Kit, list: Place, at: boolean): Row[] {
  const v = m.readPlace(list);
  if (v.kind !== "list") throw Failure.ir("`mut for` over something that isn't a list");
  return v.items.map((i, k) => {
    m.step();
    const ref: Bound = { ref: { root: list.root, path: [...list.path, { kind: "itemOpt", id: i.id }] } };
    return { k: i.id, b: at ? [num(k), ref] : [ref] };
  });
}

// ---- branches: `if` and `match`

export interface Branch extends Part {
  /** The parts it takes, and what they bind. */
  f: (m: Kit, b: Bound[], o?: Inst) => [Part[], Bound[]];
}

export const branch: Part_<Branch> = (m, n, e, out) =>
  heldPart(m, n, e, out, () => {
    const [parts, b] = n.f(m, e.b, e.o);
    return m.parts(parts, b.length ? { ...e, b: [...e.b, ...b] } : e);
  });

/** `if (mut x = …)`: the place inside an opt, or nothing when it is empty. */
export function inside(m: Kit, target: Place): Place | undefined {
  // `xs[i]` is an opt: none past the end, and otherwise the item itself, by its identity
  const last = target.path[target.path.length - 1];
  if (last?.kind === "index") {
    const list: Place = { root: target.root, path: target.path.slice(0, -1) };
    const lv = m.readPlace(list);
    const item = lv.kind === "list" && last.index >= 0 ? lv.items[last.index] : undefined;
    if (!item) return undefined;
    target = { root: target.root, path: [...list.path, { kind: "item", id: item.id }] };
  }
  const tl = target.path[target.path.length - 1];
  const path = tl?.kind === "itemOpt" ? [...target.path.slice(0, -1), { kind: "item" as const, id: tl.id }] : [...target.path, { kind: "inside" as const }];
  const now = m.readPlace(target);
  if (now.kind === "opt") return now.value === null ? undefined : { root: target.root, path };
  // a plain value where an opt is wanted is always there
  return { root: target.root, path: path.slice(0, -1) };
}

// ---- views placed

export interface Placed extends Part {
  x: X;
  /** It goes in a sentence. */
  sen?: boolean;
}

export const placed: Part_<Placed> = (m, n, e, out) =>
  heldPart(m, n, e, out, () => {
    const v = n.x(m, e.b, e.o);
    if (n.sen) checkSentence(v);
    return view(m, v, join(e.p, n.s), e, join(e.k, n.s));
  });

/** A sentence goes here, so a rich_text of blocks doesn't. */
export function checkSentence(v: Value): void {
  if (v.kind === "opt" && v.value) checkSentence(v.value);
  else if (v.kind === "content" && v.blocks) throw new Failure("Ir", "a sentence goes here, and this rich_text is blocks");
}

interface ViewValue {
  of: Widget | Template;
  args: Map<string, Value>;
  slots: Record<string, Part[]>;
  events: Record<string, Handler>;
  captured: Bound[];
  owner: Inst | undefined;
  at?: string;
}

/** A view held in a value: a call made as one, or calls. */
export function view(m: Kit, v: Value, p: string, e: Env, k: string): Drawn[] {
  if (v.kind === "opt") return v.value ? view(m, v.value, p, e, k) : [];
  if (v.kind === "list") return v.items.flatMap((i, n) => view(m, i.value, `${p}[${n}]`, e, `${k}[${i.id}]`));
  if (v.kind === "content") {
    const of = v.of as any;
    if ("parts" in of) return of.parts.flatMap((x: Value, n: number) => view(m, x, join(p, `j${n}`), e, join(k, `j${n}`)));
    return m.parts(of.nodes, { o: of.owner, b: of.captured, p: of.at?.path ?? p, k: of.at?.key ?? k });
  }
  if (v.kind !== "view") return [];
  const c = v as unknown as ViewValue;
  const who = c.at ?? k;
  const made: Env = { o: c.owner, b: c.captured, p, k: who };
  if ("v" in c.of) return useAt(m, { t: c.of, a: c.args, sl: c.slots, ev: c.events }, made, p, who);
  return externAt(m, { w: c.of, a: c.args, sl: c.slots, ev: c.events } as any, made, p, who);
}

// ---- extern widgets

export interface Extern extends Part {
  w: Widget;
  /** Its inputs, in order; a held view's are worked out already. */
  p?: [string, X][];
  a?: Map<string, Value>;
  /** Where `mut` inputs write: the place, and what a value of it has to fit. */
  wr?: [string, P, Contracts][];
  sl?: Record<string, Part[]>;
  ev?: Record<string, Handler>;
}

export const extern: Part_<Extern> = (m, n, e, out) => {
  out.push(...externAt(m, n, e, join(e.p, n.s), join(e.k, n.s)));
};

/** An extern widget's call: reused when it is the same call at the same place, with the same locals around it,
 * and nothing it read has changed. */
function externAt(m: Kit, n: Extern, e: Env, p: string, as: string): Drawn[] {
  const k = m.claim(as);
  const was = m.memo.get(`w|${k}`);
  if (was && was.n === n && was.epoch === m.epoch && was.p === p && was.o === e.o && m.unchanged(was.frame) && m.sameBound(was.b, e.b)) {
    m.replay(was.frame);
    return was.drawn;
  }
  const frame = m.enter();
  let drawn: Drawn[];
  try {
    drawn = externNow(m, n, e, p, k);
  } catch (f) {
    if (!m.contains(e)) throw f;
    drawn = [m.failed(n.w.n, p, k, f)];
  } finally {
    m.exit(frame);
  }
  if (!frame.dirty) m.memo.set(`w|${k}`, { frame, n, p, o: e.o, b: e.b, epoch: m.epoch, drawn });
  else m.memo.delete(`w|${k}`);
  return drawn;
}

function externNow(m: Kit, n: Extern, e: Env, p: string, k: string): Drawn[] {
  const w = n.w;
  const props = new Map<string, Value>(n.a ?? []);
  for (const [name, x] of n.p ?? []) props.set(name, x(m, e.b, e.o));
  let wr: Drawn["wr"] = NONE;
  for (const [name, f, c] of n.wr ?? []) {
    props.set(name, m.readPlace(f(m, e.b, e.o)));
    if (wr === NONE) wr = new Map();
    wr.set(name, { f, e, c });
  }
  for (const [name, x] of w.i ?? []) if (!props.has(name)) props.set(name, x(m, e.b, e.o));
  if (w.md) return m.parts(w.md, { o: externInst(m, w, p, props, n.sl ?? {}, e), b: [], p, k });
  // what it wrote to its own `mut` inputs stands until the caller gives them a variable
  if (w.mu) {
    m.track(`o:${k}`);
    m.back(p, (name) => (wr.has(name) ? undefined : w.mu!.find((f) => f[0] === name)?.[1]), (name, v) => {
      if (!m.own.has(k)) m.own.set(k, new Map());
      m.own.get(k)!.set(name, v);
    });
    const own = m.own.get(k);
    if (own) {
      m.ownSeen.add(k);
      (m.frames[m.frames.length - 1].own ??= []).push(k);
      for (const [name, v] of own) if (!wr.has(name)) props.set(name, v);
    }
  }
  let slots: Drawn["slots"] = NONE;
  for (const s in n.sl) {
    if (slots === NONE) slots = new Map();
    slots.set(s, m.parts(n.sl[s], e));
  }
  if (w.q) m.checkRules(w.q, externInst(m, w, p, props, n.sl ?? {}, e), p);
  m.inInputs++;
  try {
    inputs(m, w, props, p, k);
  } finally {
    m.inInputs--;
  }
  let ev: Drawn["ev"] = NONE;
  for (const name in n.ev) {
    if (ev === NONE) ev = new Map();
    ev.set(name, { h: n.ev[name], e });
  }
  return [{ p, k, w, props, slots, ev, wr }];
}

/** An extern widget's inputs and slots, for its rules and its Markdown. */
function externInst(m: Kit, w: Widget, p: string, props: Map<string, Value>, slots: Record<string, Part[]>, e: Env): Inst {
  return { id: m.newId(), k: `${p}#md`, t: w as any, i: new Map(props), pl: new Map(), c: new Map(), s: new Map(Object.entries(slots).map(([s, n]) => [s, { n, e }])), ev: new Map(), seen: m.draws };
}

/** The calls an input holds, drawn: a rich_text's and a view's, wherever in its value they are. */
function inputs(m: Kit, w: Widget, props: Map<string, Value>, path: string, key: string): void {
  const walk = (v: Value, at: string, as: string): Value => {
    switch (v.kind) {
      case "content":
      case "view":
        return { kind: "nodes", nodes: view(m, v, at, { o: undefined, b: [], p: at, k: at }, as) } as unknown as Value;
      case "list": {
        const items = v.items.map((i, k) => {
          const x = walk(i.value, `${at}[${k}]`, `${as}[${k}]`);
          return x === i.value ? i : { id: i.id, value: x };
        });
        return items.every((i, k) => i === v.items[k]) ? v : { ...v, items };
      }
      case "record": {
        const fields = v.fields.map(([f, x]) => [f, walk(x, `${at}.${f}`, `${as}.${f}`)] as const);
        return fields.every(([, x], k) => x === v.fields[k][1]) ? v : { ...v, fields };
      }
      case "case": {
        const values = v.values.map(([f, x]) => [f, walk(x, `${at}.${f}`, `${as}.${f}`)] as const);
        return values.every(([, x], k) => x === v.values[k][1]) ? v : { ...v, values };
      }
      case "opt": {
        const x = v.value && walk(v.value, at, as);
        return x === v.value ? v : { kind: "opt", value: x };
      }
      default:
        return v;
    }
  };
  const sentences = (v: Value): void => {
    if (v.kind === "list") v.items.forEach((i) => sentences(i.value));
    else checkSentence(v);
  };
  for (const [k, v] of props) {
    if (w.st?.includes(k)) sentences(v);
    const x = walk(v, join(path, k), join(key, k));
    if (x !== v) props.set(k, x);
  }
}

// ---- widgets written in Extended Mark

type Arg = [string, X] | [string, P, true];

export interface Use extends Part {
  t: Template;
  a?: Arg[];
  sl?: Record<string, Part[]>;
  ev?: Record<string, Handler>;
}

export const use: Part_<Use> = (m, n, e, out) => {
  out.push(...useAt(m, n, e, join(e.p, n.s), join(e.k, n.s)));
};

interface Call {
  t: Template;
  a?: Arg[] | Map<string, Value>;
  sl?: Record<string, Part[]>;
  ev?: Record<string, Handler>;
}

function sameRecord(a: Record<string, unknown> = {}, b: Record<string, unknown> = {}): boolean {
  if (a === b) return true;
  const ka = Object.keys(a);
  return ka.length === Object.keys(b).length && ka.every((k) => a[k] === b[k]);
}

const argValues = (a: Call["a"]): Value[] => (a instanceof Map ? [...a.keys()].sort().map((k) => a.get(k)!) : []);

function useAt(m: Kit, n: Call, e: Env, p: string, as: string): Drawn[] {
  const k = m.claim(as);
  // a failure working out whether it can be reused is left to drawing it
  try {
    const was = m.memo.get(`u|${k}`);
    if (was && was.epoch === m.epoch && was.t === n.t && was.p === p && was.o === e.o && sameRecord(was.sl, n.sl) && sameRecord(was.ev, n.ev) && m.unchanged(was.frame) && m.sameBound(was.b, e.b)) {
      const now = argValues(n.a);
      if (now.length === was.a.length && now.every((v, i) => m.equal(v, was.a[i]))) {
        m.replay(was.frame);
        return was.drawn;
      }
    }
  } catch {
    // drawn anew
  }
  const frame = m.enter();
  let drawn: Drawn[];
  try {
    const o = enterUse(m, n, e, p, k);
    drawn = m.parts(n.t.v, { o, b: [], p, k });
  } catch (f) {
    drawn = [m.failed(n.t.n, p, k, f)];
  } finally {
    m.exit(frame);
  }
  if (!frame.dirty && m.insts.has(k)) m.memo.set(`u|${k}`, { frame, t: n.t, p, o: e.o, a: argValues(n.a), b: e.b, sl: n.sl, ev: n.ev, epoch: m.epoch, drawn });
  else m.memo.delete(`u|${k}`);
  return drawn;
}

/** A widget's instance, made or kept, with its inputs, slots and events from this call. Its rules have to hold. */
function enterUse(m: Kit, n: Call, e: Env, p: string, k: string): Inst {
  const t = n.t;
  let it = m.insts.get(k)!;
  const fresh = !it || it.t !== t;
  if (fresh) {
    it = { id: m.newId(), k, t, i: new Map(), pl: new Map(), c: new Map(), s: new Map(), ev: new Map(), seen: m.draws };
    m.insts.set(k, it);
  }
  it.seen = m.draws;
  m.atPath.set(p, it);
  const top = m.frames[m.frames.length - 1];
  if (top) (top.nested ??= []).push([it, p]);
  const was = it.i;
  it.pl.clear();
  it.i = new Map();
  if (n.a instanceof Map) for (const [name, v] of n.a) it.i.set(name, v);
  else
    for (const a of n.a ?? []) {
      if (a[2]) it.pl.set(a[0], { f: a[1] as P, e });
      else it.i.set(a[0], (a[1] as X)(m, e.b, e.o));
    }
  for (const [name, x] of t.i ?? []) if (!it.i.has(name) && !it.pl.has(name)) it.i.set(name, x ? x(m, [], it) : none());
  // an input given a new value: whatever read it, in the widget's slots too, is out of date
  for (const [name, v] of it.i) {
    const before = was.get(name);
    if (!fresh && (!before || !m.equal(before, v))) m.bump(`i:${it.id}:${name}`);
  }
  const held = new Map(it.s);
  it.s.clear();
  for (const s in n.sl) it.s.set(s, { n: n.sl[s], e });
  if (!fresh) {
    for (const s of new Set([...held.keys(), ...it.s.keys()])) {
      const [a, b] = [held.get(s), it.s.get(s)];
      if (!(a && b && a.n === b.n && a.e.o === b.e.o && a.e.p === b.e.p && m.sameBound(a.e.b, b.e.b))) m.bump(`l:${it.id}:${s}`);
    }
  }
  it.ev.clear();
  for (const name in n.ev) it.ev.set(name, { h: n.ev[name], e });
  if (fresh) for (const [name, x] of t.c ?? []) it.c.set(name, x ? x(m, [], it) : none());
  m.back(p, (name) => t.c?.find((c) => c[0] === name)?.[2], (name, v) => it.c.set(name, v));
  if (t.q) m.checkRules(t.q, it, p);
  return it;
}

// ---- the calls a list slot or a widget's slot holds

/** Puts `callsIn` in a page that reads such a slot. Each kind of part says the calls it makes. */
export function withCalls(m: Kit): void {
  m.callsIn = (n, e) => {
    const out: Value[] = [];
    into(m, n, e.b, e.o, e.k, out);
    return out;
  };
}

function into(m: Kit, n: Part[], b0: Bound[], o: Inst | undefined, key: string, out: Value[]): void {
  let b = b0;
  for (const p of n as any[]) {
    const at = key ? `${key}/${p.s}` : p.s;
    if (p.d === extern || p.d === use) {
      const of = p.w ?? p.t;
      const given: [string, Value][] = (p.p ?? p.a ?? []).map((a: Arg) => [a[0], a[2] ? m.readPlace((a[1] as P)(m, b, o)) : (a[1] as X)(m, b, o)]);
      given.sort((x, y) => (x[0] < y[0] ? -1 : 1));
      out.push(m.view(callOf(p, of), given, b, o, at));
    } else if (p.d === loop) {
      for (const row of p.r(m, b, o) as Row[]) into(m, p.v, [...b, ...row.b], o, `${at}[${row.k}]`, out);
    } else if (p.d === branch) {
      const [parts, more] = p.f(m, b, o);
      into(m, parts, [...b, ...more], o, key, out);
    } else if (p.d === local) {
      b = [...b, p.x(m, b, o)];
    } else if (p.d === placed) {
      callsOf(m, p.x(m, b, o), at, out);
    }
  }
}

const calls = new WeakMap<object, { of: any }>();

/** A part as the call it is: the same one each time. */
function callOf(p: any, of: any): { of: any } {
  let c = calls.get(p);
  if (!c) calls.set(p, (c = { of, sl: p.sl, ev: p.ev } as { of: any }));
  return c;
}

function callsOf(m: Kit, v: Value, key: string, out: Value[]): void {
  if (v.kind === "opt") {
    if (v.value !== null) callsOf(m, v.value, key, out);
  } else if (v.kind === "list") v.items.forEach((i, k) => callsOf(m, i.value, `${key}[${k}]`, out));
  else if (v.kind === "view") out.push(v.at === undefined ? ({ ...v, at: key } as Value) : v);
  else if (v.kind === "content") {
    const of = v.of as any;
    if ("parts" in of) of.parts.forEach((p: Value, k: number) => callsOf(m, p, `${key}/j${k}`, out));
    else into(m, of.nodes, of.captured, of.owner, of.at?.key ?? key, out);
  }
}
