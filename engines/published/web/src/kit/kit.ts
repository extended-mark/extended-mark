// What a built page runs on: the state it has, a change done whole or undone, who each widget is, and what is
// drawn again. Generated code gives it the page: each expression, statement and rule as a fn, each part of a view
// as a `Part` with the fn that draws it. There is no IR here, and a page holds the kinds of part it has.

import { Failure, MAX_DEPTH, Ops, type Binding, type Declared, type Place, type PlaceRoot } from "@extended-mark/runtime/ops";
import { Num } from "@extended-mark/runtime/num";
import { none, numberedStart, sortedEntries, toIr, type Value } from "@extended-mark/runtime/values";
import type { Native } from "@extended-mark/runtime/natives";
import type { Input, ir } from "@extended-mark/runtime";

/** A local: its value, or a reference to a place. */
export type Bound = Value | { ref: Place };
type Entry = readonly [string, Value];

/** An expression, with the locals around it and the widget it is in. */
export type X = (m: Kit, b: Bound[], o?: Inst) => Value;
/** Statements: what they return, if they do. */
export type S = (m: Kit, b: Bound[], o?: Inst) => Value | undefined;
/** Where a place is. */
export type P = (m: Kit, b: Bound[], o?: Inst) => Place;

/** A fn: how many parameters it has, which are `mut` (a bit each), and its body. */
export interface F {
  n: number;
  mu?: number;
  f: S;
}

export interface Rule {
  message: string;
  warn?: boolean;
  f: S;
}

/** A type as a value from outside is checked by, its contract's ends fns. */
export type T = ir.Type;

/** An extern widget's declaration, as far as the page needs it. */
export interface Widget {
  n: string;
  /** The inputs with a default. */
  i?: [string, X][];
  /** The `mut` inputs: a type each, and what a value of it has to fit, for whoever draws it. */
  mu?: [string, T, Contracts][];
  /** It has type parameters. */
  g?: boolean;
  /** Its rules. */
  q?: Rule[];
  /** Its events' parameters. */
  ev?: Record<string, T[]>;
  /** The inputs that hold sentences. */
  st?: string[];
  /** The types of its slots, for its rules. */
  sl?: Record<string, SlotKind>;
  /** What it is drawn as, when the widget set has no such widget: its Markdown. */
  md?: Part[];
}

/** A contract, worked out as the page is made. */
export type Contracts = Record<string, Input | Record<string, Input> | Input[]> | ((m: Kit) => Record<string, unknown>);

/** How a slot is read: its content, its calls in a list, or the one call it holds. `opt` may be left out. */
export type SlotKind = "view" | "list" | "one" | "opt view" | "opt list" | "opt one";

/** A widget written in Extended Mark. */
export interface Template {
  n: string;
  i?: [string, X | undefined][];
  c?: [string, X | undefined, T][];
  q?: Rule[];
  sl?: Record<string, SlotKind>;
  v: Part[];
}

export interface Handler {
  n: number;
  f: S;
  /** The event it only passes on, when that is all it does. */
  only?: string;
}

/** Content in a slot. */
interface Filled {
  n: Part[];
  e: Env;
}

/** A widget written in Extended Mark, where it is on the page. */
export interface Inst {
  id: number;
  k: string;
  t: Pick<Template, "n" | "c" | "sl">;
  i: Map<string, Value>;
  /** `mut` inputs: the caller's place, read and written live. */
  pl: Map<string, { f: P; e: Env }>;
  c: Map<string, Value>;
  s: Map<string, Filled>;
  ev: Map<string, { h: Handler; e: Env }>;
  seen: number;
}

/** Where a part is: its widget, the locals around it, its place as observed and who it is. */
export interface Env {
  o: Inst | undefined;
  b: Bound[];
  p: string;
  k: string;
}

/** A part of a view, with the fn that draws it. One that binds a local gives the env after it. */
export interface Part {
  d(m: Kit, n: any, e: Env, out: Drawn[]): Env | void;
  s: string;
}

export interface Drawn {
  p: string;
  k: string;
  w: Widget | { n: string };
  props: Map<string, Value>;
  slots: Map<string, Drawn[]>;
  ev: Map<string, { h: Handler; e: Env }>;
  wr: Map<string, { f: P; e: Env; c: Contracts }>;
  failed?: string;
}

/** What drawing one part of the page read and met, itself: what is inside it has its own, in `kids`. */
export class Frame {
  deps: Map<string, number> | undefined;
  parts: { place: Place; was: Value; deps: Map<string, number> }[] | undefined;
  kids: Frame[] | undefined;
  nested: [Inst, string][] | undefined;
  own: string[] | undefined;
  /** It failed, or asked the host for a value: drawn anew every time. */
  dirty = false;
  seen = 0;
  fresh = 0;
}

export interface Broken {
  rule?: string;
  code?: string;
  site?: string;
}

/** What generated code gives: the page. */
export interface Program extends Declared {
  /** Fns called by their name: those of types, which an interface's is sent to, and the host's. */
  fns: Record<string, F | Native>;
  /** The fns somebody outside gives: their default body, and what they return. */
  ext?: Record<string, { d?: F; r?: T; host?: boolean }>;
  st?: [string, X | undefined, T][];
  q?: Rule[];
  /** Each channel's values this page reads. */
  ch?: Record<string, Record<string, [T, X | undefined]>>;
  ex?: Record<string, Record<string, [F, T[]]>>;
  v: Part[];
}

export interface Given {
  page?: { width?: string; theme?: string };
  state?: Record<string, Input>;
  data?: { state?: Record<string, Input>; own?: Record<string, Record<string, Input>> };
  externs?: Record<string, Record<string, Input>>;
  fns?: Record<string, (args: readonly Input[]) => Input | null>;
}

const MAX_PASSES = 8;
export const NONE = new Map<string, never>() as Map<string, any>;

/** The env of what comes after a local that failed: nothing is drawn in it. */
export const STOP: Env = { o: undefined, b: [], p: "", k: "" };

export function join(path: string, part: string): string {
  return path ? `${path}/${part}` : part;
}

function isRef(b: Bound): b is { ref: Place } {
  return "ref" in b;
}

function code(f: unknown): string {
  return f instanceof Failure ? `failure.${f.kind.toLowerCase()}` : "failure.ir";
}

function refused(f: unknown, site: string): Broken {
  return f instanceof Failure && f.rule ? { rule: f.rule, site } : { code: "contracts.outside", site };
}

/** Whether a value is the one it was: an opt is made anew each time it is read. */
function sameNow(a: Value, b: Value): boolean {
  return a === b || (a.kind === "opt" && b.kind === "opt" && (a.value === null || b.value === null ? a.value === b.value : sameNow(a.value, b.value)));
}

function sameRef(a: Place, b: Place): boolean {
  const [p, q] = [a.root as any, b.root as any];
  const root = p.kind === q.kind && (p.kind === "local" ? p.b === q.b && p.at === q.at : p.owner === q.owner && p.outer.kind === q.outer.kind && p.outer.name === q.outer.name);
  return root && a.path.length === b.path.length && a.path.every((s, k) => JSON.stringify(s) === JSON.stringify(b.path[k]));
}

/** A fn value of a built page. */
interface Closure {
  kind: "fn";
  f: F;
  captured: Bound[];
  owner: Inst | undefined;
}

export class Kit extends Ops {
  readonly prog: Program;
  readonly errors: unknown[] = [];
  private state = new Map<string, Value>();
  private externs = new Map<string, Value>();
  private fnsGiven: Record<string, (args: readonly Input[]) => Input | null>;
  private calls: { call: { name: string; args: Input[] } }[] = [];
  private refusals: Broken[] = [];
  private failures: Broken[] = [];
  private warnings: Broken[] = [];
  private warns = new Set<string>();
  readonly insts = new Map<string, Inst>();
  atPath = new Map<string, Inst>();
  own = new Map<string, Map<string, Value>>();
  ownSeen = new Set<string>();
  claimed = new Set<string>();
  drawn: Drawn[] = [];
  /** Where what each of the page's own parts drew ends. */
  ends: number[] = [];
  byKey: Map<string, Drawn> | undefined;
  private versions = new Map<string, number>();
  stamp = 1;
  epoch = 0;
  draws = 0;
  changes = 0;
  private held: number | undefined;
  frames: Frame[] = [];
  private partDeps: Map<string, number> | undefined;
  private whole = 0;
  /** What was kept of each part, by its key. */
  memo = new Map<string, any>();
  nodeSeen = new Set<string>();
  private rules = new Map<Rule, { deps: Map<string, number>; epoch: number; holds: boolean }>();
  inInputs = 0;
  private putBack: Map<string, Map<string, Input>> | undefined;
  /** The frame the code running has its locals in. */
  private cur: Bound[] = [];
  listener: () => void = () => {};

  constructor(prog: Program, given: Given = {}) {
    super();
    this.prog = prog;
    this.fnsGiven = given.fns ?? {};
    this.ready();
    for (const [c, values] of Object.entries(prog.ch ?? {})) {
      for (const [n, [, x]] of Object.entries(values)) if (x) this.externs.set(`${c}.${n}`, x(this, []));
    }
    for (const [n, x] of prog.st ?? []) if (x) this.state.set(n, x(this, []));
    for (const [c, values] of Object.entries(given.externs ?? {})) for (const [k, x] of Object.entries(values)) this.giveExtern(c, k, this.given(x, `${c}.${k}`));
    if (given.page) this.applyPage(given.page);
    this.startFrom(given.state ?? {});
    const took = given.data && this.takeState(given.data.state ?? {});
    this.nextId = numberedStart(this.state, this.externs);
    this.left = given.data && took ? this.drawWith(given.data.own ?? {}) : undefined;
    if (!this.left) this.redraw();
  }

  readonly left: string[] | undefined;

  /** Called before anything is drawn. */
  protected ready(): void {}

  // ---- what the evaluator's operations ask

  protected work(e: ir.Expr): Value {
    return (e as unknown as X)(this, []);
  }

  protected rule(r: ir.Require, record: Value & { kind: "record" }): Value | undefined {
    return this.runF({ n: record.fields.length, f: (r as unknown as Rule).f }, [], record.fields.map(([, v]) => v), undefined);
  }

  protected knows(name: string): boolean {
    return Object.hasOwn(this.prog.fns, name) || Object.hasOwn(this.prog.ext ?? {}, name);
  }

  override valueOf(b: Binding): Value {
    const x = b as unknown as Bound & { value?: Value };
    if (isRef(x)) return this.readPlace(x.ref);
    return "kind" in x ? x : (x as { value: Value }).value;
  }

  protected override sameCaptured(a: readonly any[], b: readonly any[]): boolean {
    return a === b || (a.length === b.length && a.every((x, k) => !isRef(x) && !isRef(b[k]) && this.equal(x, b[k])));
  }

  /** A fn by its full name: one of a type's, or the host's. */
  call(name: string, args: readonly Binding[]): Value | undefined {
    const f = this.prog.fns[name];
    if (f) {
      this.step();
      return typeof f === "function" ? f(this, args) : this.runF(f, [], args as unknown as Bound[], undefined);
    }
    if (this.prog.ext?.[name]) return this.ext(name, args.map((a) => this.valueOf(a)));
    throw Failure.ir(`no fn \`${name}\``);
  }

  /** A fn somebody outside gives. */
  ext(name: string, args: Value[]): Value | undefined {
    this.step();
    const x = this.prog.ext![name];
    const own = this.fnsGiven[name];
    if (own) {
      const out = own(args.map((a) => this.plain(a)));
      if (out === null) return undefined;
      const v = this.fromInput(out);
      if (x.r && !this.takes(v, x.r)) throw new Failure("Rule", `\`${name}\` gave what isn't its type`);
      return v;
    }
    if (x.host) {
      const top = this.frames[this.frames.length - 1];
      if (top) top.dirty = true;
      this.calls.push({ call: { name, args: args.map((a) => this.plain(a)) } });
      return undefined;
    }
    if (x.d) return this.runF(x.d, [], args, undefined);
    throw new Failure("NotConstant", `nobody supplies \`${name}\``);
  }

  /** An interface's fn, sent to the fn of the first argument's own type. */
  iface(iface: string, f: string, args: Bound[]): Value | undefined {
    this.step();
    return this.dispatch(iface, f, args as unknown as Binding[]);
  }

  /** A fn written in Extended Mark, called in a widget. */
  fn(f: F, args: Bound[], o?: Inst): Value {
    this.step();
    return this.runF(f, [], args, o)!;
  }

  /** A native fn. */
  nat(f: Native, args: Bound[]): Value {
    this.step();
    return f(this, args as unknown as Binding[])!;
  }

  /** A fn value. */
  apply(f: Value, args: Bound[]): Value | undefined {
    this.step();
    if (f.kind !== "fn") throw Failure.ir("a call of something that isn't a fn");
    const c = f as unknown as Closure;
    return this.runF(c.f, c.captured, args, c.owner);
  }

  private runF(f: F, captured: Bound[], args: readonly Bound[], owner: Inst | undefined): Value | undefined {
    if (this.depth >= MAX_DEPTH) throw Failure.ofLimit("Depth", "calls more than 64 deep");
    const b = captured.slice();
    const n = Math.min(f.n, args.length);
    for (let k = 0; k < n; k++) b.push(f.mu !== undefined && f.mu & (1 << k) ? args[k] : this.valueOf(args[k] as unknown as Binding));
    return this.body(f.f, b, owner);
  }

  private body(f: S, b: Bound[], owner: Inst | undefined): Value | undefined {
    const was = this.cur;
    this.depth += 1;
    this.cur = b;
    try {
      return f(this, b, owner);
    } finally {
      this.cur = was;
      this.depth -= 1;
    }
  }

  /** What a call that has to return something returned. */
  must(v: Value | undefined, name: string): Value {
    if (v === undefined) throw Failure.ir(`\`${name}\` returns nothing`);
    return v;
  }

  // ---- what expressions read

  st(name: string): Value {
    this.track(`s:${name}`);
    return this.state.get(name)!;
  }

  x(name: string): Value {
    this.track(`x:${name}`);
    const v = this.externs.get(name);
    if (!v) throw new Failure("NotConstant", `${name} is given by the host`);
    return v;
  }

  in(o: Inst, name: string): Value {
    const p = o.pl.get(name);
    if (p) return this.readPlace(p.f(this, p.e.b, p.e.o));
    this.track(`i:${o.id}:${name}`);
    return o.i.get(name)!;
  }

  cell(o: Inst, name: string): Value {
    this.track(`c:${o.id}:${name}`);
    return o.c.get(name)!;
  }

  slot(o: Inst, name: string): Value {
    this.track(`l:${o.id}:${name}`);
    const s = o.s.get(name);
    const kind = o.t.sl?.[name];
    if (!s) {
      if (kind?.startsWith("opt")) return none();
      throw new Failure("Ir", `the slot ${name} isn't filled`);
    }
    if (kind?.endsWith("view")) return { kind: "content", of: { nodes: s.n, captured: s.e.b, owner: s.e.o, at: { path: s.e.p, key: s.e.k } }, blocks: false } as unknown as Value;
    const calls = this.callsIn(s.n, s.e);
    return kind?.endsWith("list") ? this.makeList(calls) : (calls[0] ?? none());
  }

  /** The calls some parts make, each a view of its own: set by `calls.ts`, where a page reads such a slot. */
  callsIn(n: Part[], e: Env): Value[] {
    throw Failure.ir(`no calls of ${n.length} parts in ${e.k}`);
  }

  // ---- places

  protected readRoot(root: PlaceRoot): Value {
    const r = root as any;
    if (r.kind === "local") return this.valueOf(r.b[r.at]);
    switch (r.outer.kind) {
      case "state":
        return this.st(r.outer.name);
      case "cell":
        return this.cell(r.owner, r.outer.name);
      default:
        return this.in(r.owner, r.outer.name);
    }
  }

  protected writeRoot(root: PlaceRoot, value: Value): void {
    const r = root as any;
    if (r.kind === "local") {
      const was = r.b[r.at];
      if (isRef(was)) return this.writePlace(was.ref, value);
      // a fn given the variable writes it through a `mut` parameter, whose contract may be wider
      if (r.t && r.b !== this.cur && !this.fits(value, r.t)) throw new Failure("Rule", `${r.name} is outside its contract`);
      r.b[r.at] = value;
      return;
    }
    const { kind, name } = r.outer;
    if (kind === "state") {
      const f = this.prog.st?.find((f) => f[0] === name);
      if (f && !this.fits(value, f[2])) throw new Failure("Rule", `$state.${name} is outside its contract`);
      this.state.set(name, value);
      this.bump(`s:${name}`);
    } else if (kind === "cell") {
      const c = r.owner.t.c?.find((c: [string]) => c[0] === name);
      if (c && !this.fits(value, c[2])) throw new Failure("Rule", `${name} is outside its contract`);
      r.owner.c.set(name, value);
      this.bump(`c:${r.owner.id}:${name}`);
    } else {
      const p = r.owner.pl.get(name);
      if (!p) throw new Failure("Ir", `${name} is not a mut input given a variable`);
      this.writePlace(p.f(this, p.e.b, p.e.o), value);
    }
  }

  protected override partOf(p: Place): Value {
    const top = this.frames[this.frames.length - 1];
    if (!top || this.partDeps || this.whole) return this.valueAt(p);
    const deps = new Map<string, number>();
    const was = this.partAt(p, deps);
    (top.parts ??= []).push({ place: p, was, deps });
    return was;
  }

  private partAt(p: Place, deps: Map<string, number>): Value {
    const around = this.partDeps;
    this.partDeps = deps;
    try {
      return this.valueAt(p);
    } finally {
      this.partDeps = around;
    }
  }

  private partsStand(parts: NonNullable<Frame["parts"]>): boolean {
    try {
      return parts.every((p) => {
        if (this.current(p.deps)) return true;
        const deps = new Map<string, number>();
        if (!sameNow(p.was, this.partAt(p.place, deps))) return false;
        p.deps = deps;
        return true;
      });
    } catch {
      return false;
    }
  }

  // ---- what is kept of what was drawn

  track(key: string): void {
    const part = this.partDeps;
    if (part) {
      if (!part.has(key)) part.set(key, this.versions.get(key) ?? 0);
      return;
    }
    const f = this.frames[this.frames.length - 1];
    if (f && !f.deps?.has(key)) (f.deps ??= new Map()).set(key, this.versions.get(key) ?? 0);
  }

  bump(key: string): void {
    this.versions.set(key, (this.versions.get(key) ?? 0) + 1);
    this.stamp++;
  }

  current(deps: ReadonlyMap<string, number>): boolean {
    for (const [k, v] of deps) if ((this.versions.get(k) ?? 0) !== v) return false;
    return true;
  }

  enter(): Frame {
    const frame = new Frame();
    frame.seen = this.draws;
    this.frames.push(frame);
    return frame;
  }

  exit(frame: Frame): void {
    this.frames.pop();
    const around = this.frames[this.frames.length - 1];
    if (!around) return;
    (around.kids ??= []).push(frame);
    if (frame.dirty) around.dirty = true;
  }

  replay(frame: Frame): void {
    this.see(frame);
    const around = this.frames[this.frames.length - 1];
    if (around) (around.kids ??= []).push(frame);
  }

  private see(frame: Frame): void {
    if (frame.seen === this.draws) return;
    frame.seen = this.draws;
    for (const [inst, path] of frame.nested ?? []) {
      inst.seen = this.draws;
      this.atPath.set(path, inst);
    }
    for (const k of frame.own ?? []) this.ownSeen.add(k);
    for (const k of frame.kids ?? []) this.see(k);
  }

  unchanged(frame: Frame): boolean {
    if (frame.fresh < 0) return false;
    if (frame.fresh === this.stamp) return true;
    const ok = (!frame.deps || this.current(frame.deps)) && (!frame.parts || this.partsStand(frame.parts)) && (!frame.kids || frame.kids.every((k) => this.unchanged(k)));
    frame.fresh = ok ? this.stamp : -1;
    return ok;
  }

  sameBound(a: Bound[], b: Bound[]): boolean {
    return a === b || (a.length === b.length && a.every((x, k) => {
      const y = b[k];
      if (isRef(x) || isRef(y)) return isRef(x) && isRef(y) && sameRef(x.ref, y.ref);
      return x === y || this.equal(x, y);
    }));
  }

  /** What a fn's calls read, for whoever holds what they gave. */
  reading<R>(f: () => R, fresh = true): { value: R; deps: Map<string, number> } {
    const frame = this.enter();
    this.whole++;
    try {
      if (fresh) this.resetSteps();
      const value = f();
      const deps = new Map<string, number>();
      const all = (x: Frame): void => {
        for (const [k, v] of x.deps ?? []) if (!deps.has(k)) deps.set(k, v);
        x.kids?.forEach(all);
      };
      all(frame);
      return { value, deps };
    } finally {
      this.whole--;
      this.exit(frame);
    }
  }

  // ---- drawing

  parts(n: Part[], e: Env, out: Drawn[] = []): Drawn[] {
    for (const p of n) {
      e = p.d(this, p, e, out) ?? e;
      if (e === STOP) break;
    }
    return out;
  }

  /** Whether a failure here stays here: in the document's own code, and not in a sentence. */
  contains(e: Env): boolean {
    return e.o === undefined && this.inInputs === 0;
  }

  failed(name: string, p: string, k: string, f: unknown): Drawn {
    const top = this.frames[this.frames.length - 1];
    if (top) top.dirty = true;
    const broken: Broken = f instanceof Failure && f.kind === "Rule" && f.rule ? { rule: f.rule, site: k } : { code: code(f), site: k };
    if (!this.failures.some((b) => b.site === k && b.code === broken.code && b.rule === broken.rule)) this.failures.push(broken);
    return { p, k, w: { n: name }, props: NONE, slots: NONE, ev: NONE, wr: NONE, failed: f instanceof Error ? f.message : String(f) };
  }

  /** Who a widget is, this redraw: the place it was written, or that and which time it is placed. */
  claim(key: string): string {
    let id = key;
    for (let n = 2; this.claimed.has(id) || this.memo.get(`w|${id}`)?.frame.seen === this.draws || this.insts.get(id)?.seen === this.draws; n++) id = `${key}~${n}`;
    this.claimed.add(id);
    return id;
  }

  redraw(): void {
    this.draws++;
    this.resetSteps();
    this.claimed.clear();
    this.atPath = new Map();
    this.ownSeen.clear();
    this.nodeSeen.clear();
    for (const r of this.prog.q ?? []) {
      if (!r.warn) continue;
      try {
        this.warned_("$state", r.message, this.stateRule(r));
      } catch (f) {
        this.errors.push(f);
        this.failures.push({ code: code(f), site: "$state" });
      }
    }
    let e: Env = { o: undefined, b: [], p: "", k: "" };
    this.drawn = [];
    this.ends = this.prog.v.map((p) => {
      if (e !== STOP) e = p.d(this, p, e, this.drawn) ?? e;
      return this.drawn.length;
    });
    this.byKey = undefined;
    // a widget that left the page leaves with its variables
    for (const [k, i] of this.insts) {
      if (i.seen !== this.draws) {
        this.insts.delete(k);
        this.memo.delete(`u|${k}`);
      }
    }
    for (const k of this.own.keys()) if (!this.ownSeen.has(k)) this.own.delete(k);
    for (const [k, m] of this.memo) if (m.frame.seen !== this.draws) this.memo.delete(k);
    this.listener();
  }

  warned_(site: string, message: string, holds: boolean): void {
    const at = `${site}\n${message}`;
    if (holds) this.warns.delete(at);
    else if (!this.warns.has(at)) {
      this.warns.add(at);
      this.warnings.push({ rule: message, site });
    }
  }

  /** A widget's rules: a `require` has to hold, and a `warn` is told. */
  checkRules(rules: Rule[], o: Inst, site: string): void {
    for (const r of rules) {
      const ok = this.body_(r.f, [], o);
      const holds = !(ok && ok.kind === "bool" && !ok.b);
      if (r.warn) this.warned_(site, r.message, holds);
      else if (!holds) throw new Failure("Rule", r.message, r.message);
    }
  }

  /** Code run as a fn of its own. */
  private body_(f: S, b: Bound[], o: Inst | undefined): Value | undefined {
    this.step();
    return this.runF({ n: 0, f }, b, [], o);
  }

  // ---- changes: each is done whole, with every rule after it, or undone

  private snapshot() {
    return {
      state: new Map(this.state),
      cells: [...this.insts.values()].filter((i) => i.c.size).map((i) => [i, new Map(i.c)] as const),
      calls: this.calls.length,
    };
  }

  private restore(s: ReturnType<Kit["snapshot"]>): void {
    this.epoch++;
    this.state = s.state;
    for (const [i, c] of s.cells) i.c = c;
    this.calls.length = s.calls;
  }

  private stateRule(r: Rule): boolean {
    const m = this.rules.get(r);
    if (m && m.epoch === this.epoch && this.current(m.deps)) return m.holds;
    const { value, deps } = this.reading(() => this.body_(r.f, [], undefined), false);
    const holds = !(value && value.kind === "bool" && !value.b);
    this.rules.set(r, { deps, epoch: this.epoch, holds });
    return holds;
  }

  private stateRuleBroken(): string | undefined {
    for (const r of this.prog.q ?? []) if (!r.warn && !this.stateRule(r)) return r.message;
    return undefined;
  }

  private stateRefusal(): Broken | undefined {
    try {
      const rule = this.stateRuleBroken();
      return rule === undefined ? undefined : { rule };
    } catch (f) {
      this.errors.push(f);
      return { code: code(f), site: "$state" };
    }
  }

  private transaction(site: string, run: () => void): void {
    this.changes++;
    const snap = this.snapshot();
    try {
      this.resetSteps();
      run();
      const rule = this.stateRuleBroken();
      if (rule) throw new Failure("Rule", rule, rule);
    } catch (f) {
      this.errors.push(f);
      this.restore(snap);
      if (f instanceof Failure && f.kind === "Rule" && f.rule) this.failures.push({ rule: f.rule, site });
      else this.failures.push({ code: code(f), site });
    }
    this.redraw();
  }

  runHandler(h: Handler, e: Env, args: Bound[]): void {
    this.step();
    this.runF({ n: h.n, f: h.f }, e.b, args, e.o);
  }

  /** `invoke event(args)`. */
  invoke(o: Inst | undefined, event: string, args: Value[]): void {
    const ev = o?.ev.get(event);
    if (ev) this.runHandler(ev.h, ev.e, args);
  }

  // ---- what the reader and the widgets do

  find(id: string): Drawn | undefined {
    if (!this.byKey) {
      const by = (this.byKey = new Map<string, Drawn>());
      const walk = (ds: readonly Drawn[]): void => {
        for (const d of ds) {
          by.set(d.k, d);
          for (const kids of childrenOf(d)) walk(kids);
        }
      };
      walk(this.drawn);
    }
    return this.byKey.get(id);
  }

  event(id: string, name: string, args: readonly Input[] = []): void {
    const d = this.find(id);
    const given = d && this.givenAll(args, id);
    const ev = d?.ev.get(name);
    if (!d || !given || !ev) return;
    if (!this.takesArgs((d.w as Widget).ev?.[name] ?? [], given, d.k)) return;
    this.transaction(d.k, () => this.runHandler(ev.h, ev.e, given));
  }

  write(id: string, input: string, x: Input): void {
    const d = this.find(id);
    const value = d && this.given(x, id);
    if (!d || !value) return;
    this.changes++;
    this.resetSteps();
    const w = d.wr.get(input);
    if (!w) {
      // a widget's own value: kept by the page for it, checked against the input's type
      const f = (d.w as Widget).mu?.find((f) => f[0] === input);
      if (!f) return;
      const broken = this.refusal(value, f[1], d.k);
      if (broken) {
        this.refusals.push(broken);
        return;
      }
      this.keepOwn(d.k, input, value);
      this.redraw();
      return;
    }
    const snap = this.snapshot();
    try {
      this.writeInto(d, input, w, value);
      const broken = this.stateRefusal();
      if (broken) {
        this.restore(snap);
        this.refusals.push(broken);
      }
    } catch (f) {
      this.restore(snap);
      this.refusals.push(refused(f, d.k));
    }
    this.redraw();
  }

  keepOwn(key: string, input: string, value: Value): void {
    if (!this.own.has(key)) this.own.set(key, new Map());
    this.own.get(key)!.set(input, value);
    this.bump(`o:${key}`);
  }

  private writeInto(d: Drawn, input: string, w: { f: P; e: Env }, value: Value): void {
    const f = (d.w as Widget).mu?.find((f) => f[0] === input);
    if (f && !this.takes(value, f[1])) throw new Failure("Rule", `${input} is given what isn't its type`);
    this.writePlace(w.f(this, w.e.b, w.e.o), value);
    // an input of a type parameter's type takes any value: the variable's own type says which
    if ((d.w as Widget).g) {
      const root = this.rootOf(w.f(this, w.e.b, w.e.o).root as any);
      if (root && !this.takes(root.value, root.type)) throw new Failure("Rule", `${input} is given what isn't its variable's type`);
    }
  }

  /** The variable a place is in, `$state`'s or a widget's own: its type, and its value now. */
  private rootOf(r: any): { type: T; value: Value } | undefined {
    if (r.kind === "local") return undefined;
    const { kind, name } = r.outer;
    if (kind === "input") {
      const p = r.owner.pl.get(name);
      return p && this.rootOf(p.f(this, p.e.b, p.e.o).root);
    }
    const type = kind === "state" ? this.prog.st?.find((f) => f[0] === name)?.[2] : r.owner.t.c?.find((c: [string]) => c[0] === name)?.[2];
    const value = kind === "state" ? this.state.get(name) : r.owner.c.get(name);
    return type && value && { type, value };
  }

  /** Passes run until one changes nothing: each pass's writes are one change. */
  settle(pass: () => { writes: { id: string; input: string; value: Input }[]; of: string[] }): void {
    if (this.held === this.changes) return;
    this.held = undefined;
    let last: string[] = [];
    for (let k = 0; k < MAX_PASSES; k++) {
      const { writes, of } = pass();
      if (!of.length || !this.layoutWrite(writes)) return;
      last = of;
    }
    for (const id of last) if (!this.failures.some((b) => b.site === id && b.code === "failure.layout")) this.failures.push({ code: "failure.layout", site: id });
    this.held = this.changes;
  }

  private layoutWrite(writes: { id: string; input: string; value: Input }[]): boolean {
    const values = writes.flatMap((w) => {
      const d = this.find(w.id);
      const value = d && this.given(w.value, d.k);
      return d && value ? [{ d, input: w.input, value }] : [];
    });
    this.resetSteps();
    const snap = this.snapshot();
    const ownBefore = new Map([...this.own].map(([k, m]) => [k, new Map(m)]));
    const back = (broken: Broken) => {
      this.restore(snap);
      this.own = ownBefore;
      this.refusals.push(broken);
      return false;
    };
    let changed = false;
    let shared = false;
    for (const { d, input, value } of values) {
      const now = d.props.get(input);
      if (now && this.equal(now, value)) continue;
      const w = d.wr.get(input);
      try {
        if (w) {
          this.writeInto(d, input, w, value);
          shared = true;
        } else {
          const f = (d.w as Widget).mu?.find((f) => f[0] === input);
          if (!f) continue;
          if (!this.takes(value, f[1])) throw new Failure("Rule", `${input} is given what isn't its type`);
          this.keepOwn(d.k, input, value);
        }
        changed = true;
      } catch (f) {
        return back(refused(f, d.k));
      }
    }
    if (!changed) return false;
    const broken = this.stateRefusal();
    if (broken) return back(broken);
    this.redraw();
    return shared;
  }

  // ---- what the host does

  set(fields: Record<string, Input>): void {
    this.changes++;
    this.resetSteps();
    if (this.takeState(fields)) this.redraw();
  }

  expose(channel: string, name: string, args: readonly Input[] = []): void {
    const given = this.givenAll(args, `${channel}.${name}`);
    const x = this.prog.ex?.[channel]?.[name];
    if (!given || !x || !this.takesArgs(x[1], given, `${channel}.${name}`)) return;
    this.transaction(`${channel}.${name}`, () => {
      this.step();
      this.runF(x[0], [], given, undefined);
    });
  }

  extern(channel: string, values: Record<string, Input>): void {
    this.changes++;
    this.resetSteps();
    const given = Object.entries(values).map(([k, x]) => [k, this.given(x, `${channel}.${k}`)] as const);
    for (const [k, v] of given) {
      this.giveExtern(channel, k, v);
      this.bump(`x:${channel}.${k}`);
    }
    this.redraw();
  }

  page(values: { width?: string; theme?: string }): void {
    this.changes++;
    this.applyPage(values);
    this.redraw();
  }

  /** The page's data put back, as one change: `$state` whole or not at all, then widgets' own variables. */
  restoreData(data: { state?: Record<string, Input>; own?: Record<string, Record<string, Input>> }): string[] | undefined {
    this.changes++;
    this.resetSteps();
    return this.takeState(data.state ?? {}) ? this.drawWith(data.own ?? {}) : undefined;
  }

  // ---- what the host is told

  stateNow(): Record<string, Input> {
    return Object.fromEntries([...this.state].map(([k, v]) => [k, this.plain(v)]));
  }

  ownNow(): Record<string, Record<string, Input>> {
    const out: Record<string, Record<string, Input>> = {};
    for (const [path, i] of this.atPath) if (i.c.size) out[path] = Object.fromEntries([...i.c].map(([k, v]) => [k, this.plain(v)]));
    const walk = (ds: readonly Drawn[]): void => {
      for (const d of ds) {
        const own = [...(this.own.get(d.k) ?? [])].filter(([k]) => !d.wr.has(k));
        if (own.length) out[d.p] = { ...out[d.p], ...Object.fromEntries(own.map(([k, v]) => [k, this.plain(v)])) };
        for (const kids of childrenOf(d)) walk(kids);
      }
    };
    walk(this.drawn);
    return out;
  }

  told() {
    for (const message of this.warned.splice(0)) {
      if (this.warns.has(message)) continue;
      this.warns.add(message);
      this.warnings.push({ rule: message });
    }
    const all = { calls: this.calls, refused: this.refusals, failures: this.failures, warnings: this.warnings };
    this.calls = [];
    this.refusals = [];
    this.failures = [];
    this.warnings = [];
    return Object.fromEntries(Object.entries(all).filter(([, x]) => x.length)) as Partial<typeof all>;
  }

  externNow(name: string): Value | undefined {
    return this.externs.get(name);
  }

  // ---- inside

  private applyPage(page: { width?: string; theme?: string }): void {
    for (const k of ["width", "theme"] as const) {
      if (!page[k]) continue;
      this.externs.set(`core/page.${k}`, { kind: "case", ty: `core/${k}`, name: page[k], values: [] });
      this.bump(`x:core/page.${k}`);
    }
  }

  private giveExtern(channel: string, name: string, v: Value | undefined): void {
    const f = this.prog.ch?.[channel]?.[name];
    if (!f) return;
    const broken = v && this.refusal(v, f[0], `${channel}.${name}`);
    if (broken) this.refusals.push(broken);
    const x = v && !broken ? v : f[1]?.(this, []);
    if (x) this.externs.set(`${channel}.${name}`, x);
  }

  /** `$state` as the host gives it: all together, or else each field that fits and keeps the rules. */
  private startFrom(given: Record<string, Input>): void {
    const values: Record<string, Value> = {};
    for (const [k, x] of Object.entries(given)) {
      const v = this.given(x, `$state.${k}`);
      if (v) values[k] = v;
    }
    const told = this.refusals.length;
    if (Object.keys(values).length === 0 || this.writeState(values)) return;
    this.refusals.length = told;
    for (const [k, v] of Object.entries(values)) this.writeState({ [k]: v });
  }

  private takeState(given: Record<string, Input>): boolean {
    const values: Record<string, Value> = {};
    for (const [k, x] of Object.entries(given)) {
      const v = this.given(x, `$state.${k}`);
      if (!v) return false;
      values[k] = v;
    }
    return this.writeState(values);
  }

  private writeState(changes: Record<string, Value>): boolean {
    const snap = this.snapshot();
    for (const [k, v] of Object.entries(changes)) {
      const f = this.prog.st?.find((f) => f[0] === k);
      const broken = f ? this.refusal(v, f[2], `$state.${k}`) : { code: "contracts.outside", site: `$state.${k}` };
      if (broken) {
        this.restore(snap);
        this.refusals.push(broken);
        return false;
      }
      this.state.set(k, v);
      this.bump(`s:${k}`);
    }
    const broken = this.stateRefusal();
    if (broken) {
      this.restore(snap);
      this.refusals.push(broken);
      return false;
    }
    return true;
  }

  private drawWith(own: Record<string, Record<string, Input>>): string[] {
    const back = new Map(Object.entries(own).map(([path, vars]) => [path, new Map(Object.entries(vars))]));
    this.putBack = back;
    this.epoch++;
    try {
      this.redraw();
    } finally {
      this.putBack = undefined;
    }
    return [...back].flatMap(([path, vars]) => [...vars.keys()].map((n) => `${path}.${n}`));
  }

  /** What is put back at a place, as it is drawn. */
  back(path: string, type: (name: string) => T | undefined, put: (name: string, v: Value) => void): void {
    const vars = this.putBack?.get(path);
    for (const [name, x] of vars ? [...vars] : []) {
      const t = type(name);
      if (!t) continue;
      let v: Value;
      try {
        v = this.fromInput(x);
      } catch {
        continue;
      }
      if (this.refusal(v, t, path)) continue;
      put(name, v);
      vars!.delete(name);
    }
  }

  private refusal(v: Value, t: T, site: string): Broken | undefined {
    try {
      return this.takes(v, t) ? undefined : { code: "contracts.outside", site };
    } catch (f) {
      return refused(f, site);
    }
  }

  private given(x: Input, site: string): Value | undefined {
    try {
      return this.fromInput(x);
    } catch (f) {
      this.errors.push(f);
      this.refusals.push(refused(f, site));
      return undefined;
    }
  }

  private givenAll(xs: readonly Input[], site: string): Value[] | undefined {
    const out: Value[] = [];
    for (const x of xs) {
      const v = this.given(x, site);
      if (!v) return undefined;
      out.push(v);
    }
    return out;
  }

  private takesArgs(types: readonly T[], args: readonly Value[], site: string): boolean {
    for (const [k, t] of types.entries()) {
      const broken = args[k] && this.refusal(args[k], t, site);
      if (broken) {
        this.refusals.push(broken);
        return false;
      }
    }
    return true;
  }

  /** A value from outside, in the IR's form. */
  fromInput(x: Input): Value {
    return readInput(this, x as ir.Expr);
  }

  /** A value that holds no widgets and no fn, as whoever is outside reads it. */
  plain(v: Value): Input {
    return this.input(v, "", []);
  }

  /** A value as a widget is given it: the IR's form, its widgets nodes and its fns to call. `nodes` and fns are
   * set by `nodes.ts`. */
  input(v: Value, at: string, fns: string[]): Input {
    switch (v.kind) {
      case "list":
        return { list: v.items.map((i, k) => this.input(i.value, `${at}[${k}]`, fns)) };
      case "record":
        return { record: { type: v.ty, fields: Object.fromEntries(v.fields.map(([k, x]) => [k, this.input(x, `${at}.${k}`, fns)])) } };
      case "case":
        if (this.prog.tokens.has(v.ty)) {
          const held = v.values[0]?.[1];
          return held ? { tokval: { type: v.ty, name: v.name, value: this.input(held, at, fns) } } : { token: `${v.ty}.${v.name}` };
        }
        if (!v.values.length) return { case: { enum: v.ty, name: v.name } };
        return { case: { enum: v.ty, name: v.name, values: Object.fromEntries(v.values.map(([k, x]) => [k, this.input(x, `${at}.${k}`, fns)])) } };
      case "opt":
        return v.value === null ? { none: {} } : { some: this.input(v.value, at, fns) };
      case "range":
        return { range: Object.fromEntries(Object.entries({ low: v.low, high: v.high, step: v.step }).flatMap(([k, x]) => (x ? [[k, this.input(x, at, fns)]] : []))) };
      case "nodes":
      case "fn":
        return this.live(v, at, fns);
      default:
        return toIr(v) as Input;
    }
  }

  /** Widgets and fns, as a widget is given them: set by whoever draws the page. */
  live(v: Value, _at: string, _fns: string[]): Input {
    return toIr(v) as Input;
  }

  // ---- values a page's code makes

  closure(f: F, b: Bound[], o: Inst | undefined): Value {
    return { kind: "fn", f, captured: b, owner: o } as unknown as Value;
  }

  sameFn(a: Value, b: Value): boolean {
    const [x, y] = [a as unknown as Closure, b as unknown as Closure];
    return a === b || (x.f === y.f && x.owner === y.owner && this.sameBound(x.captured, y.captured));
  }

  content(nodes: Part[], b: Bound[], o: Inst | undefined, blocks: boolean): Value {
    return { kind: "content", of: { nodes, captured: b, owner: o }, blocks } as unknown as Value;
  }

  /** A view value: the call it is of, its inputs worked out. */
  view(n: { of: { n: string }; sl?: Record<string, Part[]>; ev?: Record<string, Handler> }, args: Entry[], b: Bound[], o: Inst | undefined, at?: string): Value {
    return { kind: "view", widget: n.of.n, of: n.of, args: new Map(args), slots: n.sl ?? {}, events: n.ev ?? {}, exprs: n, captured: b, owner: o, at } as unknown as Value;
  }

  ordered(ty: string, name: string, values: Entry[]): Entry[] {
    return this.orderCaseValues(ty, name, values);
  }

  fieldOf(ty: string, name: string): string {
    return this.caseFields(ty, name)[0] ?? "value";
  }

  bool(v: Value): boolean {
    return v.kind === "bool" && v.b;
  }

  test(v: Value): boolean {
    if (v.kind !== "bool") throw Failure.ir("a condition that isn't a bool");
    return v.b;
  }

  /** `>`: whether `b` is before `a`. */
  after(a: Value, b: Value): boolean {
    return this.before(b, a);
  }

  /** `try`: what a rule refuses is none. */
  tried(f: () => Value): Value {
    try {
      return { kind: "opt", value: f() };
    } catch (e) {
      if (e instanceof Failure && e.kind === "Rule") {
        this.lastRule = e.rule;
        return none();
      }
      throw e;
    }
  }

  changed(base: Value, given: Entry[], checked: boolean): Value {
    return this.with(base, given, checked);
  }
}

/** The widgets drawn inside one: its slots', and those its inputs hold. */
export function childrenOf(d: Drawn): readonly (readonly Drawn[])[] {
  const out: (readonly Drawn[])[] = [...d.slots.values()];
  const walk = (v: Value): void => {
    if (v.kind === "nodes") out.push(v.nodes as unknown as Drawn[]);
    else if (v.kind === "list") for (const i of v.items) walk(i.value);
    else if (v.kind === "record") for (const [, x] of v.fields) walk(x);
    else if (v.kind === "case") for (const [, x] of v.values) walk(x);
    else if (v.kind === "opt" && v.value) walk(v.value);
  };
  for (const v of d.props.values()) walk(v);
  return out;
}

function readInput(m: Kit, x: ir.Expr): Value {
  if ("num" in x || "duration" in x) return number(x);
  if ("text" in x) return { kind: "text", s: x.text };
  if ("bool" in x) return { kind: "bool", b: x.bool };
  if ("list" in x) return m.makeList(x.list.map((y) => readInput(m, y)));
  if ("record" in x) return m.makeRecord(x.record.type, sortedEntries(x.record.fields).map(([k, y]) => [k, readInput(m, y)] as const), false);
  if ("case" in x) return { kind: "case", ty: x.case.enum, name: x.case.name, values: m.ordered(x.case.enum, x.case.name, sortedEntries(x.case.values).map(([k, y]) => [k, readInput(m, y)] as const)) };
  if ("some" in x) return { kind: "opt", value: readInput(m, x.some) };
  if ("none" in x) return none();
  if ("range" in x) return { kind: "range", low: x.range.low ? readInput(m, x.range.low) : null, high: x.range.high ? readInput(m, x.range.high) : null, step: x.range.step ? readInput(m, x.range.step) : null };
  if ("token" in x) {
    const dot = x.token.lastIndexOf(".");
    if (dot < 0) throw Failure.ir("a token without a case");
    return { kind: "case", ty: x.token.slice(0, dot), name: x.token.slice(dot + 1), values: [] };
  }
  if ("tokval" in x) return { kind: "case", ty: x.tokval.type, name: x.tokval.name, values: [[m.fieldOf(x.tokval.type, x.tokval.name), readInput(m, x.tokval.value)]] };
  throw Failure.ir("a value no host gives");
}

function number(x: { num: string } | { duration: string }): Value {
  const s = "num" in x ? x.num : x.duration;
  const n = Num.literal(s);
  if (n === null) throw Failure.ir(`\`${s}\` is not a number`);
  return "num" in x ? { kind: "num", n } : { kind: "duration", n };
}

