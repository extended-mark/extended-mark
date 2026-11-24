// Reading a node's inputs for drawing. Every input arrives with its default filled in, so these only
// unwrap: an opt to its value, a case to its name, a record to its fields.

import { Num } from "@extended-mark/runtime/num";
import type { Input, Node } from "@extended-mark/runtime";

/** The value inside an opt (and inside that one), or the value itself; undefined for none. */
export function inner(v: Input | undefined): Input | undefined {
  while (v && "some" in v) v = v.some;
  return v && "none" in v ? undefined : v;
}

/** A record's field, or a case's value, by name. */
export function field(v: Input | undefined, name: string): Input | undefined {
  v = inner(v);
  if (!v) return undefined;
  if ("record" in v) return v.record.fields[name];
  return "case" in v ? v.case.values?.[name] : undefined;
}

/** A value as the words it shows: a text, a number in its exact form, a bool. */
export function str(v: Input | undefined): string {
  v = inner(v);
  if (!v) return "";
  if ("text" in v) return v.text;
  if ("num" in v) return v.num;
  if ("duration" in v) return v.duration;
  return "bool" in v ? String(v.bool) : "";
}

/** A number as JS reads it, for drawing only: writes go back as exact Nums. */
export function num(v: Input | undefined): number | undefined {
  v = inner(v);
  if (!v) return undefined;
  return "num" in v ? Number(v.num) : "duration" in v ? Number(v.duration) : undefined;
}

export function exact(v: Input | undefined): Num | undefined {
  v = inner(v);
  return v && "num" in v ? Num.parse(v.num) ?? undefined : undefined;
}

export function flag(v: Input | undefined, otherwise = false): boolean {
  v = inner(v);
  return v && "bool" in v ? v.bool : otherwise;
}

/** An enum's or a token's case name: `START`, `ACCENT`. */
export function kase(v: Input | undefined): string | undefined {
  v = inner(v);
  if (!v) return undefined;
  if ("case" in v) return v.case.name;
  if ("token" in v) return v.token.slice(v.token.lastIndexOf(".") + 1);
  return "tokval" in v ? v.tokval.name : undefined;
}

export function list(v: Input | undefined): Input[] {
  v = inner(v);
  return v && "list" in v ? v.list : [];
}

/** A range's ends and its step. */
export function range(v: Input | undefined): { low?: Input; high?: Input; step?: Input } | undefined {
  v = inner(v);
  return v && "range" in v ? v.range : undefined;
}

export interface XY {
  x: number;
  y: number;
}

export function point(v: Input | undefined): XY | undefined {
  const x = num(field(v, "x"));
  const y = num(field(v, "y"));
  return x === undefined || y === undefined ? undefined : { x, y };
}

export function region(v: Input | undefined): { from: XY; to: XY } | undefined {
  const from = point(field(v, "from"));
  const to = point(field(v, "to"));
  return from && to ? { from, to } : undefined;
}

/** An exact point, to give the page. */
export function pointInput(p: { x: Num; y: Num }): Input {
  return { record: { type: "core/point", fields: { x: { num: p.x.show() }, y: { num: p.y.show() } } } };
}

/** An exact region the page gave. */
export function regionExact(v: Input | undefined): { from: { x: Num; y: Num }; to: { x: Num; y: Num } } | undefined {
  const p = (w: Input | undefined) => {
    const [x, y] = [exact(field(w, "x")), exact(field(w, "y"))];
    return x && y ? { x, y } : undefined;
  };
  const [from, to] = [p(field(v, "from")), p(field(v, "to"))];
  return from && to ? { from, to } : undefined;
}

/** A value's identity for "did it change". A fn's is its version. */
export function sig(v: Input | undefined): string {
  return v ? JSON.stringify(v) : "";
}

/** The widgets an input holds, a slot's, a rich_text's or a view's, in order. */
export function nodes(v: Input | undefined): readonly Node[] {
  v = inner(v);
  return v && "nodes" in v ? v.nodes : [];
}

/** A JS number as an exact Num, rounded to `places` digits after the point: pointer maths is
 * floating point, and a write should be a number a person would write. */
export function toNum(x: number, places = 2): Num {
  if (!Number.isFinite(x)) return Num.zero();
  const p = Math.max(0, Math.min(12, places));
  return Num.parse(x.toFixed(p)) ?? Num.zero();
}

/** How many digits after the point keep a sub-pixel's worth, for `units` units per pixel. */
export function placesFor(unitsPerPx: number): number {
  if (!(unitsPerPx > 0) || !Number.isFinite(unitsPerPx)) return 2;
  return Math.max(0, Math.ceil(-Math.log10(unitsPerPx)) + 1);
}
