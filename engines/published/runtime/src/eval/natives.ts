// The native fns: what core declares `extern fn` with no body, because Extended Mark can't do it in terms of
// anything else (page 5). List storage, Unicode text, exact numbers and their maths, the interface
// fns of the built-in types, a range's ends, a duration's milliseconds, and `clamp`. A port of
// compiler/eval/src/natives.rs.
// Each is a fn of its own, so what holds a page holds the ones it calls.

import { Num } from "./num.js";
import * as V from "./value.js";
import type { Item, ListValue, Value } from "./value.js";
import { Failure, MAX_ITEMS, numOp, type Binding, type Ops } from "./ops.js";
import { chars, digits, showDuration, sliceChars, textLength, trim } from "./text.js";

/** A native fn: its value, or `undefined` when it returns nothing (the list fns that change their list). */
export type Native = (m: Ops, a: readonly Binding[]) => Value | undefined;

/** core's bodyless `extern fn`s that only an engine can supply, from what it runs on (page 5,
 * plan tab 14): every other bodyless one is a native. */
export const ENGINE_ONLY: readonly string[] = ["core/moment.local"];

// ---- arguments

function num(m: Ops, args: readonly Binding[], k: number): Num {
  const n = V.asNum(m.valueOf(args[k]));
  if (n === undefined) {
    throw Failure.ir("a native fn given something that isn't a number");
  }
  return n;
}

function text(m: Ops, args: readonly Binding[], k: number): string {
  const v = m.valueOf(args[k]);
  if (v.kind !== "text") {
    throw Failure.ir("a native fn given something that isn't a text");
  }
  return v.s;
}

function list(m: Ops, args: readonly Binding[], k: number): ListValue {
  const v = m.valueOf(args[k]);
  if (v.kind !== "list") {
    throw Failure.ir("a native fn given something that isn't a list");
  }
  return v;
}

function whole(n: Num, what: string): number {
  const k = n.toI64();
  if (k === null) {
    throw new Failure("Position", `${what} that isn't a whole number`);
  }
  return k;
}

/** Writes a changed list back through `mut self`. */
function setList(m: Ops, args: readonly Binding[], items: Item[]): void {
  if (items.length > MAX_ITEMS) {
    throw Failure.ofLimit("Items", "a list of more than 10,000 items");
  }
  const self = args[0];
  if (!("ref" in self)) {
    throw Failure.ir("a list fn that changes its list, given a list it can't write");
  }
  m.writePlace(self.ref, { kind: "list", items });
}

function someNum(r: Num | null): Value {
  return r === null ? V.none() : V.some(V.num(r));
}

// ---- the fns

export const number_show: Native = (m, a) => {
  return V.text(num(m, a, 0).show());
};

export const number_same: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.bool(x.equals(num(m, a, 1)));
};

export const duration_same: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.bool(x.equals(num(m, a, 1)));
};

export const number_before: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.bool(x.lt(num(m, a, 1)));
};

export const duration_before: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.bool(x.lt(num(m, a, 1)));
};

export const number_add: Native = (m, a) => {
  const x = num(m, a, 0);
  const y = num(m, a, 1);
  return V.num(numOp(() => x.add(y)));
};

export const number_sub: Native = (m, a) => {
  const x = num(m, a, 0);
  const y = num(m, a, 1);
  return V.num(numOp(() => x.sub(y)));
};

export const number_scale: Native = (m, a) => {
  const x = num(m, a, 0);
  const y = num(m, a, 1);
  return V.num(numOp(() => x.mul(y)));
};

export const duration_show: Native = (m, a) => {
  return V.text(showDuration(num(m, a, 0)));
};

export const duration_add: Native = (m, a) => {
  const x = num(m, a, 0);
  const y = num(m, a, 1);
  return V.duration(numOp(() => x.add(y)));
};

export const duration_sub: Native = (m, a) => {
  const x = num(m, a, 0);
  const y = num(m, a, 1);
  return V.duration(numOp(() => x.sub(y)));
};

export const duration_scale: Native = (m, a) => {
  const x = num(m, a, 0);
  const y = num(m, a, 1);
  return V.duration(numOp(() => x.mul(y)));
};

export const duration_ms: Native = (m, a) => {
  return V.num(num(m, a, 0));
};

export const text_show: Native = (m, a) => {
  return V.text(text(m, a, 0));
};

export const text_same: Native = (m, a) => {
  const x = text(m, a, 0);
  return V.bool(x === text(m, a, 1));
};

export const text_before: Native = (m, a) => {
  const x = text(m, a, 0);
  return V.bool(V.compareText(x, text(m, a, 1)) < 0);
};

export const bool_show: Native = (m, a) => {
  return V.text(V.asBool(m.valueOf(a[0])) ?? false ? "true" : "false");
};

export const bool_same: Native = (m, a) => {
  const x = V.asBool(m.valueOf(a[0]));
  return V.bool(x === V.asBool(m.valueOf(a[1])));
};

export const bool_before: Native = (m, a) => {
  const x = V.asBool(m.valueOf(a[0])) ?? false;
  return V.bool(!x && (V.asBool(m.valueOf(a[1])) ?? false));
};

export const range_low: Native = (m, a) => {
  const r = m.valueOf(a[0]);
  if (r.kind !== "range") {
    throw Failure.ir("a range fn given something that isn't a range");
  }
  const part = r.low;
  return part === null ? V.none() : V.some(part);
};

export const range_high: Native = (m, a) => {
  const r = m.valueOf(a[0]);
  if (r.kind !== "range") {
    throw Failure.ir("a range fn given something that isn't a range");
  }
  const part = r.high;
  return part === null ? V.none() : V.some(part);
};

export const range_stride: Native = (m, a) => {
  const r = m.valueOf(a[0]);
  if (r.kind !== "range") {
    throw Failure.ir("a range fn given something that isn't a range");
  }
  const part = r.step;
  return part === null ? V.none() : V.some(part);
};

export const range_has: Native = (m, a) => {
  const r = m.valueOf(a[0]);
  if (r.kind !== "range") {
    throw Failure.ir("a range fn given something that isn't a range");
  }
  const x = m.valueOf(a[1]);
  if (r.low !== null && m.before(x, r.low)) {
    return V.bool(false);
  }
  if (r.high !== null && m.before(r.high, x)) {
    return V.bool(false);
  }
  const step = r.step !== null ? V.asNum(r.step) : undefined;
  const xn = V.asNum(x);
  if (step !== undefined && xn !== undefined) {
    const from = (r.low !== null ? V.asNum(r.low) : undefined) ?? Num.zero();
    if (!numOp(() => xn.sub(from).div(step)).isInt()) {
      return V.bool(false);
    }
  }
  return V.bool(true);
};

export const number_round: Native = (m, a) => {
  return V.num(num(m, a, 0).round());
};

export const number_floor: Native = (m, a) => {
  return V.num(num(m, a, 0).floor());
};

export const number_ceil: Native = (m, a) => {
  return V.num(num(m, a, 0).ceil());
};

export const number_abs: Native = (m, a) => {
  return V.num(num(m, a, 0).abs());
};

export const number_sign: Native = (m, a) => {
  return V.num(num(m, a, 0).sign());
};

export const number_fmt: Native = (m, a) => {
  const places = Math.min(Math.max(whole(num(m, a, 1), "a number of places"), 0), 10);
  return V.text(num(m, a, 0).fmt(places));
};

export const number_sqrt: Native = (m, a) => {
  return someNum(num(m, a, 0).sqrt());
};

export const number_pow: Native = (m, a) => {
  const x = num(m, a, 0);
  const p = num(m, a, 1);
  return someNum(numOp(() => x.pow(p)));
};

export const number_exp: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.num(numOp(() => x.exp()));
};

export const number_ln: Native = (m, a) => {
  return someNum(num(m, a, 0).ln());
};

export const number_sin: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.num(numOp(() => x.sin()));
};

export const number_cos: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.num(numOp(() => x.cos()));
};

export const number_tan: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.num(numOp(() => x.tan()));
};

export const number_asin: Native = (m, a) => {
  return someNum(num(m, a, 0).asin());
};

export const number_acos: Native = (m, a) => {
  return someNum(num(m, a, 0).acos());
};

export const number_atan: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.num(numOp(() => x.atan()));
};

export const number_atan2: Native = (m, a) => {
  const y = num(m, a, 0);
  const x = num(m, a, 1);
  return V.num(numOp(() => y.atan2(x)));
};

export const number_sinh: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.num(numOp(() => x.sinh()));
};

export const number_cosh: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.num(numOp(() => x.cosh()));
};

export const number_tanh: Native = (m, a) => {
  const x = num(m, a, 0);
  return V.num(numOp(() => x.tanh()));
};

export const list_len: Native = (m, a) => {
  return V.num(Num.int(list(m, a, 0).items.length));
};

export const list_at: Native = (m, a) => {
  const l = list(m, a, 0);
  const k = num(m, a, 1).toI64();
  return k !== null && k >= 0 && k < l.items.length ? V.some(l.items[k].value) : V.none();
};

export const list_add: Native = (m, a) => {
  const items = list(m, a, 0).items.slice();
  const value = m.valueOf(a[1]);
  const id = m.newId();
  items.push({ id, value });
  setList(m, a, items);
  return undefined;
};

export const list_insert: Native = (m, a) => {
  const items = list(m, a, 0).items.slice();
  const pos = Math.max(whole(num(m, a, 1), "a position"), 0);
  const value = m.valueOf(a[2]);
  const id = m.newId();
  items.splice(Math.min(pos, items.length), 0, { id, value });
  setList(m, a, items);
  return undefined;
};

export const list_remove_at: Native = (m, a) => {
  const items = list(m, a, 0).items.slice();
  const pos = whole(num(m, a, 1), "a position");
  if (pos < 0 || pos >= items.length) {
    throw new Failure("Position", "remove_at with a position outside the list");
  }
  items.splice(pos, 1);
  setList(m, a, items);
  return undefined;
};

export const list_move: Native = (m, a) => {
  const items = list(m, a, 0).items.slice();
  const pos = whole(num(m, a, 1), "a position");
  const to = whole(num(m, a, 2), "a position");
  const n = items.length;
  if (pos < 0 || pos >= n || to < 0 || to >= n) {
    throw new Failure("Position", "move with a position outside the list");
  }
  const [item] = items.splice(pos, 1);
  items.splice(to, 0, item);
  setList(m, a, items);
  return undefined;
};

export const list_remove: Native = (m, a) => {
  const l = list(m, a, 0);
  const x = m.valueOf(a[1]);
  const kept: Item[] = [];
  for (const i of l.items) {
    if (!m.equal(i.value, x)) {
      kept.push(i);
    }
  }
  setList(m, a, kept);
  return undefined;
};

export const list_sort: Native = (m, a) => {
  const l = list(m, a, 0);
  // a stable insertion of each item after every one that isn't after it
  const out: Item[] = [];
  for (const item of l.items) {
    let at = out.length;
    while (at > 0 && m.before(item.value, out[at - 1].value)) {
      at -= 1;
    }
    out.splice(at, 0, item);
  }
  setList(m, a, out);
  return undefined;
};

export const text_len: Native = (m, a) => {
  return V.num(Num.int(textLength(text(m, a, 0))));
};

export const text_slice: Native = (m, a) => {
  const t = text(m, a, 0);
  const start = Math.max(whole(num(m, a, 1), "a start"), 0);
  const count = Math.max(whole(num(m, a, 2), "a count"), 0);
  return V.text(sliceChars(t, start, count));
};

export const text_chars: Native = (m, a) => {
  const t = text(m, a, 0);
  return m.makeList(chars(t).map((c) => V.text(c)));
};

export const text_lower: Native = (m, a) => {
  return V.text(text(m, a, 0).toLowerCase());
};

export const text_upper: Native = (m, a) => {
  return V.text(text(m, a, 0).toUpperCase());
};

export const text_trim: Native = (m, a) => {
  return V.text(trim(text(m, a, 0)));
};

export const text_has: Native = (m, a) => {
  const t = text(m, a, 0);
  return V.bool(t.includes(text(m, a, 1)));
};

export const text_split: Native = (m, a) => {
  const t = text(m, a, 0);
  const sep = text(m, a, 1);
  if (sep.length === 0) {
    throw Failure.rule("split takes a separator of at least one character");
  }
  return m.makeList(t.split(sep).map((p) => V.text(p)));
};

export const text_number: Native = (m, a) => {
  const t = text(m, a, 0);
  const body = t.startsWith("-") ? t.slice(1) : t;
  const dot = body.indexOf(".");
  const w = dot < 0 ? body : body.slice(0, dot);
  const f = dot < 0 ? null : body.slice(dot + 1);
  if (digits(w) && (f === null || digits(f))) {
    return someNum(Num.parse(t));
  }
  return V.none();
};

export const text_of_code: Native = (m, a) => {
  const k = num(m, a, 0).toI64();
  if (k === null || k < 0 || k > 0x10ffff || (k >= 0xd800 && k <= 0xdfff)) {
    return V.none();
  }
  return V.some(V.text(String.fromCodePoint(k)));
};

export const md_parts: Native = (m, a) => {
  const parts = list(m, a, 0).items.map((i) => i.value);
  return { kind: "content", of: { parts }, blocks: parts.some((p) => p.kind === "content" && p.blocks) };
};

export const md_blocks: Native = (m, a) => {
  return { kind: "content", of: { parts: [m.valueOf(a[0])] }, blocks: true };
};

export const ordered_clamp: Native = (m, a) => {
  const x = m.valueOf(a[0]);
  const low = m.valueOf(a[1]);
  const high = m.valueOf(a[2]);
  if (m.before(x, low)) {
    return low;
  }
  if (m.before(high, x)) {
    return high;
  }
  return x;
};

/** Every native fn, by its full name. */
const ALL: ReadonlyMap<string, Native> = new Map<string, Native>([
  ["core/number.show", number_show],
  ["core/number.same", number_same],
  ["core/duration.same", duration_same],
  ["core/number.before", number_before],
  ["core/duration.before", duration_before],
  ["core/number.add", number_add],
  ["core/number.sub", number_sub],
  ["core/number.scale", number_scale],
  ["core/duration.show", duration_show],
  ["core/duration.add", duration_add],
  ["core/duration.sub", duration_sub],
  ["core/duration.scale", duration_scale],
  ["core/duration.ms", duration_ms],
  ["core/text.show", text_show],
  ["core/text.same", text_same],
  ["core/text.before", text_before],
  ["core/bool.show", bool_show],
  ["core/bool.same", bool_same],
  ["core/bool.before", bool_before],
  ["core/range.low", range_low],
  ["core/range.high", range_high],
  ["core/range.stride", range_stride],
  ["core/range.has", range_has],
  ["core/number.round", number_round],
  ["core/number.floor", number_floor],
  ["core/number.ceil", number_ceil],
  ["core/number.abs", number_abs],
  ["core/number.sign", number_sign],
  ["core/number.fmt", number_fmt],
  ["core/number.sqrt", number_sqrt],
  ["core/number.pow", number_pow],
  ["core/number.exp", number_exp],
  ["core/number.ln", number_ln],
  ["core/number.sin", number_sin],
  ["core/number.cos", number_cos],
  ["core/number.tan", number_tan],
  ["core/number.asin", number_asin],
  ["core/number.acos", number_acos],
  ["core/number.atan", number_atan],
  ["core/number.atan2", number_atan2],
  ["core/number.sinh", number_sinh],
  ["core/number.cosh", number_cosh],
  ["core/number.tanh", number_tanh],
  ["core/list.len", list_len],
  ["core/list.at", list_at],
  ["core/list.add", list_add],
  ["core/list.insert", list_insert],
  ["core/list.remove_at", list_remove_at],
  ["core/list.move", list_move],
  ["core/list.remove", list_remove],
  ["core/list.sort", list_sort],
  ["core/text.len", text_len],
  ["core/text.slice", text_slice],
  ["core/text.chars", text_chars],
  ["core/text.lower", text_lower],
  ["core/text.upper", text_upper],
  ["core/text.trim", text_trim],
  ["core/text.has", text_has],
  ["core/text.split", text_split],
  ["core/text.number", text_number],
  ["core/text.of_code", text_of_code],
  ["core/md_parts", md_parts],
  ["core/md_blocks", md_blocks],
  ["core/ordered.clamp", ordered_clamp],
]);

export const NATIVES: readonly string[] = [...ALL.keys()];

export function exists(name: string): boolean {
  return ALL.has(name);
}

/** Runs the native fn `name` (one `exists` names). */
export function run(m: Ops, name: string, a: readonly Binding[]): Value | undefined {
  const f = ALL.get(name);
  if (f === undefined) throw Failure.ir(`no native \`${name}\``);
  return f(m, a);
}
