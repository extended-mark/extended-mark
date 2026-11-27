// What generated code calls besides the kit itself: values, places and statements, short.

import { Num } from "@extended-mark/runtime/num";
import { Failure, type Place } from "@extended-mark/runtime/ops";
import { asNum, bool, none, num as numOf, some, type Value } from "@extended-mark/runtime/values";
import { inside } from "./parts.js";
import type { Bound, Inst, Kit, T } from "./kit.js";

export { bool, none, some };

export const num = (s: string): Value => numOf(Num.literal(s)!);
export const int = (k: number): Value => numOf(k);
export const dur = (s: string): Value => ({ kind: "duration", n: Num.literal(s)! });
export const text = (s: string): Value => ({ kind: "text", s });
export const kase = (ty: string, name: string, values: (readonly [string, Value])[] = []): Value => ({ kind: "case", ty, name, values });
export const range = (low: Value | null, high: Value | null, step: Value | null): Value => ({ kind: "range", low, high, step });

// ---- places

export const state = (name: string): Place => ({ root: { kind: "outer", outer: { kind: "state", name }, owner: undefined }, path: [] });
export const cellAt = (o: Inst | undefined, name: string): Place => ({ root: { kind: "outer", outer: { kind: "cell", name }, owner: o as any }, path: [] });
export const inputAt = (o: Inst | undefined, name: string): Place => ({ root: { kind: "outer", outer: { kind: "input", name }, owner: o as any }, path: [] });

/** A local: the place it refers to, or its own. */
export function localAt(b: Bound[], at: number, name: string, t?: T): Place {
  const x = b[at];
  return "ref" in x ? x.ref : { root: { kind: "local", b, at, name, t } as any, path: [] };
}

export const at = (p: Place, ...path: Place["path"][number][]): Place => ({ root: p.root, path: [...p.path, ...path] });
export const field = (name: string): Place["path"][number] => ({ kind: "field", name });

/** An item by its position. */
export function item(v: Value): Place["path"][number] {
  const n = asNum(v)?.toI64() ?? null;
  if (n === null) throw new Failure("Position", "a position that isn't a whole number");
  return { kind: "index", index: n };
}

export const ref = (p: Place): Bound => ({ ref: p });

// ---- operators

export function neg(v: Value): Value {
  if (v.kind === "num" || v.kind === "duration") return { kind: v.kind, n: v.n.neg() };
  throw Failure.ir("`-` on something that isn't a number");
}

/** A text of its pieces, each value shown. */
export const shown = (m: Kit, ...parts: (string | Value)[]): Value => text(parts.map((p) => (typeof p === "string" ? p : m.show(p))).join(""));

/** A template of another type: pieces, and holes. */
export function template(m: Kit, pieces: string[], holes: Value[]): Value {
  const list = m.makeList(pieces.map(text));
  const held = m.makeList(holes.map((v) => {
    const view = v.kind === "view" || v.kind === "content";
    return kase("core/hole", view ? "VIEW" : "TEXT", [[view ? "child" : "value", view ? v : text(m.show(v))]]);
  }));
  return { kind: "record", ty: "core/template", fields: [["pieces", list], ["holes", held]] };
}

/** What an opt holds, or undefined: `if (let …)`. A plain value where an opt is wanted is always there. */
export const opened = (v: Value): Value | undefined => (v.kind === "opt" ? (v.value ?? undefined) : v);

/** The values a `for` walks. */
export function walked(m: Kit, over: Value): readonly Value[] {
  if (over.kind === "list") return over.items.map((i) => i.value);
  if (over.kind === "range") return m.walkRange(over);
  throw Failure.ir("`for` over something that isn't a list or a range");
}

/** The items a `mut for` walks: a reference to each. */
export function refs(m: Kit, list: Place): Bound[] {
  const v = m.readPlace(list);
  if (v.kind !== "list") throw Failure.ir("`mut for` over something that isn't a list");
  return v.items.map((i) => ref(at(list, { kind: "itemOpt", id: i.id })));
}

/** A `case` pattern: what the case or the opt holds, or undefined when it is another. */
export function isCase(v: Value, ty: string, name: string): Value[] | undefined {
  if (v.kind === "case") return v.ty === ty && v.name === name ? v.values.map((x) => x[1]) : undefined;
  if (v.kind === "opt") return name === "SOME" ? (v.value ? [v.value] : undefined) : name === "NONE" && !v.value ? [] : undefined;
  if (v.kind === "bool") return name === (v.b ? "TRUE" : "FALSE") ? [] : undefined;
  return undefined;
}

/** A view's `match`: an enum's case by its name alone. */
export function isCaseNamed(v: Value, name: string): Value[] | undefined {
  if (v.kind === "case") return v.name === name ? v.values.map((x) => x[1]) : undefined;
  if (v.kind === "opt") return name === "SOME" ? (v.value ? [v.value] : undefined) : name === "NONE" && !v.value ? [] : undefined;
  return undefined;
}

export const isNum = (v: Value, s: string): boolean => v.kind === "num" && Num.literal(s)!.equals(v.n);
export const isText = (v: Value, s: string): boolean => v.kind === "text" && v.s === s;
export const isBool = (v: Value, b: boolean): boolean => v.kind === "bool" && v.b === b;

/** Whether a value is a sentence's rich_text. */
export const sentence = (v: Value): Value => bool(!(v.kind === "content" && v.blocks));

/** `if (let x = …)`: what an opt holds, bound. */
export function open(m: Kit, b: Bound[], at: number, v: Value, t?: T): boolean {
  const x = opened(v);
  // with a contract it is a proof as well: it opens only when the value fits
  if (x === undefined || (t && !m.fits(x, t))) return false;
  b[at] = x;
  return true;
}

/** `if (mut x = …)`: the place inside an opt, bound. */
export function openMut(m: Kit, b: Bound[], at: number, p: Place, t?: T): boolean {
  const x = inside(m, p);
  if (x === undefined || (t && !m.fits(m.readPlace(x), t))) return false;
  b[at] = ref(x);
  return true;
}

/** What a pattern's case holds, bound. */
export function binds(b: Bound[], at: number, n: number, held: Value[] | undefined): boolean {
  if (!held) return false;
  for (let k = 0; k < n; k++) b[at + k] = held[k] ?? none();
  return true;
}

/** A case's values, in the order it declares them. */
export const order = <X>(xs: X[], by: number[]): X[] => by.map((k) => xs[k]);

/** `try` of a case: none when a value it holds isn't inside its type's contract. */
export function fitting(m: Kit, v: Value, types: T[]): Value {
  if (v.kind === "case") for (let k = 0; k < Math.min(v.values.length, types.length); k++) if (!m.fits(v.values[k][1], types[k])) return none();
  return some(v);
}
