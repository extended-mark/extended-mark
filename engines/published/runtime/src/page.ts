// A page running: the lowered IR mounted, redrawn after every change, with every change made
// through a transaction that is checked and undone when it fails (page 6, plan tab 11).
//
// No DOM: a page is drawn by whoever reads its tree (@extended-mark/dom on the web, the test engine in
// checks). Each widget is identified by its path: the sites of the template instances it belongs
// to, each loop row as [position], then its own site.

import type * as ir from "./generated/ir.js";
import { Constants, Failure, Machine, Num, Program, type Binding, type Host, type Outer } from "./eval/index.js";
import { ENGINE_ONLY, exists as native } from "./eval/natives.js";
import { Value, caseOf, none, toIr, type Value as V } from "./eval/value.js";

/** What the host tells the page about where it is read. */
export interface PageValues {
  width?: "NARROW" | "MEDIUM" | "WIDE";
  theme?: "LIGHT" | "DARK";
}

/** How a page is mounted: what the host and the engine give it. */
export interface CoreOptions {
  page?: PageValues;
  /** `$state`'s values, in the IR's form, over its starting ones. */
  state?: Record<string, ir.Expr>;
  /** Each channel's values, in the IR's form. */
  externs?: Record<string, Record<string, ir.Expr>>;
  /** The value fns the engine supplies, by full name. */
  fns?: Record<string, (args: readonly ir.Expr[]) => ir.Expr | null>;
  /** Whether the engine draws an extern widget, by full name. One it doesn't shows its Markdown
   * instead (page 6), and is reported in `missing()`. Left out, every one is drawn. */
  supplies?: (widget: string) => boolean;
  /** How a value is given to the fns and the host: the IR's form. */
  json?: (v: V) => ir.Expr;
  /** Reuse a widget's drawn subtree when nothing it read has changed (on by default). Off
   * draws every widget anew on every change, for comparing the two. */
  memo?: boolean;
}

/** Where a node's values are read: its widget instance, the locals around it, its path as
 * observed (loop rows by position) and its identity (loop rows by their item). */
export interface Env {
  owner: number | undefined;
  bindings: [string, Binding][];
  path: string;
  key?: string;
}

/** A drawn extern widget, as the page asks an engine to draw it. */
export interface Drawn {
  path: string;
  /** Who it is across redraws: a loop row's widgets follow its item, not its position. */
  key: string;
  widget: string;
  props: Map<string, V>;
  slots: Map<string, Drawn[]>;
  /** Events the caller set, each to a handler. */
  events: Map<string, { handler: string; env: Env }>;
  /** `mut` inputs given a variable: where the widget writes. */
  writes: Map<string, { place: ir.Place; env: Env }>;
  levels: ir.Level[];
  /** A widget that failed: why, shown in its place (page 6). `widget` is the one that did. */
  failed?: string;
}

/** What a widget is given: expressions to work out here, or the values of a call made as one. */
type Args = Record<string, ir.Expr> | Map<string, V>;

/** A rule that failed: a `require`'s message, or a code, and where. */
export interface Broken {
  rule?: string;
  code?: string;
  site?: string;
}

/** A drawn widget as a test reads it. */
export interface Observed {
  widget: string;
  props: Record<string, ir.Expr>;
  slots?: Record<string, string[]>;
  failed?: string;
}

export interface Outcome {
  refused: Broken[];
  failures: Broken[];
}

export interface Call {
  call: { name: string; args: ir.Expr[] };
}

interface Instance {
  id: number;
  path: string;
  template: ir.Template;
  inputs: Map<string, V>;
  /** `mut` inputs: the caller's place, read and written live. */
  places: Map<string, { place: ir.Place; env: Env }>;
  cells: Map<string, V>;
  slots: Map<string, { nodes: ir.Node[]; env: Env }>;
  events: Map<string, { handler: string; env: Env }>;
  seen: boolean;
}

/** What a widget's drawing read and made, so a redraw can reuse it while none of it changed. */
interface Frame {
  /** Each outside value read, by key (`s:field`, `x:channel.value`, `c:id:cell`, `o:key`), with
   * its version then. */
  deps: Map<string, number>;
  /** The instances drawn inside it, with their paths: seen again when it is reused. */
  nested: [Instance, string][];
  own: string[];
  missed: string[];
  /** The extern calls drawn inside it, whose memos a reuse keeps. */
  ext: string[];
  /** The loops, branches, places and locals inside it, whose memos a reuse keeps. */
  held: string[];
  /** It failed, or asked the host for a value: drawn anew every time. */
  dirty: boolean;
}

interface Memo extends Frame {
  template: string;
  path: string;
  owner: number | undefined;
  args: V[];
  bindings: [string, Binding][];
  slots: Record<string, ir.Node[]>;
  events: Record<string, string>;
  epoch: number;
  drawn: Drawn[];
}

const NO_ARGS: Record<string, ir.Expr> = {};
const NO_NODES: Record<string, ir.Node[]> = {};
const NO_EVENTS: Record<string, string> = {};

function join(path: string, part: string): string {
  return path ? `${path}/${part}` : part;
}

function newFrame(): Frame {
  return { deps: new Map(), nested: [], own: [], missed: [], ext: [], held: [], dirty: false };
}

/** The values a call made as a value was given. */
function argValues(args: Args): V[] {
  return args instanceof Map ? [...args.keys()].sort().map((k) => args.get(k)!) : [];
}

function failureCode(f: unknown): string {
  return f instanceof Failure ? `failure.${String(f.kind).toLowerCase()}` : "failure.ir";
}

function message(f: unknown): string {
  return f instanceof Error ? f.message : String(f);
}

/** The IR this runtime runs: a whole number of its own, apart from its release. */
export const IR_VERSION = 4;

/** What an IR of another version is told. */
function otherVersion(version: unknown): string {
  const other = typeof version === "number" && version > IR_VERSION;
  return `this page is IR ${version}, this engine reads IR ${IR_VERSION}: ${other ? "use a later engine" : "run xtmd migrate"}`;
}

/** A page running: what `Page`, the host's side of it, is made on. Its tree has the runtime's own values
 * in it, and its changes are told with nothing. */
export class Core {
  readonly module: ir.Module;
  readonly program: Program;
  readonly machine: Machine;
  private readonly templates = new Map<string, ir.Template>();
  private readonly externWidgets = new Map<string, ir.ExternWidget>();
  private readonly handlers = new Map<string, ir.Handler>();
  private readonly hostFns: Set<string>;
  private readonly stateDecl: ir.State | undefined;
  private readonly instances = new Map<number, Instance>();
  /** Instances by identity: rows follow their items, so a row's widget keeps its variables. */
  private readonly byPath = new Map<string, Instance>();
  /** The same instances by the path they were last drawn at. */
  private atPath = new Map<string, Instance>();
  /** Extern widgets' own `mut` values, for inputs the caller gave no variable, by identity. */
  private own = new Map<string, Map<string, V>>();
  private ownSeen = new Set<string>();
  /** Who has been drawn this redraw: content placed twice is two widgets. */
  private claimed = new Set<string>();
  private state = new Map<string, V>();
  private externs = new Map<string, V>();
  private calls: Call[] = [];
  private outcome: Outcome = { refused: [], failures: [] };
  private drawn: Drawn[] = [];
  private readonly listeners = new Set<() => void>();
  /** Each outside value's version, bumped when it is written, and the epoch, bumped when every
   * memo must go (an undo, what the engine supplies). */
  private versions = new Map<string, number>();
  private epoch = 0;
  private memo = new Map<string, Memo>();
  private externMemo = new Map<string, Frame & { node: ir.NodeExtern; path: string; owner: number | undefined; bindings: [string, Binding][]; epoch: number; drawn: Drawn[] }>();
  /** What each loop, branch, place and local of a view gave, with what it read. */
  private nodeMemo = new Map<string, Frame & { node: ir.Node; path: string; owner: number | undefined; bindings: [string, Binding][]; epoch: number; drawn: Drawn[]; value?: V }>();
  private nodeSeen = new Set<string>();
  /** The page's rules that held or didn't, with what they read. */
  private ruleMemo = new Map<ir.Require, { deps: Map<string, number>; epoch: number; holds: boolean }>();
  /** The extern calls drawn this redraw: the memos of the rest are dropped after it. */
  private externSeen = new Set<string>();
  private frames: Frame[] = [];
  private readonly memoOn: boolean;
  /** How many changes came from outside the layout: the reader's, the host's, the page's
   * values. A layout that didn't settle is held until the next one. */
  changes = 0;
  /** Warnings for the host, since it last took them: not failures, the page carries on. */
  private warnings: Broken[] = [];
  /** The `warn` rules that don't hold now, so each is told once as it becomes false. */
  private warns = new Set<string>();
  /** Every failure caught, with its message and stack, for a host's log. */
  readonly errors: unknown[] = [];
  /** Defects switched on, for the test target: each must be caught by a check. */
  /** Whether the engine draws an extern widget. */
  private supplies: (widget: string) => boolean;
  /** Extern widgets drawn as their Markdown because the engine doesn't supply them. */
  private readonly missed = new Set<string>();
  /** A value as whoever the page is given to reads it. */
  readonly json: (v: V) => ir.Expr;
  /** The value fns the engine supplies, by full name: `core/moment.local`. */
  private readonly engineFns: Record<string, (args: readonly ir.Expr[]) => ir.Expr | null>;

  constructor(ir: ir.File | ir.Module, opts: CoreOptions = {}) {
    this.engineFns = opts.fns ?? {};
    this.json = opts.json ?? toIr;
    this.supplies = opts.supplies ?? (() => true);
    this.memoOn = opts.memo ?? true;
    this.module = "ir" in ir ? ir.ir : ir;
    if (this.module.version !== IR_VERSION) throw Failure.ir(otherVersion(this.module.version));
    this.program = Program.fromModule(this.module);
    for (const t of this.module.templates) {
      this.templates.set(t.name, t);
      for (const f of (t.fns ?? [])) this.program.fns.set(f.name, f);
    }
    for (const h of this.module.handlers ?? []) this.handlers.set(h.id, h);
    this.hostFns = new Set(this.module.needs.host);
    let state: ir.State | undefined;
    for (const d of this.module.declarations) {
      const decl = d.declare as Record<string, unknown>;
      if ("$state" in decl) state = decl["$state"] as ir.State;
      if ("extern" in decl) {
        const x = decl.extern as Record<string, unknown>;
        if ("widget" in x) this.externWidgets.set((x.widget as ir.ExternWidget).name, x.widget as ir.ExternWidget);
      }
    }
    this.stateDecl = state;
    // core's fns are the engine's: one it doesn't have is another core's
    const unknown = this.module.needs.core.find((n) => !this.externWidgets.has(n) && !this.supplied(n));
    if (unknown !== undefined) throw Failure.ir(`this page needs \`${unknown}\`, which this engine doesn't have`);
    this.machine = new Machine(this.program, this.host());
    // the page's values, then $state's starting values, then what the host gives
    for (const d of this.module.declarations) {
      const decl = d.declare as Record<string, unknown>;
      if (!("extern" in decl)) continue;
      const x = decl.extern as Record<string, unknown>;
      if (!("channel" in x)) continue;
      const c = x.channel as ir.ExternChannel;
      for (const f of (c.values ?? [])) {
        if (f.default) this.externs.set(`${c.name}.${f.name}`, this.constant(f.default));
      }
    }
    for (const f of state?.fields ?? []) {
      if (f.default) this.state.set(f.name, this.constant(f.default));
    }
    for (const [k, x] of Object.entries(opts.state ?? {})) this.state.set(k, this.constant(x));
    for (const [c, values] of Object.entries(opts.externs ?? {})) {
      for (const [k, x] of Object.entries(values)) this.externs.set(`${c}.${k}`, this.constant(x));
    }
    if (opts.page) this.applyPage(opts.page);
    this.redraw();
  }

  // ---- reading the page

  /** The drawn extern widgets, in order. */
  tree(): Drawn[] {
    return this.drawn;
  }

  /** Every drawn extern widget by its path, as a check observes it. */
  observeTree(): Record<string, Observed> {
    const out: Record<string, Observed> = {};
    const walk = (nodes: readonly Drawn[]) => {
      for (const n of nodes) {
        const props: Record<string, ir.Expr> = {};
        for (const [k, v] of [...n.props.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) {
          if (v.kind !== "fn") props[k] = toIr(v);
        }
        const entry: Observed = { widget: n.widget, props };
        if (n.slots.size) {
          entry.slots = {};
          for (const [s, kids] of n.slots) entry.slots[s] = kids.map((k) => k.path);
        }
        if (n.failed !== undefined) entry.failed = n.failed;
        out[n.path] = entry;
        for (const kids of childrenOf(n)) walk(kids);
      }
    };
    walk(this.drawn);
    return out;
  }

  /** Every `shade` drawn, by path: the fn it was given, by name, and its arguments (plan tab 16,
   * `shaded`). With `samples`, also the colour at the area's two corners and its middle, worked
   * out by the shader itself, as the test engine does. */
  observeShaded(samples = false): Record<string, { shader: string; args: ir.Expr; samples?: { at: ir.Expr; pixel: ir.Expr }[] }> {
    const out: Record<string, { shader: string; args: ir.Expr; samples?: { at: ir.Expr; pixel: ir.Expr }[] }> = {};
    const walk = (nodes: readonly Drawn[]) => {
      for (const n of nodes) {
        if (n.widget === "core/shade") {
          const fn = n.props.get("shader");
          const args = n.props.get("args") ?? none();
          const entry: { shader: string; args: ir.Expr; samples?: { at: ir.Expr; pixel: ir.Expr }[] } = { shader: fn ? shaderName(fn) : "", args: toIr(args) };
          const area = n.props.get("area");
          if (samples && fn && area?.kind === "record") {
            const pt = (name: string) => area.fields.find(([k]) => k === name)?.[1];
            const from = pt("from");
            const to = pt("to");
            const coord = (p: V | undefined, k: string) => (p?.kind === "record" ? p.fields.find(([f]) => f === k)?.[1] : undefined);
            const fx = coord(from, "x"), fy = coord(from, "y"), tx = coord(to, "x"), ty = coord(to, "y");
            if (fx?.kind === "num" && fy?.kind === "num" && tx?.kind === "num" && ty?.kind === "num") {
              const half = Num.literal("0.5")!;
              const point = (x: Num, y: Num): V => ({ kind: "record", ty: "core/point", fields: [["x", { kind: "num", n: x }], ["y", { kind: "num", n: y }]] }) as V;
              const px = { kind: "num", n: tx.n.sub(fx.n).abs().div(Num.int(64)) } as V;
              entry.samples = [point(fx.n, fy.n), point(fx.n.add(tx.n).mul(half), fy.n.add(ty.n).mul(half)), point(tx.n, ty.n)].map((at) => {
                this.machine.resetSteps();
                const pixel = this.machine.apply(fn, [{ value: at }, { value: px }, { value: args }]);
                return { at: toIr(at), pixel: pixel ? toIr(pixel) : { none: {} } };
              });
            }
          }
          out[n.path] = entry;
        }
        for (const kids of childrenOf(n)) walk(kids);
      }
    };
    walk(this.drawn);
    return out;
  }

  /** `$state` as it is now, in the IR's form. */
  observeState(): Record<string, ir.Expr> {
    const out: Record<string, ir.Expr> = {};
    for (const [k, v] of this.state) out[k] = toIr(v);
    return out;
  }

  stateValue(name: string): V | undefined {
    return this.state.get(name);
  }

  /** The extern fns of the host called since the last time they were taken. */
  takeCalls(): Call[] {
    const c = this.calls;
    this.calls = [];
    return c;
  }

  /** Refused writes and failures since the last time they were taken. */
  takeOutcome(): Outcome {
    const o = this.outcome;
    this.outcome = { refused: [], failures: [] };
    return o;
  }

  /** Called after every redraw. */
  onChange(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  // ---- changing it: every change is one transaction, checked, and undone when it fails

  /** A reader's event on a drawn widget. */
  event(path: string, name: string, args: V[] = []): void {
    const node = this.find(path);
    const ev = node?.events.get(name);
    if (!node || !ev) return;
    this.transaction(node.key, () => this.runHandler(ev.handler, ev.env, args));
  }

  /** A drawn widget writing one of its `mut` inputs, as a slider does. */
  write(path: string, input: string, value: V): void {
    this.changes++;
    this.machine.resetSteps();
    const node = this.find(path);
    if (!node) return;
    const w = node.writes.get(input);
    if (!w) {
      // a widget's own value: kept by the page for it, checked against the input's type
      const f = this.externWidgets.get(node.widget)?.inputs?.find((f) => f.name === input && f.mut);
      if (!f) return;
      if (!this.machine.fits(value, f.type)) {
        this.outcome.refused.push({ code: "contracts.outside", site: node.key });
        return;
      }
      if (!this.own.has(node.key)) this.own.set(node.key, new Map());
      this.own.get(node.key)!.set(input, value);
      this.bump(`o:${node.key}`);
      this.redraw();
      return;
    }
    const snap = this.snapshot();
    try {
      this.writePlace(w.place, w.env, value);
      const broken = this.contractBroken(w.place, w.env);
      if (broken) {
        this.restore(snap);
        this.outcome.refused.push({ code: "contracts.outside", site: node.key });
        this.redraw();
        return;
      }
      const rule = this.stateRuleBroken();
      if (rule) {
        this.restore(snap);
        this.outcome.refused.push({ rule });
      }
    } catch (f) {
      this.restore(snap);
      this.outcome.refused.push({ code: "contracts.outside", site: node.key });
    }
    this.redraw();
  }

  /** A layout pass's writes, as one change: a value equal to the one there is no change, and a
   * refused write undoes the pass. Whether the layout has to run again: only when a variable
   * changed, since a widget's own values are read by no Extended Mark code. A spot in a loop over a list
   * made anew at each redraw gets a new identity each time, so its own box never stays. */
  layoutWrite(writes: { path: string; input: string; value: V }[]): boolean {
    this.machine.resetSteps();
    const snap = this.snapshot();
    const ownBefore = new Map([...this.own].map(([k, m]) => [k, new Map(m)]));
    let changed = false;
    let shared = false;
    for (const { path, input, value } of writes) {
      const node = this.find(path);
      if (!node) continue;
      const now = node.props.get(input);
      if (now && this.machine.equal(now, value)) continue;
      const w = node.writes.get(input);
      try {
        if (w) {
          this.writePlace(w.place, w.env, value);
          if (this.contractBroken(w.place, w.env)) throw new Failure("Rule", "contracts.outside");
          shared = true;
        } else {
          if (!this.own.has(node.key)) this.own.set(node.key, new Map());
          this.own.get(node.key)!.set(input, value);
      this.bump(`o:${node.key}`);
        }
        changed = true;
      } catch {
        this.restore(snap);
        this.own = ownBefore;
        this.outcome.refused.push({ code: "contracts.outside", site: node.key });
        return false;
      }
    }
    if (!changed) return false;
    const rule = this.stateRuleBroken();
    if (rule) {
      this.restore(snap);
      this.own = ownBefore;
      this.outcome.refused.push({ rule });
      return false;
    }
    this.redraw();
    return shared;
  }

  /** The warnings since the host last took them: `warn` rules found false (page 2). */
  takeWarnings(): Broken[] {
    // a record's `warn` has no place of its own on the page: told once
    for (const message of this.machine.warned.splice(0)) {
      if (this.warns.has(message)) continue;
      this.warns.add(message);
      this.warnings.push({ rule: message });
    }
    const w = this.warnings;
    this.warnings = [];
    return w;
  }

  /** A `warn` rule, worked out: told once as it becomes false. */
  private warned(site: string, message: string, holds: boolean): void {
    const at = `${site}\n${message}`;
    if (holds) {
      this.warns.delete(at);
    } else if (!this.warns.has(at)) {
      this.warns.add(at);
      this.warnings.push({ rule: message, site });
    }
  }

  /** A widget's rules, with everything they read: a `require` has to hold, and a `warn` is told. */
  private checkRules(rules: ir.Require[], own: Env, site: string): void {
    for (const r of rules) {
      const ok = this.runBody([], r.body, own, []);
      const holds = !(ok && ok.kind === "bool" && !ok.b);
      if (r.warn) this.warned(site, r.message, holds);
      else if (!holds) throw new Failure("Rule", r.message, r.message);
    }
  }

  /** A plane whose layout didn't settle (page 6, failures). */
  layoutFailed(id: string): void {
    if (!this.outcome.failures.some((b) => b.site === id && b.code === "failure.layout")) this.outcome.failures.push({ code: "failure.layout", site: id });
  }

  /** A host's write to `$state`: one change, checked once. */
  set(changes: Record<string, V>): void {
    this.changes++;
    this.machine.resetSteps();
    const snap = this.snapshot();
    const fields = new Map((this.stateDecl?.fields ?? []).map((f) => [f.name, f]));
    for (const [k, v] of Object.entries(changes)) {
      const f = fields.get(k);
      if (!f || !this.ofType(v, f.type) || !this.machine.fits(v, f.type)) {
        this.restore(snap);
        this.outcome.refused.push({ code: "contracts.outside", site: `$state.${k}` });
        return;
      }
      this.state.set(k, v);
      this.bump(`s:${k}`);
    }
    const rule = this.stateRuleBroken();
    if (rule) {
      this.restore(snap);
      this.outcome.refused.push({ rule });
      return;
    }
    this.redraw();
  }

  /** A host calling a fn the document exposes. */
  expose(channel: string, name: string, args: V[] = []): void {
    for (const d of this.module.declarations) {
      const decl = d.declare as Record<string, unknown>;
      if (!("expose" in decl)) continue;
      const e = decl.expose as ir.Expose;
      if (e.channel !== channel) continue;
      const f = e.fns.find((f) => f.name === name);
      if (!f) continue;
      this.transaction(`${channel}.${name}`, () => this.runBody(f.params, f.body, { owner: undefined, bindings: [], path: "" }, args));
      return;
    }
  }

  /** The host giving a channel's values. */
  setExtern(channel: string, values: Record<string, V>): void {
    this.changes++;
    for (const [k, v] of Object.entries(values)) {
      this.externs.set(`${channel}.${k}`, v);
      this.bump(`x:${channel}.${k}`);
    }
    this.redraw();
  }

  /** The page's own values: its width and theme. */
  setPage(page: PageValues): void {
    this.changes++;
    this.applyPage(page);
    this.redraw();
  }

  /** Which extern widgets the engine draws: set by an engine before it first draws the page,
   * as @extended-mark/dom does from its registry. The page is drawn again. */
  setSupplies(supplies: (widget: string) => boolean): void {
    this.supplies = supplies;
    this.epoch++;
    this.missed.clear();
    this.redraw();
  }

  /** The extern widgets this page uses that the engine doesn't supply, each shown as its
   * Markdown: reported when the page is mounted, and kept up to date as it changes. */
  missing(): string[] {
    return [...this.missed].sort();
  }

  /** Whether something gives a fn: the engine, or its own body. */
  private supplied(name: string): boolean {
    return !!this.engineFns[name] || native(name) || ENGINE_ONLY.includes(name) || !!this.program.externs.get(name)?.default?.length;
  }

  /** What the page takes (its IR's `needs`) and nobody gives: the engine's fns and a pack's, and the
   * host's values. The page runs without them until it reads one. */
  lacking(): string[] {
    const { core, packs, host } = this.module.needs;
    const fns = [...core, ...packs.flatMap((p) => p.externs)].filter((n) => this.program.externs.has(n) && !this.engineFns[n] && !native(n) && !this.program.externs.get(n)?.default?.length && !this.hostFns.has(n));
    const values = host.filter((n) => !this.program.externs.has(n) && !this.externs.has(n));
    return [...fns, ...values].sort();
  }

  // ---- the page's Markdown

  /** The page as Markdown, with the values it has now. */
  markdown(): string {
    // set by markdown.ts, which reads the live page
    this.machine.resetSteps();
    return pageMarkdown(this);
  }

  // ---- widgets' own variables, for a snapshot

  /** Every drawn widget's own variables now, by the place it is drawn at (`s0/s4[2]/s1`): a widget
   * written in Extended Mark, its variables; an extern widget, the `mut` inputs no caller gave a variable. */
  observeOwn<T = ir.Expr>(as: (v: V) => T = toIr as (v: V) => T): Record<string, Record<string, T>> {
    const out: Record<string, Record<string, T>> = {};
    for (const [path, inst] of this.atPath) {
      if (inst.cells.size) out[path] = Object.fromEntries([...inst.cells].map(([k, v]) => [k, as(v)]));
    }
    const walk = (nodes: readonly Drawn[]) => {
      for (const n of nodes) {
        const own = [...(this.own.get(n.key) ?? [])].filter(([k]) => !n.writes.has(k));
        if (own.length) out[n.path] = { ...out[n.path], ...Object.fromEntries(own.map(([k, v]) => [k, as(v)])) };
        for (const kids of childrenOf(n)) walk(kids);
      }
    };
    walk(this.drawn);
    return out;
  }

  /** Widgets' own variables put back by place, as one change: each value checked against its
   * variable's type. One that doesn't fit, or whose widget isn't drawn at that place, is left out,
   * and returned as `place.name`. */
  restoreOwn(values: Record<string, Record<string, ir.Expr>>): string[] {
    this.changes++;
    this.machine.resetSteps();
    const nodes = new Map<string, Drawn>();
    const walk = (ns: readonly Drawn[]) => {
      for (const n of ns) {
        nodes.set(n.path, n);
        for (const kids of childrenOf(n)) walk(kids);
      }
    };
    walk(this.drawn);
    const left: string[] = [];
    for (const [path, vars] of Object.entries(values)) {
      const inst = this.atPath.get(path);
      const node = nodes.get(path);
      for (const [name, x] of Object.entries(vars)) {
        let v: V;
        try {
          v = this.value(x);
        } catch {
          left.push(`${path}.${name}`);
          continue;
        }
        const cell = inst?.template.cells?.find((c) => c.name === name);
        if (inst && cell && this.machine.fits(v, cell.type)) {
          inst.cells.set(name, v);
          continue;
        }
        const input = node ? this.externWidgets.get(node.widget)?.inputs?.find((f) => f.name === name && f.mut) : undefined;
        if (node && input && !node.writes.has(name) && this.machine.fits(v, input.type)) {
          if (!this.own.has(node.key)) this.own.set(node.key, new Map());
          this.own.get(node.key)!.set(name, v);
          continue;
        }
        left.push(`${path}.${name}`);
      }
    }
    // every memo read the old values
    this.epoch++;
    this.redraw();
    return left;
  }

  // ---- inside

  private applyPage(page: PageValues): void {
    if (page.width) {
      this.externs.set("core/page.width", caseOf("core/width", page.width));
      this.bump("x:core/page.width");
    }
    if (page.theme) {
      this.externs.set("core/page.theme", caseOf("core/theme", page.theme));
      this.bump("x:core/page.theme");
    }
  }

  /** Whether a value has a type's shape. The host's values come from outside, so their type is
   * checked before their contract: the compiler has proved only the page's own. */
  ofType(v: V, t: ir.Type, depth = 0): boolean {
    let ty = t;
    for (let guard = 0; this.program.aliases.has(ty.name) && guard < 16; guard++) ty = this.program.aliases.get(ty.name)!;
    const arg = ty.args?.[0];
    if (depth > 64) return false;
    switch (ty.name) {
      case "core/number":
      case "core/int":
        return v.kind === "num";
      case "core/text":
        return v.kind === "text";
      case "core/bool":
        return v.kind === "bool";
      case "core/duration":
        return v.kind === "duration";
      case "core/list":
        return v.kind === "list" && (!arg || v.items.every((i) => this.ofType(i.value, arg, depth + 1)));
      case "core/opt":
        return v.kind === "opt" && (v.value === null || !arg || this.ofType(v.value, arg, depth + 1));
      case "core/range":
        return v.kind === "range";
    }
    const rec = this.program.records.get(ty.name);
    if (rec) {
      if (v.kind !== "record" || v.ty !== ty.name) return false;
      return rec.fields.every((f) => {
        const x = v.fields.find(([k]) => k === f.name);
        return x ? this.ofType(x[1], f.type, depth + 1) : false;
      });
    }
    if (this.program.enums.has(ty.name) || this.program.tokens.has(ty.name)) return v.kind === "case" && v.ty === ty.name;
    // a type parameter, or a fn or view, which the host doesn't write
    return true;
  }

  /** A value in the IR's form, as the host and the checks give them. */
  value(x: ir.Expr): V {
    return this.constant(x);
  }

  private constant(x: ir.Expr): V {
    return new Machine(this.program, new Constants()).eval(x);
  }

  private host(): Host {
    const page = this;
    return {
      read(m: Machine, what: Outer, owner?: number): V {
        switch (what.kind) {
          case "state": {
            page.track(`s:${what.name}`);
            const v = page.state.get(what.name);
            if (!v) throw new Failure("Ir", `$state has no field ${what.name}`);
            return v;
          }
          case "extern": {
            page.track(`x:${what.name}`);
            const v = page.externs.get(what.name);
            if (!v) throw new Failure("NotConstant", `${what.name} is given by the host`);
            return v;
          }
          case "let": {
            const v = m.prog.lets.get(what.name);
            if (v) return v;
            const d = page.module.declarations.find((d) => "let" in (d.declare as object) && ((d.declare as { let: ir.LetDecl }).let.name === what.name));
            if (d) return m.eval((d.declare as { let: ir.LetDecl }).let.value);
            throw new Failure("Ir", `no let ${what.name}`);
          }
          case "input": {
            const inst = owner === undefined ? undefined : page.instances.get(owner);
            if (!inst) throw new Failure("Ir", `no input ${what.name} here`);
            const p = inst.places.get(what.name);
            if (p) return page.machine.evalWith(p.env.bindings, p.env.owner, { place: p.place } as ir.Expr);
            // a slot's content reads its caller's inputs inside the widget it is placed in, whose
            // own inputs may not change: the read is what makes that widget be drawn again
            page.track(`i:${owner}:${what.name}`);
            const v = inst.inputs.get(what.name);
            if (!v) throw new Failure("Ir", `no input ${what.name}`);
            return v;
          }
          case "cell": {
            if (owner !== undefined) page.track(`c:${owner}:${what.name}`);
            const v = owner === undefined ? undefined : page.instances.get(owner)?.cells.get(what.name);
            if (!v) throw new Failure("Ir", `no variable ${what.name}`);
            return v;
          }
          case "slot": {
            const inst = owner === undefined ? undefined : page.instances.get(owner);
            if (!inst) throw new Failure("Ir", `no slot ${what.name} here`);
            // read as an input is: whoever reads it is out of date when the caller gives it anew
            page.track(`l:${inst.id}:${what.name}`);
            const s = inst.slots.get(what.name);
            const decl = (inst.template.slots ?? []).find((x) => x.name === what.name);
            if (!s) {
              // only an `opt` slot may be left out (page 3): the runtime makes up no value
              if (decl?.type.name === "core/opt") return none();
              throw new Failure("Ir", `the slot ${what.name} isn't filled`);
            }
            return m.slotValue(decl?.type, s.nodes, s.env.bindings, s.env.owner, { path: s.env.path, key: s.env.key ?? s.env.path });
          }
        }
      },
      write(m: Machine, what: Outer, owner: number | undefined, value: V): void {
        switch (what.kind) {
          case "state": {
            const f = page.stateDecl?.fields.find((f) => f.name === what.name);
            if (f && !m.fits(value, f.type)) throw new Failure("Rule", `$state.${what.name} is outside its contract`);
            page.state.set(what.name, value);
            page.bump(`s:${what.name}`);
            return;
          }
          case "cell": {
            const inst = owner === undefined ? undefined : page.instances.get(owner);
            if (!inst) throw new Failure("Ir", `no variable ${what.name} here`);
            const c = (inst.template.cells ?? []).find((c) => c.name === what.name);
            if (c && !m.fits(value, c.type)) throw new Failure("Rule", `${what.name} is outside its contract`);
            inst.cells.set(what.name, value);
            page.bump(`c:${inst.id}:${what.name}`);
            return;
          }
          case "input": {
            const inst = owner === undefined ? undefined : page.instances.get(owner);
            const p = inst?.places.get(what.name);
            if (!p) throw new Failure("Ir", `${what.name} is not a mut input given a variable`);
            page.writePlace(p.place, p.env, value);
            return;
          }
          default:
            throw new Failure("Ir", `${what.kind} ${what.name} can't be written`);
        }
      },
      callExtern(_m: Machine, name: string, args: V[]): V | null | undefined {
        const own = page.engineFns[name];
        if (own) {
          const out = own(args.map(page.json));
          return out === null ? null : page.constant(out);
        }
        if (!page.hostFns.has(name)) return undefined;
        for (const f of page.frames) f.dirty = true;
        page.calls.push({ call: { name, args: args.map(page.json) } });
        return null;
      },
      invoke(_m: Machine, event: string, args: V[], owner: number | undefined): void {
        const inst = owner === undefined ? undefined : page.instances.get(owner);
        const ev = inst?.events.get(event);
        if (ev) page.runHandler(ev.handler, ev.env, args);
      },
    };
  }

  evalIn(env: Env, e: ir.Expr): V {
    return this.machine.evalWith(env.bindings, env.owner, e);
  }

  private writePlace(place: ir.Place, env: Env, value: V): void {
    this.machine.setPlace(env.bindings, env.owner, place, value);
  }

  /** Whether the variable a place writes now breaks its own contract. */
  private contractBroken(place: ir.Place, env: Env): boolean {
    const root = place.root as Record<string, string>;
    if ("$state" in root && (place.path ?? []).length === 0) {
      const f = this.stateDecl?.fields.find((f) => f.name === root["$state"]);
      const v = this.state.get(root["$state"]);
      return !!f && !!v && !this.machine.fits(v, f.type);
    }
    if ("cell" in root && (place.path ?? []).length === 0 && env.owner !== undefined) {
      const inst = this.instances.get(env.owner);
      const c = inst?.template.cells?.find((c) => c.name === root.cell);
      const v = inst?.cells.get(root.cell);
      return !!c && !!v && !this.machine.fits(v, c.type);
    }
    return false;
  }

  /** The message of the first of `$state`'s rules that no longer holds. */
  private stateRuleBroken(): string | undefined {
    for (const r of this.stateDecl?.requires ?? []) {
      if (!r.warn && !this.stateRule(r)) return r.message;
    }
    return undefined;
  }

  /** Whether one of `$state`'s rules holds: worked out again only when something it read changed. */
  private stateRule(r: ir.Require): boolean {
    const m = this.ruleMemo.get(r);
    if (m && m.epoch === this.epoch && this.current(m.deps)) return m.holds;
    const { value, deps } = this.reading(() => this.runBody([], r.body, { owner: undefined, bindings: [], path: "" }, []), false);
    const holds = !(value && value.kind === "bool" && !value.b);
    this.ruleMemo.set(r, { deps, epoch: this.epoch, holds });
    return holds;
  }

  private runHandler(id: string, env: Env, args: V[]): void {
    const h = this.handlers.get(id);
    if (!h) throw new Failure("Ir", `no handler ${id}`);
    this.runBody(h.params ?? [], h.body, env, args);
  }

  private runBody(params: ir.Param[], body: ir.Stmt[], env: Env, args: V[]): V | undefined {
    const f: V = { kind: "fn", params, body, captured: env.bindings, owner: env.owner } as V;
    return this.machine.apply(f, args.map((value) => ({ value }))) ?? undefined;
  }

  private snapshot() {
    return {
      state: new Map(this.state),
      cells: new Map([...this.instances].map(([id, i]) => [id, new Map(i.cells)])),
      calls: this.calls.length,
    };
  }

  private restore(s: ReturnType<Core["snapshot"]>): void {
    this.epoch++;
    this.state = s.state;
    for (const [id, cells] of s.cells) {
      const i = this.instances.get(id);
      if (i) i.cells = cells;
    }
    this.calls.length = s.calls;
  }

  /** A change from outside: done whole, with `$state`'s rules after it, or undone. */
  private transaction(site: string, run: () => void): void {
    this.changes++;
    const snap = this.snapshot();
    try {
      this.machine.resetSteps();
      run();
      const rule = this.stateRuleBroken();
      if (rule) throw new Failure("Rule", rule, rule);
    } catch (f) {
      this.errors.push(f);
      this.restore(snap);
      if (f instanceof Failure && f.kind === "Rule" && f.rule) this.outcome.failures.push({ rule: f.rule, site });
      else this.outcome.failures.push({ code: failureCode(f), site });
    }
    this.redraw();
  }

  /** The widget drawn at a path now, if there is one. */
  find(path: string): Drawn | undefined {
    const walk = (nodes: readonly Drawn[]): Drawn | undefined => {
      for (const n of nodes) {
        if (n.path === path) return n;
        for (const kids of childrenOf(n)) {
          const f = walk(kids);
          if (f) return f;
        }
      }
      return undefined;
    };
    return walk(this.drawn);
  }

  // ---- drawing

  /** Draws the whole page again: once per change. */
  redraw(): void {
    // each redraw has the step limit to itself, as each event does: steps never pile up across changes
    this.machine.resetSteps();
    for (const i of this.instances.values()) i.seen = false;
    this.missed.clear();
    this.claimed.clear();
    this.atPath = new Map();
    this.ownSeen = new Set();
    this.externSeen = new Set();
    this.nodeSeen = new Set();
    for (const r of this.stateDecl?.requires ?? []) if (r.warn) this.warned("$state", r.message, this.stateRule(r));
    const doc = this.templates.get("doc");
    this.drawn = doc ? this.drawNodes(doc.view, { owner: undefined, bindings: [], path: "" }) : [];
    // a widget that left the page leaves with its variables
    for (const [id, i] of [...this.instances]) {
      if (!i.seen) {
        this.instances.delete(id);
        // a widget of another kind may have taken its key this redraw: that one stays
        if (this.byPath.get(i.path) === i) {
          this.byPath.delete(i.path);
          this.memo.delete(i.path);
        }
      }
    }
    for (const k of [...this.own.keys()]) if (!this.ownSeen.has(k)) this.own.delete(k);
    for (const k of [...this.externMemo.keys()]) if (!this.externSeen.has(k)) this.externMemo.delete(k);
    for (const k of [...this.nodeMemo.keys()]) if (!this.nodeSeen.has(k)) this.nodeMemo.delete(k);
    for (const l of this.listeners) l();
  }

  pickBranch(n: ir.NodeIf, env: Env): { nodes: ir.Node[]; env: Env } | undefined {
    for (const b of n.branches) {
      const w = b.when as Record<string, unknown>;
      if ("cond" in w) {
        const v = this.evalIn(env, w.cond as ir.Expr);
        if (v.kind === "bool" && v.b) return { nodes: b.then, env };
      } else {
        const opened = this.machine.openCond(env.bindings, env.owner, w.open as ir.CondOpen);
        if (opened) return { nodes: b.then, env: { ...env, bindings: [...env.bindings, [(w.open as ir.CondOpen).name, opened]] } };
      }
    }
    return { nodes: n.else ?? [], env };
  }

  drawNodes(nodes: ir.Node[], env0: Env): Drawn[] {
    let env = env0;
    const out: Drawn[] = [];
    for (const n of nodes) {
      if ("declare" in n) {
        const l = n.declare.let;
        const at = env;
        const m = this.hold(n, `let ${l.name}`, at, () => ({ drawn: [], value: this.evalIn(at, l.value) }));
        env = { ...env, bindings: [...env.bindings, [l.name, { value: m.value! }]] };
      } else if ("extern" in n || "use" in n) {
        out.push(...this.drawNode(n, env));
      } else {
        const site = "for" in n ? n.for.site : "if" in n ? n.if.site : "match" in n ? n.match.site : n.place.site;
        const at = env;
        out.push(...this.hold(n, site, at, () => ({ drawn: this.drawNode(n, at) })).drawn);
      }
    }
    return out;
  }

  /** What a loop, a branch, a place or a local gives: as it was, when it is the same one with the same
   * locals around it and nothing it read has changed. */
  private hold(node: ir.Node, site: string, env: Env, work: () => { drawn: Drawn[]; value?: V }): { drawn: Drawn[]; value?: V } {
    const key = `${env.key ?? env.path}|${site}`;
    // the same content placed twice in one redraw is two
    const twice = this.nodeSeen.has(key);
    this.nodeSeen.add(key);
    for (const f of this.frames) f.held.push(key);
    const m = twice ? undefined : this.nodeMemo.get(key);
    if (this.memoOn && m && m.node === node && m.epoch === this.epoch && m.path === env.path && m.owner === env.owner && this.current(m.deps) && this.sameBindings(m.bindings, env.bindings)) {
      this.replay(m);
      return m;
    }
    const frame: Frame = newFrame();
    this.frames.push(frame);
    let made: { drawn: Drawn[]; value?: V };
    try {
      made = work();
    } finally {
      this.frames.pop();
    }
    if (this.memoOn && !frame.dirty && !twice) this.nodeMemo.set(key, { ...frame, node, path: env.path, owner: env.owner, bindings: env.bindings, epoch: this.epoch, ...made });
    else if (!twice) this.nodeMemo.delete(key);
    return made;
  }

  private drawNode(n: ir.Node, env: Env): Drawn[] {
    const out: Drawn[] = [];
    {
      if ("for" in n) {
        const rows = this.machine.loopRows(env.bindings, env.owner, n.for);
        rows.forEach((row, k) =>
          out.push(...this.drawNodes(n.for.body, { owner: env.owner, bindings: [...env.bindings, ...row.bindings], path: join(env.path, `${n.for.site}[${k}]`), key: join(env.key ?? env.path, `${n.for.site}[${row.key}]`) })),
        );
      } else if ("if" in n) {
        const b = this.pickBranch(n.if, env);
        if (b) out.push(...this.drawNodes(b.nodes, b.env));
      } else if ("match" in n) {
        const v = this.evalIn(env, n.match.value);
        let done = false;
        for (const c of n.match.cases) {
          const binds = this.matches(v, c.when);
          if (binds) {
            out.push(...this.drawNodes(c.then, { ...env, bindings: [...env.bindings, ...binds] }));
            done = true;
            break;
          }
        }
        if (!done) out.push(...this.drawNodes(n.match.else ?? [], env));
      } else if ("place" in n) {
        const v = this.evalIn(env, n.place.view);
        if (n.place.sentence) checkSentence(v);
        out.push(...this.drawView(v, join(env.path, n.place.site), env, join(env.key ?? env.path, n.place.site)));
      } else if ("extern" in n) {
        out.push(...this.drawExtern(n.extern, env));
      } else if ("use" in n) {
        out.push(...this.drawUse(n.use.name, join(env.path, n.use.site), n.use.args ?? NO_ARGS, n.use.slots ?? NO_NODES, n.use.events ?? NO_EVENTS, env, n.use.levels, join(env.key ?? env.path, n.use.site)));
      }
    }
    return out;
  }

  matches(v: V, patterns: ir.Pattern[]): [string, Binding][] | undefined {
    for (const p of patterns) {
      const q = p as Record<string, unknown>;
      if ("case" in q) {
        const pc = q.case as ir.PatternCase;
        if (v.kind === "case" && v.name === pc.name) return (pc.binds ?? []).map((b, k) => [b, { value: v.values[k]?.[1] ?? none() }]);
        if (v.kind === "opt" && pc.name === "SOME" && v.value) return (pc.binds ?? []).slice(0, 1).map((b) => [b, { value: v.value! }]);
        if (v.kind === "opt" && pc.name === "NONE" && !v.value) return [];
      } else if ("num" in q && v.kind === "num" && this.machine.equal(v, this.constant({ num: q.num } as ir.Expr))) return [];
      else if ("text" in q && v.kind === "text" && v.s === q.text) return [];
      else if ("bool" in q && v.kind === "bool" && v.b === q.bool) return [];
    }
    return undefined;
  }

  /** An extern widget's call: reused from the last redraw when it is the same call at the same
   * place, with the same locals around it, and nothing it read has changed. */
  private drawExtern(x: ir.NodeExtern & { given?: ReadonlyMap<string, V> }, env: Env, path: string = join(env.path, x.site), as: string = join(env.key ?? env.path, x.site)): Drawn[] {
    const key = this.claim(as);
    const m = this.externMemo.get(key);
    if (this.memoOn && m && m.node === x && m.epoch === this.epoch && m.path === path && m.owner === env.owner && this.current(m.deps) && this.sameBindings(m.bindings, env.bindings)) {
      this.replay(m);
      this.externSeen.add(key);
      for (const f of this.frames) f.ext.push(key);
      return m.drawn;
    }
    const frame = newFrame();
    this.frames.push(frame);
    let drawn: Drawn[];
    try {
      drawn = this.drawExternNow(x, env, path, key);
    } finally {
      this.frames.pop();
    }
    if (this.memoOn && !frame.dirty) this.externMemo.set(key, { ...frame, node: x, path, owner: env.owner, bindings: env.bindings, epoch: this.epoch, drawn });
    else this.externMemo.delete(key);
    this.externSeen.add(key);
    for (const f of this.frames) f.ext.push(key);
    return drawn;
  }

  /** Who a widget is, this redraw: the place it was written, or that and which time it is placed. */
  private claim(key: string): string {
    let id = key;
    for (let n = 2; this.claimed.has(id); n++) id = `${key}~${n}`;
    this.claimed.add(id);
    return id;
  }

  /** What a reused subtree did when it was drawn: its instances are seen again, and every widget
   * being drawn around it is told what it read. */
  private replay(m: Frame): void {
    for (const [inst, p] of m.nested) {
      inst.seen = true;
      this.claimed.add(inst.path);
      this.atPath.set(p, inst);
    }
    for (const k of m.own) this.ownSeen.add(k);
    for (const n of m.missed) this.missed.add(n);
    for (const k of m.ext) {
      this.externSeen.add(k);
      this.claimed.add(k);
    }
    for (const k of m.held) this.nodeSeen.add(k);
    for (const f of this.frames) {
      for (const [k, v] of m.deps) if (!f.deps.has(k)) f.deps.set(k, v);
      f.nested.push(...m.nested);
      f.own.push(...m.own);
      f.missed.push(...m.missed);
      f.ext.push(...m.ext);
      f.held.push(...m.held);
    }
  }

  private drawExternNow(x: ir.NodeExtern & { given?: ReadonlyMap<string, V> }, env: Env, path: string, key: string): Drawn[] {
    const props = x.given ? new Map(x.given) : new Map<string, V>();
    if (!x.given) for (const [k, e] of Object.entries(x.props ?? {})) props.set(k, this.evalIn(env, e));
    const writes = new Map<string, { place: ir.Place; env: Env }>();
    for (const [k, p] of Object.entries(x.writes ?? {})) {
      props.set(k, this.machine.evalWith(env.bindings, env.owner, { place: p } as ir.Expr));
      writes.set(k, { place: p, env });
    }
    const w = this.externWidgets.get(x.name);
    for (const f of w?.inputs ?? []) {
      if (!props.has(f.name) && f.default) props.set(f.name, this.evalIn(env, f.default));
    }
    // a widget the engine doesn't draw shows its Markdown instead (page 6), drawn with the
    // blocks the engine does: its markdown block, in an instance holding its inputs
    if (w && !this.supplies(x.name)) {
      this.missed.add(x.name);
      for (const f of this.frames) f.missed.push(x.name);
      const inst = this.enterExtern(w, path, props, x.slots ?? {}, env);
      try {
        return this.drawNodes(w.markdown ?? [], { ...inst, path, key });
      } finally {
        this.leave(inst);
      }
    }
    // what it wrote to its own `mut` inputs stands until the caller gives them a variable
    this.track(`o:${key}`);
    const own = this.own.get(key);
    if (own) {
      this.ownSeen.add(key);
      for (const f of this.frames) f.own.push(key);
      for (const [k, v] of own) if (!writes.has(k)) props.set(k, v);
    }
    const slots = new Map<string, Drawn[]>();
    for (const [s, nodes] of Object.entries(x.slots ?? {})) slots.set(s, this.drawNodes(nodes, env));
    if (w?.requires?.length) {
      const inst = this.enterExtern(w, path, props, x.slots ?? {}, env);
      try {
        this.checkRules(w.requires, inst, path);
      } finally {
        this.leave(inst);
      }
    }
    this.drawInputs(w, props, path, key);
    const events = new Map<string, { handler: string; env: Env }>();
    for (const [e, h] of Object.entries(x.events ?? {})) events.set(e, { handler: h, env });
    return [{ path, key, widget: x.name, props, slots, events, writes, levels: x.levels ?? [] }];
  }

  /** The calls an input holds, drawn: a rich_text's and a view's, wherever in its value they are. */
  private drawInputs(w: ir.ExternWidget | undefined, props: Map<string, V>, path: string, key: string): void {
    const walk = (v: V, at: string, as: string): V => {
      switch (v.kind) {
        case "content":
        case "view":
          return { kind: "nodes", nodes: this.drawView(v, at, { owner: undefined, bindings: [], path: at }, as) };
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
    const sentences = (v: V): void => {
      if (v.kind === "list") v.items.forEach((i) => sentences(i.value));
      else checkSentence(v);
    };
    for (const [k, v] of props) {
      if (w?.piece && takesSentence(w.piece, k)) sentences(v);
      const x = walk(v, join(path, k), join(key, k));
      if (x !== v) props.set(k, x);
    }
  }

  /** A widget written in Extended Mark: its instance, kept by its path, and its view. */
  private drawUse(name: string, path: string, args: Args, slots: Record<string, ir.Node[]>, events: Record<string, string>, env: Env, levels: ir.Level[] = [], key: string = path): Drawn[] {
    const t = this.templates.get(name);
    if (!t) {
      // an extern widget held as a view value
      return this.drawExtern({ name, site: "", props: {}, given: args as ReadonlyMap<string, V>, slots, events, writes: {}, levels }, env, path, key);
    }
    key = this.claim(key);
    // a failure working out whether it can be reused is left to drawing it, so it fails there,
    // as it would with no reuse at all
    let reused: Drawn[] | undefined;
    try {
      reused = this.reuse(name, path, key, args, slots, events, env);
    } catch {
      reused = undefined;
    }
    if (reused) return reused;
    const frame = newFrame();
    this.frames.push(frame);
    let drawn: Drawn[];
    try {
      const own = this.enterUse(t, path, args, slots, events, env, key);
      drawn = this.drawNodes(t.view, own);
    } catch (f) {
      drawn = [this.failed(name, path, key, f)];
    } finally {
      this.frames.pop();
    }
    const inst = this.byPath.get(key);
    if (this.memoOn && !frame.dirty && inst) {
      this.memo.set(key, { ...frame, template: name, path, owner: env.owner, args: argValues(args), bindings: env.bindings, slots, events, epoch: this.epoch, drawn });
    } else {
      this.memo.delete(key);
    }
    return drawn;
  }

  // ---- reusing what a widget drew (plan tab 2: only the bindings that read what changed)

  /** A read of an outside value, for every widget being drawn. */
  track(key: string): void {
    if (!this.frames.length) return;
    const v = this.versions.get(key) ?? 0;
    for (const f of this.frames) if (!f.deps.has(key)) f.deps.set(key, v);
  }

  /** An outside value written: every memo that read it is out of date. */
  bump(key: string): void {
    this.versions.set(key, (this.versions.get(key) ?? 0) + 1);
  }

  /** What a fn's calls read, for whoever holds what they gave. */
  reading<T>(f: () => T, fresh = true): { value: T; deps: Map<string, number> } {
    const frame = newFrame();
    this.frames.push(frame);
    try {
      if (fresh) this.machine.resetSteps();
      return { value: f(), deps: frame.deps };
    } finally {
      this.frames.pop();
    }
  }

  /** Whether nothing read has changed since. */
  current(deps: ReadonlyMap<string, number>): boolean {
    for (const [k, v] of deps) if ((this.versions.get(k) ?? 0) !== v) return false;
    return true;
  }

  /** Whether two fns are the same code with the same values around it. */
  sameFn(a: V & { kind: "fn" }, b: V & { kind: "fn" }): boolean {
    return a === b || (a.body === b.body && a.owner === b.owner && this.sameBindings(a.captured as [string, Binding][], b.captured as [string, Binding][]));
  }

  /** The widget's subtree from the last redraw, when it is called the same way at the same place
   * and nothing it read has changed since: its instances, own values and missing widgets are
   * seen again, and every widget around it is told what it read. */
  private reuse(name: string, path: string, key: string, args: Args, slots: Record<string, ir.Node[]>, events: Record<string, string>, env: Env): Drawn[] | undefined {
    const m = this.memo.get(key);
    if (!this.memoOn || !m || m.epoch !== this.epoch || m.template !== name || m.path !== path || m.owner !== env.owner) return undefined;
    if (!sameRecord(m.slots, slots) || !sameRecord(m.events, events)) return undefined;
    for (const [k, v] of m.deps) if ((this.versions.get(k) ?? 0) !== v) return undefined;
    if (!this.sameBindings(m.bindings, env.bindings)) return undefined;
    // an input worked out here is the same while what it read and the locals around it are
    const now = argValues(args);
    if (now.length !== m.args.length || now.some((v, k) => !this.machine.equal(v, m.args[k]))) return undefined;
    this.replay(m);
    return m.drawn;
  }

  private sameBindings(a: [string, Binding][], b: [string, Binding][]): boolean {
    if (a === b) return true;
    if (a.length !== b.length) return false;
    return a.every(([n, x], k) => {
      const [m, y] = b[k];
      if (n !== m) return false;
      if ("value" in x && "value" in y) return x.value === y.value || this.machine.equal(x.value, y.value);
      if ("ref" in x && "ref" in y) return JSON.stringify(x.ref) === JSON.stringify(y.ref);
      return false;
    });
  }

  /** A widget that failed: reported once, and in its place (page 6). */
  private failed(name: string, path: string, key: string, f: unknown): Drawn {
    for (const fr of this.frames) fr.dirty = true;
    const broken: Broken = f instanceof Failure && f.kind === "Rule" && f.rule ? { rule: f.rule, site: key } : { code: failureCode(f), site: key };
    if (!this.outcome.failures.some((b) => b.site === key && b.code === broken.code && b.rule === broken.rule)) this.outcome.failures.push(broken);
    return { path, key, widget: name, props: new Map(), slots: new Map(), events: new Map(), writes: new Map(), levels: [], failed: message(f) };
  }

  /** A widget's instance at a path, made or kept, with its inputs, slots and events from this
   * call: the env its own code runs in. Its rules have to hold. */
  enterUse(t: ir.Template, path: string, args: Args, slots: Record<string, ir.Node[]>, events: Record<string, string>, env: Env, key: string = path): Env {
    let inst = this.byPath.get(key);
    const fresh = !inst || inst.template.name !== t.name;
    if (fresh) {
      const id = this.machine.newId();
      inst = { id, path: key, template: t, inputs: new Map(), places: new Map(), cells: new Map(), slots: new Map(), events: new Map(), seen: true };
      this.instances.set(id, inst);
      this.byPath.set(key, inst);
    }
    const it = inst!;
    it.seen = true;
    this.atPath.set(path, it);
    for (const f of this.frames) f.nested.push([it, path]);
    const was = it.inputs;
    it.places.clear();
    it.inputs = new Map();
    if (args instanceof Map) for (const [k, v] of args) it.inputs.set(k, v);
    else for (const [k, e] of Object.entries(args)) {
      if ("place" in e) it.places.set(k, { place: (e as { place: ir.Place }).place, env });
      else it.inputs.set(k, this.evalIn(env, e));
    }
    const own: Env = { owner: it.id, bindings: [], path, key };
    for (const f of (t.inputs ?? [])) {
      if (!it.inputs.has(f.name) && !it.places.has(f.name)) it.inputs.set(f.name, f.default ? this.evalIn(own, f.default) : none());
    }
    // an input given a new value: whatever read it, in the widget's slots too, is out of date
    for (const [k, v] of it.inputs) {
      const before = was.get(k);
      if (!fresh && (!before || !this.machine.equal(before, v))) this.bump(`i:${it.id}:${k}`);
    }
    // a slot given new content, or the same content with other values around it: the same
    const held = new Map(it.slots);
    it.slots.clear();
    for (const [s, nodes] of Object.entries(slots)) it.slots.set(s, { nodes, env });
    if (!fresh) {
      for (const s of new Set([...held.keys(), ...it.slots.keys()])) {
        const [a, b] = [held.get(s), it.slots.get(s)];
        const same = a && b && a.nodes === b.nodes && a.env.owner === b.env.owner && a.env.path === b.env.path && this.sameBindings(a.env.bindings, b.env.bindings);
        if (!same) this.bump(`l:${it.id}:${s}`);
      }
    }
    it.events.clear();
    for (const [e, h] of Object.entries(events)) it.events.set(e, { handler: h, env });
    if (fresh) for (const c of (t.cells ?? [])) it.cells.set(c.name, c.default ? this.evalIn(own, c.default) : none());
    this.checkRules(t.requires ?? [], own, path);
    return own;
  }

  /** An extern widget's instance, for its markdown block: its inputs' values and its slots. */
  enterExtern(w: ir.ExternWidget, path: string, props: Map<string, V>, slots: Record<string, ir.Node[]>, env: Env): Env {
    const t = { name: w.name, generics: [], inputs: w.inputs, slots: w.slots, events: w.events, cells: [], requires: [], fns: [], view: [], markdown: w.markdown, levels: w.levels } as unknown as ir.Template;
    const id = this.machine.newId();
    const inst: Instance = { id, path: `${path}#md`, template: t, inputs: new Map(props), places: new Map(), cells: new Map(), slots: new Map(), events: new Map(), seen: true };
    for (const f of (w.inputs ?? [])) {
      if (!inst.inputs.has(f.name)) inst.inputs.set(f.name, f.default ? this.evalIn(env, f.default) : none());
    }
    for (const [s, nodes] of Object.entries(slots)) inst.slots.set(s, { nodes, env });
    this.instances.set(id, inst);
    return { owner: id, bindings: [], path };
  }

  /** Drops an instance made only to write Markdown. */
  leave(env: Env): void {
    const i = env.owner === undefined ? undefined : this.instances.get(env.owner);
    if (i && i.path.endsWith("#md")) this.instances.delete(i.id);
  }

  templateOf(name: string): ir.Template | undefined {
    return this.templates.get(name);
  }

  externWidget(name: string): ir.ExternWidget | undefined {
    return this.externWidgets.get(name);
  }

  /** Whether a drawn widget's event runs anything. A handler that only passes the event on, as
   * `column` gives its `flex` `click = fn() { invoke click(); }`, counts only when the caller of
   * the widget it passes it to set that event in turn: so an engine makes only what does something
   * clickable. */
  listens(d: Drawn, event: string): boolean {
    let ev = d.events.get(event);
    for (let depth = 0; ev && depth < 64; depth++) {
      const h = this.handlers.get(ev.handler);
      const only = h && h.body.length === 1 ? ((h.body[0] as Record<string, unknown>).invoke as { event: string } | undefined) : undefined;
      if (!only) return !!h;
      const inst = ev.env.owner === undefined ? undefined : this.instances.get(ev.env.owner);
      ev = inst?.events.get(only.event);
    }
    return false;
  }

  /** The type of the variable a drawn widget's `mut` input writes, its contract included: where a
   * slider's ends and step come from (page 5). The input's own type when the caller gave no
   * variable; undefined for a local, whose type the IR doesn't keep. */
  writeType(d: Drawn, input: string): ir.Type | undefined {
    const w = d.writes.get(input);
    if (!w) return this.externWidgets.get(d.widget)?.inputs?.find((f) => f.name === input)?.type;
    const root = w.place.root as Record<string, string>;
    const inst = w.env.owner === undefined ? undefined : this.instances.get(w.env.owner);
    let t: ir.Type | undefined;
    if ("$state" in root) t = this.stateDecl?.fields.find((f) => f.name === root["$state"])?.type;
    else if ("cell" in root) t = inst?.template.cells?.find((c) => c.name === root.cell)?.type;
    else if ("input" in root) t = inst?.template.inputs?.find((f) => f.name === root.input)?.type;
    for (const s of (w.place.path ?? [])) {
      let base = t;
      for (let k = 0; base && k < 16 && this.program.aliases.has(base.name); k++) base = this.program.aliases.get(base.name);
      if (!base) return undefined;
      // a record's field, or a list's item
      if ("field" in s) t = (base.fields ?? this.program.records.get(base.name)?.fields)?.find((f) => f.name === s.field.name)?.type;
      else t = base.args?.[0];
    }
    return t;
  }

  /** Each row of a loop with the locals it binds, as drawing walks it. */
  rows(n: ir.NodeFor, env: Env): [string, Binding][][] {
    return this.machine.loopRows(env.bindings, env.owner, n).map((r) => r.bindings);
  }

  /** The same rows with their keys: a `mut for` row's is its item's identity. */
  keyedRows(n: ir.NodeFor, env: Env): { key: string; bindings: [string, Binding][] }[] {
    return this.machine.loopRows(env.bindings, env.owner, n).map((r, k) => ({ key: String(r.key), bindings: r.bindings }));
  }

  /** A view held in a value: a call made as one, or calls. */
  drawView(v: V, path: string, env: Env, key: string = path): Drawn[] {
    if (v.kind === "opt") return v.value ? this.drawView(v.value, path, env, key) : [];
    if (v.kind === "list") return v.items.flatMap((i, k) => this.drawView(i.value, `${path}[${k}]`, env, `${key}[${i.id}]`));
    if (v.kind === "content") {
      if ("parts" in v.of) return v.of.parts.flatMap((p, k) => this.drawView(p, join(path, `j${k}`), env, join(key, `j${k}`)));
      const c = v.of;
      return this.drawNodes(c.nodes as ir.Node[], { owner: c.owner, bindings: c.captured as [string, Binding][], path: c.at?.path ?? path, key: c.at?.key ?? key });
    }
    if (v.kind !== "view") return [];
    const who = v.at ?? key;
    const made: Env = { owner: v.owner, bindings: [...v.captured], path, key: who };
    return this.drawUse(v.widget, path, v.args as Map<string, V>, v.slots as Record<string, ir.Node[]>, v.events as Record<string, string>, made, [], who);
  }

  /** The live instance at a path, for the Markdown writer. */
  instanceAt(path: string): { id: number; cells: Map<string, V> } | undefined {
    return this.atPath.get(path);
  }

  stateNow(): Map<string, V> {
    return this.state;
  }

  externsNow(): Map<string, V> {
    return this.externs;
  }
}

/** A shader's name: a named fn is lowered to a lambda that calls it with its own parameters. */
function shaderName(f: V): string {
  if (f.kind !== "fn" || f.body.length !== 1) return "";
  const ret = (f.body[0] as { return?: { value?: ir.Expr } }).return?.value as { call?: ir.Call } | undefined;
  const call = ret?.call;
  if (!call) return "";
  const passes = (call.args ?? []).length === f.params.length && (call.args ?? []).every((a, k) => (a as { get?: { local?: string } }).get?.local === f.params[k].name);
  return passes ? call.name : "";
}

function sameRecord<T>(a: Record<string, T>, b: Record<string, T>): boolean {
  if (a === b) return true;
  const ka = Object.keys(a);
  return ka.length === Object.keys(b).length && ka.every((k) => a[k] === b[k]);
}

/** The widgets drawn inside one: its slots', and those its inputs hold. */
export function childrenOf(d: Drawn): (readonly Drawn[])[] {
  const out: (readonly Drawn[])[] = [...d.slots.values()];
  const walk = (v: V): void => {
    if (v.kind === "nodes") out.push(v.nodes);
    else if (v.kind === "list") for (const i of v.items) walk(i.value);
    else if (v.kind === "record") for (const [, x] of v.fields) walk(x);
    else if (v.kind === "case") for (const [, x] of v.values) walk(x);
    else if (v.kind === "opt" && v.value) walk(v.value);
  };
  for (const v of d.props.values()) walk(v);
  return out;
}

/** Whether a piece of Markdown's input or slot holds a sentence, and no blocks. */
export function takesSentence(piece: string, part: string): boolean {
  switch (piece) {
    case "paragraph":
    case "heading":
      return part === "value";
    case "table":
      return part === "head" || part === "rows";
    case "bold":
    case "italic":
    case "strike":
    case "link":
      return part === "body";
    default:
      return false;
  }
}

/** A sentence goes here, so a rich_text of blocks doesn't. */
export function checkSentence(v: V): void {
  if (v.kind === "opt" && v.value) checkSentence(v.value);
  else if (v.kind === "content" && v.blocks) throw new Failure("Ir", "a sentence goes here, and this rich_text is blocks");
}

// the Markdown writer, set by markdown.ts
let pageMarkdown: (p: Core) => string = () => "";
export function setMarkdownWriter(w: (p: Core) => string): void {
  pageMarkdown = w;
}
