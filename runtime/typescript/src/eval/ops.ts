// What an evaluator does to values, whatever works expressions out: equality, order, how a value is shown,
// records and their rules, contracts, places in a value, and the limits of page 6. The interpreter (interp.ts)
// is made on it, and so is what an engine writes a page as: it walks no IR.

import type * as ir from "../generated/ir.js";
import { Num, NumError, type NumErrorKind } from "./num.js";
import * as V from "./value.js";
import type { CaseValue, Entry, Item, RecordValue, Value } from "./value.js";
import * as text from "./text.js";

/** Page 6's limits. */
export const MAX_STEPS = 1_000_000;
export const MAX_ITEMS = 10_000;
export const MAX_DEPTH = 64;

export type Limit = "Steps" | "Items" | "Depth";

/** interp.rs's FailKind, by its variants' names. `Number` and `Limit` say which in `num` and `limit`. */
export type FailKind =
  /** A number that doesn't fit: a division by zero, or a result too large. */
  | "Number"
  /** A `require` or a contract, false for values that are not constants. */
  | "Rule"
  /** A position outside a list. */
  | "Position"
  /** An item used after it was removed. */
  | "Removed"
  | "Limit"
  /** What is only known when the page runs: a variable, `$state`, an extern nobody supplies. */
  | "NotConstant"
  /** A bug in the IR: a name that isn't declared, a value of the wrong kind. */
  | "Ir";

/** Which number failure or which limit, for kinds `Number` and `Limit`. */
export interface FailureDetail {
  num?: NumErrorKind;
  limit?: Limit;
}

export class Failure extends Error {
  readonly kind: FailKind;
  /** A `require`'s message, when a rule failed. */
  readonly rule?: string;
  /** For `Number`: a division by zero, or a result too large. */
  readonly num?: NumErrorKind;
  /** For `Limit`: which one. */
  readonly limit?: Limit;

  /** As Rust's `Failure { kind, message, rule }`. */
  constructor(kind: FailKind, message: string, rule?: string, detail: FailureDetail = {}) {
    super(message);
    this.name = "Failure";
    this.kind = kind;
    if (rule !== undefined) {
      this.rule = rule;
    }
    if (detail.num !== undefined) {
      this.num = detail.num;
    }
    if (detail.limit !== undefined) {
      this.limit = detail.limit;
    }
  }

  static ir(message: string): Failure {
    return new Failure("Ir", message);
  }

  static rule(message: string): Failure {
    return new Failure("Rule", `broken rule: ${message}`, message);
  }

  static ofNum(e: NumError): Failure {
    return new Failure("Number", e.message, undefined, { num: e.kind });
  }

  static ofLimit(limit: Limit, message: string): Failure {
    return new Failure("Limit", message, undefined, { limit });
  }
}

/** Runs number maths, a NumError becoming a Failure of kind Number, as Rust's `?` on a NumResult. */
export function numOp<T>(f: () => T): T {
  try {
    return f();
  } catch (e) {
    if (e instanceof NumError) {
      throw Failure.ofNum(e);
    }
    throw e;
  }
}

export type OuterKind = "cell" | "input" | "state" | "let" | "extern" | "slot";

/** What a name outside a fn's locals stands for. */
export interface Outer {
  readonly kind: OuterKind;
  readonly name: string;
}

const OUTER_DEBUG: { [K in OuterKind]: string } = { cell: "Cell", input: "Input", state: "State", let: "Let", extern: "Extern", slot: "Slot" };

/** An Outer as Rust's `{:?}` writes it, for messages: `Cell("n")`. */
export function outerDebug(o: Outer): string {
  return `${OUTER_DEBUG[o.kind]}(${JSON.stringify(o.name)})`;
}

/** A local, or a reference to a place for a `mut` parameter, a `mut for` or an `if (mut …)`. */
/** A `mut` local keeps its type, for the writes of the fns it is given to. */
export type Binding = { readonly value: Value; readonly type?: ir.Type } | { readonly ref: Place };

export interface Place {
  readonly root: PlaceRoot;
  readonly path: readonly Step[];
}

export type PlaceRoot =
  | { readonly kind: "local"; readonly frame: number; readonly name: string }
  | { readonly kind: "outer"; readonly outer: Outer; readonly owner: number | undefined };

export type Step =
  | { readonly kind: "field"; readonly name: string }
  /** An item by its position, as `xs[i] = v` writes. */
  | { readonly kind: "index"; readonly index: number }
  /** The real item, by its identity, wherever it now is. */
  | { readonly kind: "item"; readonly id: number }
  /** The real item as an `opt`: `none` once it has been removed (a `mut for` variable). */
  | { readonly kind: "itemOpt"; readonly id: number }
  /** The value inside an `opt`. */
  | { readonly kind: "inside" };

const INSIDE: Step = { kind: "inside" };

function short(ty: string): string {
  const k = ty.lastIndexOf("/");
  return k < 0 ? ty : ty.slice(k + 1);
}

const OP_DEBUG: { [K in ir.OpName]: string } = {
  "+": "Plus",
  "-": "Minus",
  "*": "Times",
  "/": "Divide",
  "%": "Rem",
  "==": "Eq",
  "!=": "Ne",
  "<": "Lt",
  "<=": "Le",
  ">": "Gt",
  ">=": "Ge",
  "&&": "And",
  "||": "Or",
  "!": "Not",
  "?": "Pick",
};

const SMALL: Num[] = [];

/** A position or a count as a number. */
export function int(k: number): Num {
  if (k >= 0 && k < 1024) {
    let n = SMALL[k];
    if (n === undefined) {
      n = Num.int(k);
      SMALL[k] = n;
    }
    return n;
  }
  return Num.int(k);
}

const EMPTY: readonly never[] = [];

/** The declarations values are made and checked by, by their full names. */
export interface Declared {
  readonly records: ReadonlyMap<string, ir.Record>;
  readonly enums: ReadonlyMap<string, ir.Enum>;
  readonly tokens: ReadonlyMap<string, ir.Token>;
  readonly aliases: ReadonlyMap<string, ir.Type>;
}

/** An expression a declaration holds (a default, a contract's end, a token's value) and a rule's body are whatever
 * the evaluator makes of them: `work` and `rule` are given what the declarations have. */
export abstract class Ops {
  steps = 0;
  depth = 0;
  nextId = 0;
  /** The message of the last rule a `try` turned into `none`, for a literal's error. */
  lastRule: string | undefined = undefined;
  /** The messages of the records' `warn` rules found false since someone last took them. */
  warned: string[] = [];

  abstract readonly prog: Declared;
  /** An expression of a declaration, worked out. */
  protected abstract work(e: ir.Expr): Value;
  /** Whether a record keeps one of its rules. */
  protected abstract rule(r: ir.Require, record: RecordValue): Value | undefined;
  /** Calls a fn by its full name: `undefined` when it returns nothing. */
  abstract call(name: string, args: readonly Binding[]): Value | undefined;
  /** Whether there is a fn of a full name. */
  protected abstract knows(name: string): boolean;
  protected abstract readRoot(root: PlaceRoot): Value;
  protected abstract writeRoot(root: PlaceRoot, value: Value): void;

  /** The value a token case stands for: the build's own, unless the engine has others. */
  protected tokenValue(c: CaseValue): Value {
    return this.defaultTokenValue(c);
  }

  /** A part of a value outside, at a place in it. */
  protected partOf(p: Place): Value {
    return this.valueAt(p);
  }

  /** Starts counting steps again, for one redraw, one event or one constant. */
  resetSteps(): void {
    this.steps = 0;
  }

  step(): void {
    this.steps += 1;
    if (this.steps > MAX_STEPS) {
      throw Failure.ofLimit("Steps", "more than 1,000,000 steps");
    }
  }

  newId(): number {
    this.nextId += 1;
    return this.nextId;
  }

  makeList(values: readonly Value[]): Value {
    if (values.length > MAX_ITEMS) {
      throw Failure.ofLimit("Items", "a list of more than 10,000 items");
    }
    const ids = { next: this.nextId };
    const v = V.list(values, ids);
    this.nextId = ids.next;
    return v;
  }

  readPlace(p: Place): Value {
    return p.path.length > 0 && p.root.kind === "outer" ? this.partOf(p) : this.valueAt(p);
  }

  valueAt(p: Place): Value {
    let v = this.readRoot(p.root);
    for (const s of p.path) {
      switch (s.kind) {
        case "field": {
          if (v.kind === "record") {
            const x = V.entry(v.fields, s.name);
            if (x === undefined) {
              throw Failure.ir(`no field \`${s.name}\``);
            }
            v = x;
            continue;
          }
          break;
        }
        case "index": {
          if (v.kind === "list") {
            const item = s.index >= 0 ? v.items[s.index] : undefined;
            if (item === undefined) {
              throw new Failure("Position", "a position outside the list");
            }
            v = item.value;
            continue;
          }
          break;
        }
        case "item": {
          if (v.kind === "list") {
            const item = findItem(v.items, s.id);
            if (item === undefined) {
              throw new Failure("Removed", "an item used after it was removed");
            }
            v = item.value;
            continue;
          }
          break;
        }
        case "itemOpt": {
          if (v.kind === "list") {
            const item = findItem(v.items, s.id);
            v = item === undefined ? V.none() : V.some(item.value);
            continue;
          }
          break;
        }
        case "inside": {
          if (v.kind === "opt") {
            if (v.value === null) {
              throw new Failure("Removed", "an opt that is empty");
            }
            v = v.value;
            continue;
          }
          break;
        }
      }
      throw Failure.ir("a place that doesn't fit its value");
    }
    return v;
  }

  writePlace(p: Place, value: Value): void {
    if (p.path.length === 0) {
      this.writeRoot(p.root, value);
      return;
    }
    const whole = writePath(this.readRoot(p.root), p.path, 0, value);
    this.checkAlong(whole, p.path, 0);
    this.writeRoot(p.root, whole);
  }

  /** The records a write went through, from the inside out: the field written keeps its contract, and
   * each record its rules (page 2, every change is checked). */
  private checkAlong(v: Value, path: readonly Step[], k: number): void {
    if (k === path.length) return;
    const s = path[k];
    let inside: Value | undefined;
    if (s.kind === "field") inside = v.kind === "record" ? V.entry(v.fields, s.name) : undefined;
    else if (s.kind === "inside") inside = v.kind === "opt" ? (v.value ?? undefined) : undefined;
    else if (v.kind === "list") inside = (s.kind === "index" ? v.items[s.index] : v.items.find((i) => i.id === s.id))?.value;
    if (inside === undefined) return;
    this.checkAlong(inside, path, k + 1);
    if (s.kind !== "field" || v.kind !== "record") return;
    const decl = this.prog.records.get(v.ty);
    const f = decl?.fields.find((f) => f.name === s.name);
    if (decl === undefined || f === undefined) return;
    if (!this.fits(inside, f.type)) {
      throw new Failure("Rule", `${short(v.ty)}'s ${f.name} is outside its contract`);
    }
    this.checkRequires(decl, v);
  }

  /** The numbers a range walks: from its low end to its high one, by its step (1 when none). */
  walkRange(r: { readonly low: Value | null; readonly high: Value | null; readonly step: Value | null }): Value[] {
    const low = r.low !== null ? V.asNum(r.low) : undefined;
    if (low === undefined) {
      throw Failure.ir("a range with no start can't be walked");
    }
    const step = (r.step !== null ? V.asNum(r.step) : undefined) ?? int(1);
    if (step.isZero()) {
      throw Failure.ofLimit("Items", "a range walked by a step of 0");
    }
    const high = r.high !== null ? V.asNum(r.high) : undefined;
    const down = step.isNegative();
    const out: Value[] = [];
    let x = low;
    for (;;) {
      if (high !== undefined && ((!down && x.gt(high)) || (down && x.lt(high)))) {
        break;
      }
      if (out.length >= MAX_ITEMS) {
        throw Failure.ofLimit("Items", "a range of more than 10,000 items");
      }
      out.push(V.num(x));
      const from = x;
      x = numOp(() => from.add(step));
    }
    return out;
  }

  field(v: Value, name: string): Value {
    if (v.kind === "record") {
      const x = V.entry(v.fields, name);
      if (x === undefined) {
        throw Failure.ir(`\`${v.ty}\` has no field \`${name}\``);
      }
      return x;
    }
    if (v.kind === "view") {
      const x = v.args.get(name);
      if (x === undefined) {
        throw Failure.ir(`the view has no input \`${name}\``);
      }
      return x;
    }
    // a token's record value lets its fields through: color.INK.light
    if (v.kind === "case" && this.prog.tokens.has(v.ty)) {
      const value = this.tokenValue(v);
      return this.field(value, name);
    }
    throw Failure.ir(`no field \`${name}\` on a ${V.typeName(v)}`);
  }

  defaultTokenValue(c: CaseValue): Value {
    const t = this.prog.tokens.get(c.ty);
    if (t === undefined) {
      throw Failure.ir(`\`${c.ty}\` is not a token type`);
    }
    if (c.values.length > 0) {
      return c.values[0][1];
    }
    if (!Object.hasOwn(t.values, c.name)) {
      throw Failure.ir(`no value for \`${c.ty}.${c.name}\``);
    }
    return this.work(t.values[c.name]);
  }

  protected caseDecl(ty: string, name: string): ir.CaseDecl | undefined {
    const cases = this.prog.enums.get(ty)?.cases ?? this.prog.tokens.get(ty)?.cases;
    return cases?.find((c) => c.name === name);
  }

  protected caseFields(ty: string, name: string): string[] {
    return (this.caseDecl(ty, name)?.fields ?? EMPTY).map((f) => f.name);
  }

  protected orderCaseValues(ty: string, name: string, values: Entry[]): Entry[] {
    const order = this.caseFields(ty, name);
    if (order.length === 0) {
      return values;
    }
    const out: Entry[] = [];
    for (const f of order) {
      const found = values.find(([k]) => k === f);
      if (found !== undefined) {
        out.push(found);
      }
    }
    return out;
  }

  valueOf(b: Binding): Value {
    return "value" in b ? b.value : this.readPlace(b.ref);
  }

  /** An interface's fn, sent to the fn of the first argument's own type. */
  protected dispatch(iface: string, f: string, args: readonly Binding[]): Value | undefined {
    if (args.length === 0) {
      throw Failure.ir("an interface fn with no value");
    }
    const first = this.valueOf(args[0]);
    const own = `${V.typeName(first)}.${f}`;
    if (this.knows(own)) {
      return this.call(own, args);
    }
    // what the language gives every type: same field by field, and enums in their order
    if (iface === "core/same" && f === "same") {
      const b = this.valueOf(args[1]);
      return V.bool(this.equal(first, b));
    }
    if (iface === "core/ordered" && f === "before") {
      const b = this.valueOf(args[1]);
      return V.bool(this.before(first, b));
    }
    if (iface === "core/shown" && f === "show") {
      return V.text(this.show(first));
    }
    throw Failure.ir(`\`${V.typeName(first)}\` has no fn \`${f}\``);
  }

  /** An operator on two values already worked out. */
  arith(name: ir.OpName, l: Value, r: Value): Value {
    switch (name) {
      case "==":
        return V.bool(this.equal(l, r));
      case "!=":
        return V.bool(!this.equal(l, r));
      case "<":
        return V.bool(this.before(l, r));
      case ">":
        return V.bool(this.before(r, l));
      case "<=":
        return V.bool(!this.before(r, l));
      case ">=":
        return V.bool(!this.before(l, r));
    }
    let x: Num;
    let y: Num;
    if ((l.kind === "num" && r.kind === "num") || (l.kind === "duration" && (r.kind === "duration" || r.kind === "num"))) {
      x = l.n;
      y = (r as { n: Num }).n;
    } else {
      throw Failure.ir(`\`${OP_DEBUG[name]}\` on values that aren't numbers`);
    }
    let n: Num;
    switch (name) {
      case "+":
        n = numOp(() => x.add(y));
        break;
      case "-":
        n = numOp(() => x.sub(y));
        break;
      case "*":
        n = numOp(() => x.mul(y));
        break;
      case "/":
        n = numOp(() => x.div(y));
        break;
      case "%":
        n = numOp(() => x.rem(y));
        break;
      default:
        throw Failure.ir("an operator that isn't arithmetic");
    }
    if (l.kind === "duration" && name !== "%") {
      return V.duration(n);
    }
    return V.num(n);
  }

  /** `==`: field by field, unless a type's pack writes its own `same`. */
  equal(a: Value, b: Value): boolean {
    switch (a.kind) {
      case "num":
        if (b.kind === "num") {
          return a.n.equals(b.n);
        }
        break;
      case "duration":
        if (b.kind === "duration") {
          return a.n.equals(b.n);
        }
        break;
      case "text":
        if (b.kind === "text") {
          return a.s === b.s;
        }
        break;
      case "bool":
        if (b.kind === "bool") {
          return a.b === b.b;
        }
        break;
      case "list":
        if (b.kind === "list") {
          if (a.items.length !== b.items.length) {
            return false;
          }
          for (let k = 0; k < a.items.length; k++) {
            if (!this.equal(a.items[k].value, b.items[k].value)) {
              return false;
            }
          }
          return true;
        }
        break;
      case "record":
        if (b.kind === "record") {
          if (a.ty !== b.ty) {
            return false;
          }
          const same = this.prog.records.get(a.ty)?.same;
          if (same !== undefined) {
            const out = this.call(same, [{ value: a }, { value: b }]);
            return out !== undefined && out.kind === "bool" ? out.b : false;
          }
          const n = Math.min(a.fields.length, b.fields.length);
          for (let k = 0; k < n; k++) {
            if (!this.equal(a.fields[k][1], b.fields[k][1])) {
              return false;
            }
          }
          return true;
        }
        break;
      case "case":
        if (b.kind === "case") {
          if (a.ty !== b.ty || a.name !== b.name || a.values.length !== b.values.length) {
            return false;
          }
          for (let k = 0; k < a.values.length; k++) {
            if (!this.equal(a.values[k][1], b.values[k][1])) {
              return false;
            }
          }
          return true;
        }
        break;
      case "opt":
        if (b.kind === "opt") {
          if (a.value === null || b.value === null) {
            return a.value === null && b.value === null;
          }
          return this.equal(a.value, b.value);
        }
        break;
    }
    // a plain value where an opt is wanted
    if (a.kind === "opt" && a.value !== null && b.kind !== "opt") {
      return this.equal(a.value, b);
    }
    if (b.kind === "opt" && b.value !== null && a.kind !== "opt") {
      return this.equal(b.value, a);
    }
    if (a.kind === "range" && b.kind === "range") {
      const parts = [
        [a.low, b.low],
        [a.high, b.high],
        [a.step, b.step],
      ] as const;
      for (const [p, q] of parts) {
        if (p === null && q === null) {
          continue;
        }
        if (p !== null && q !== null && this.equal(p, q)) {
          continue;
        }
        return false;
      }
      return true;
    }
    // the same calls, made in the same place with the same values around them
    if (a.kind === "content" && b.kind === "content") {
      if (a === b) return true;
      if (a.blocks !== b.blocks) return false;
      const [x, y] = [a.of, b.of];
      if ("parts" in x) return "parts" in y && x.parts.length === y.parts.length && x.parts.every((p, k) => this.equal(p, y.parts[k]));
      return !("parts" in y) && x.nodes === y.nodes && x.owner === y.owner && x.at?.path === y.at?.path && x.at?.key === y.at?.key && this.sameCaptured(x.captured, y.captured);
    }
    if (a.kind === "view" && b.kind === "view") {
      if (a === b) return true;
      if (a.widget !== b.widget || a.owner !== b.owner || a.exprs !== b.exprs || a.args.size !== b.args.size) return false;
      for (const [k, v] of a.args) {
        const w = b.args.get(k);
        if (w === undefined || !this.equal(v, w)) return false;
      }
      return this.sameCaptured(a.captured, b.captured);
    }
    return false;
  }

  protected sameCaptured(a: readonly (readonly [string, Binding])[], b: readonly (readonly [string, Binding])[]): boolean {
    return a === b || (a.length === b.length && a.every(([n, x], k) => n === b[k][0] && "value" in x && "value" in b[k][1] && this.equal(x.value, (b[k][1] as { value: Value }).value)));
  }

  /** `<`: numbers and texts by value, enums in the order written, anything else by `before`. */
  before(a: Value, b: Value): boolean {
    if ((a.kind === "num" && b.kind === "num") || (a.kind === "duration" && b.kind === "duration")) {
      return a.n.lt(b.n);
    }
    if (a.kind === "text" && b.kind === "text") {
      return V.compareText(a.s, b.s) < 0;
    }
    if (a.kind === "bool" && b.kind === "bool") {
      return !a.b && b.b;
    }
    if (a.kind === "case" && b.kind === "case" && a.ty === b.ty) {
      const cases = this.prog.enums.get(a.ty)?.cases ?? EMPTY;
      const at = (n: string): number => {
        const k = cases.findIndex((c) => c.name === n);
        return k < 0 ? 0 : k;
      };
      return at(a.name) < at(b.name);
    }
    const out = this.call(`${V.typeName(a)}.before`, [{ value: a }, { value: b }]);
    return out !== undefined && out.kind === "bool" ? out.b : false;
  }

  /** How a value is shown in a hole (page 5). */
  show(v: Value): string {
    switch (v.kind) {
      case "num":
        return v.n.show();
      case "text":
        return v.s;
      case "bool":
        return v.b ? "true" : "false";
      case "duration":
        return text.showDuration(v.n);
      case "opt":
        if (v.value !== null) {
          return this.show(v.value);
        }
        break;
    }
    const out = this.call(`${V.typeName(v)}.show`, [{ value: v }]);
    if (out !== undefined && out.kind === "text") {
      return out.s;
    }
    throw Failure.ir(`\`${V.typeName(v)}\` has no show`);
  }

  // ---- records

  /** A record from its fields, with the defaults it has, its rules run. With `checked`, its
   * fields' contracts are checked too, as `try` does. */
  makeRecord(ty: string, given: readonly (readonly [string, Value])[], checked: boolean): Value {
    const decl = this.prog.records.get(ty);
    if (decl === undefined) {
      // an inline record: the fields as given
      return V.record(ty, given.map(([k, v]) => [k, v] as const));
    }
    const fields: Entry[] = [];
    for (const f of decl.fields) {
      let v = given.find(([k]) => k === f.name)?.[1];
      if (v === undefined) {
        if (f.default === undefined) {
          throw Failure.ir(`\`${ty}\` is made without its field \`${f.name}\``);
        }
        v = this.work(f.default);
      }
      if (checked && !this.fits(v, f.type)) {
        throw Failure.rule(`${short(ty)}'s ${f.name} is outside its contract`);
      }
      fields.push([f.name, v]);
    }
    const record = V.record(ty, fields);
    this.checkRequires(decl, record);
    return record;
  }

  protected with(base: Value, given: readonly (readonly [string, Value])[], checked: boolean): Value {
    if (base.kind !== "record") {
      throw Failure.ir("`with` on something that isn't a record");
    }
    const fields: Entry[] = base.fields.slice();
    for (const [k, v] of given) {
      const at = fields.findIndex(([n]) => n === k);
      if (at >= 0) {
        fields[at] = [fields[at][0], v];
      }
    }
    const decl = this.prog.records.get(base.ty);
    if (checked && decl !== undefined) {
      for (const f of decl.fields) {
        const v = V.entry(fields, f.name);
        if (v !== undefined && !this.fits(v, f.type)) {
          throw Failure.rule(`${short(base.ty)}'s ${f.name} is outside its contract`);
        }
      }
    }
    const record = V.record(base.ty, fields);
    if (decl !== undefined) {
      this.checkRequires(decl, record);
    }
    return record;
  }

  protected checkRequires(decl: ir.Record, record: RecordValue): void {
    if ((decl.requires ?? []).length === 0) {
      return;
    }
    for (const rule of (decl.requires ?? [])) {
      const out = this.rule(rule, record);
      if (!(out !== undefined && out.kind === "bool" && out.b)) {
        if (rule.warn) {
          this.warned.push(rule.message);
          continue;
        }
        throw Failure.rule(rule.message);
      }
    }
  }

  protected caseFieldTypes(ty: string, name: string): ir.Type[] {
    return (this.caseDecl(ty, name)?.fields ?? EMPTY).map((f) => f.type);
  }

  /** Whether a value is inside a type's contract, its aliases' contracts included. */
  fits(v: Value, ty: ir.Type): boolean {
    const contracts: ir.Contract[] = [];
    let t = ty;
    let guard = 0;
    for (;;) {
      if (t.contract !== undefined) {
        contracts.push(t.contract);
      }
      const next = this.prog.aliases.get(t.name);
      if (next !== undefined && guard < 16) {
        t = next;
        guard += 1;
      } else {
        break;
      }
    }
    for (const c of contracts) {
      if (!this.fitsContract(v, c)) {
        return false;
      }
    }
    // the items of a list, and the value inside an opt
    if (v.kind === "list" && t.name === "core/list") {
      const item = t.args?.[0];
      if (item !== undefined) {
        for (const i of v.items) {
          if (!this.fits(i.value, item)) {
            return false;
          }
        }
      }
    } else if (v.kind === "opt" && v.value !== null && t.name === "core/opt") {
      const inner = t.args?.[0];
      if (inner !== undefined) {
        return this.fits(v.value, inner);
      }
    }
    return true;
  }

  /** Whether a value that comes from outside, from a host, an engine or a snapshot, is one of a type: its
   * shape, and every contract in it. A record's own rule that is false is a failure. The compiler has
   * proved only the page's own values. */
  takes(v: Value, ty: ir.Type, depth = 0): boolean {
    if (depth > MAX_DEPTH) {
      return false;
    }
    let t = ty;
    const contracts: ir.Contract[] = [];
    for (let guard = 0; ; guard++) {
      if (t.contract !== undefined) {
        contracts.push(t.contract);
      }
      const next = this.prog.aliases.get(t.name);
      if (next === undefined || guard >= 16) {
        break;
      }
      t = next;
    }
    return this.shaped(v, t, depth) && contracts.every((c) => this.fitsContract(v, c));
  }

  private shaped(v: Value, t: ir.Type, depth: number): boolean {
    const arg = t.args?.[0];
    const each = (x: Value, ty: ir.Type | undefined): boolean => ty === undefined || this.takes(x, ty, depth + 1);
    switch (t.name) {
      case "core/number":
        return v.kind === "num";
      case "core/text":
        return v.kind === "text";
      case "core/bool":
        return v.kind === "bool";
      case "core/duration":
        return v.kind === "duration";
      case "core/list":
        return v.kind === "list" && v.items.every((i) => each(i.value, arg));
      case "core/opt":
        return v.kind === "opt" && (v.value === null || each(v.value, arg));
      case "core/range":
        return v.kind === "range";
      case "fn":
        return v.kind === "fn";
      case "record":
        return v.kind === "record" && this.shapedEntries(v.fields, t.fields ?? [], (f) => f.type, depth);
    }
    const rec = this.prog.records.get(t.name);
    if (rec !== undefined) {
      if (v.kind !== "record" || v.ty !== t.name || !this.shapedEntries(v.fields, rec.fields, (f) => filled(f.type, rec.params, t.args), depth)) {
        return false;
      }
      this.checkRequires(rec, v);
      return true;
    }
    const cases = this.prog.enums.get(t.name) ?? this.prog.tokens.get(t.name);
    if (cases !== undefined) {
      if (v.kind !== "case" || v.ty !== t.name) {
        return false;
      }
      const c = cases.cases.find((c) => c.name === v.name);
      const params = "params" in cases ? cases.params : undefined;
      return c !== undefined && this.shapedEntries(v.values, c.fields ?? [], (f) => filled(f.type, params, t.args), depth);
    }
    // a type parameter, or a view, which comes from no one outside
    return true;
  }

  private shapedEntries(given: readonly Entry[], fields: readonly ir.Field[], type: (f: ir.Field) => ir.Type, depth: number): boolean {
    return given.length === fields.length && fields.every((f) => {
      const x = V.entry(given, f.name);
      return x !== undefined && this.takes(x, type(f), depth + 1);
    });
  }

  private fitsContract(v: Value, c: ir.Contract): boolean {
    if (c.range !== undefined) {
      if (c.range.low !== undefined) {
        const low = this.work(c.range.low);
        if (this.before(v, low)) {
          return false;
        }
      }
      if (c.range.high !== undefined) {
        const high = this.work(c.range.high);
        if (this.before(high, v)) {
          return false;
        }
      }
    }
    if (c.step !== undefined) {
      const step = V.asNum(this.work(c.step)) ?? int(1);
      const low = c.range?.low;
      const from = low !== undefined ? (V.asNum(this.work(low)) ?? Num.zero()) : Num.zero();
      const x = V.asNum(v);
      if (x !== undefined) {
        const k = numOp(() => x.sub(from).div(step));
        if (!k.isInt()) {
          return false;
        }
      }
    }
    if (c.length !== undefined) {
      const n = v.kind === "text" ? text.textLength(v.s) : v.kind === "list" ? v.items.length : 0;
      if (c.length.low !== undefined) {
        const l = V.asNum(this.work(c.length.low))?.toI64() ?? null;
        if (l !== null && n < l) {
          return false;
        }
      }
      if (c.length.high !== undefined) {
        const h = V.asNum(this.work(c.length.high))?.toI64() ?? null;
        if (h !== null && n > h) {
          return false;
        }
      }
    }
    if (c.only !== undefined && c.only.length > 0) {
      let any = false;
      for (const x of c.only) {
        const o = this.work(x);
        if (this.equal(v, o)) {
          any = true;
          break;
        }
      }
      if (!any) {
        return false;
      }
    }
    return true;
  }
}

function findItem(items: readonly Item[], id: number): Item | undefined {
  for (const i of items) {
    if (i.id === id) {
      return i;
    }
  }
  return undefined;
}

/** Writes `value` at `path` inside `whole`: a new value along the path, the rest shared. */
/** A field's type with its declaration's type parameters as a use gives them. */
function filled(t: ir.Type, params: readonly ir.TypeParam[] | undefined, args: readonly ir.Type[] | undefined): ir.Type {
  if (!params?.length || !args?.length) {
    return t;
  }
  const at = params.findIndex((p) => p.name === t.name);
  if (at >= 0) {
    return args[at] ?? t;
  }
  return t.args ? { ...t, args: t.args.map((a) => filled(a, params, args)) } : t;
}

function writePath(whole: Value, path: readonly Step[], k: number, value: Value): Value {
  if (k === path.length) {
    return value;
  }
  const s = path[k];
  switch (s.kind) {
    case "field":
      if (whole.kind === "record") {
        const at = whole.fields.findIndex(([n]) => n === s.name);
        if (at < 0) {
          throw Failure.ir(`no field \`${s.name}\``);
        }
        const fields = whole.fields.slice();
        fields[at] = [fields[at][0], writePath(fields[at][1], path, k + 1, value)];
        return V.record(whole.ty, fields);
      }
      break;
    case "index":
      if (whole.kind === "list") {
        const item = s.index >= 0 ? whole.items[s.index] : undefined;
        if (item === undefined) {
          throw new Failure("Position", "a position outside the list");
        }
        const items = whole.items.slice();
        items[s.index] = { id: item.id, value: writePath(item.value, path, k + 1, value) };
        return { kind: "list", items };
      }
      break;
    case "item":
    case "itemOpt":
      if (whole.kind === "list") {
        const at = whole.items.findIndex((i) => i.id === s.id);
        if (at < 0) {
          throw new Failure("Removed", "an item used after it was removed");
        }
        const items = whole.items.slice();
        items[at] = { id: items[at].id, value: writePath(items[at].value, path, k + 1, value) };
        return { kind: "list", items };
      }
      break;
    case "inside":
      if (whole.kind === "opt") {
        if (whole.value === null) {
          throw new Failure("Removed", "an opt that is empty");
        }
        return V.some(writePath(whole.value, path, k + 1, value));
      }
      break;
  }
  throw Failure.ir("a place that doesn't fit its value");
}
