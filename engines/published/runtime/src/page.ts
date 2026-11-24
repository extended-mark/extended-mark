// A page running: the lowered IR mounted, redrawn after every change, with every change made
// through a transaction that is checked and undone when it fails (page 6, plan tab 11).
//
// No DOM: a page is drawn by whoever reads its tree (@extended-mark/dom on the web, the test engine in
// checks). Each widget is identified by its path: the sites of the template instances it belongs
// to, each loop row as [position], then its own site.

import type * as ir from "./generated/ir.js";
import { Constants, Failure, Machine, Num, Program, type Binding, type Host, type Outer, type Place, type Step } from "./eval/index.js";
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
  /** `$state`'s values, in the IR's form, over its starting ones: checked as a host's write is, and what
   * doesn't fit is in the outcome and keeps its starting value. */
  state?: Record<string, ir.Expr>;
  /** The page's data to start from, as the host's `data()` gave it: `$state` as one checked write, refused whole, and
   * then widgets' own variables, each where it fits. The page is drawn once, with them. */
  data?: { state?: Record<string, ir.Expr>; own?: Record<string, Record<string, ir.Expr>> };
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
  /** The last redraw it is drawn in. */
  seen: number;
}

/** What a widget's drawing read and made, so a redraw can reuse it while none of it changed. */
/** What drawing one part of the page read and met, itself: what is inside it has its own, in `kids`. */
interface Part {
  place: Place;
  was: V;
  /** What the place is in, and what was read to get there, with their versions: while they are as they
   * were, so is it. */
  deps: Map<string, number>;
}

class Frame {
  /** Each outside value read, by key (`s:field`, `x:channel.value`, `c:id:cell`, `o:key`), with
   * its version then. */
  deps: Map<string, number> | undefined;
  /** Each part of a value read at its place, an item of a list among them, and what was there: another
   * part that changes is nothing to it. */
  parts: Part[] | undefined;
  kids: Frame[] | undefined;
  /** The instances drawn here, with their paths: seen again when it is reused. */
  nested: [Instance, string][] | undefined;
  own: string[] | undefined;
  missed: string[] | undefined;
  /** It failed, or asked the host for a value: drawn anew every time. */
  dirty = false;
  /** The last redraw it is a part of. */
  seen = 0;
  /** When everything it read was last found as it was; -1 once something wasn't, which it never is again. */
  fresh = 0;
}

interface Memo {
  frame: Frame;
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

/** The slots, events and writes of a widget with none: never written to. */
const NONE = new Map<string, never>() as Map<string, any>;
const NO_ARGS: Record<string, ir.Expr> = {};
const NO_NODES: Record<string, ir.Node[]> = {};
const NO_EVENTS: Record<string, string> = {};

/** Whether a value is the one it was: an opt is made anew each time it is read. */
function sameNow(a: V, b: V): boolean {
  return a === b || (a.kind === "opt" && b.kind === "opt" && (a.value === null || b.value === null ? a.value === b.value : sameNow(a.value, b.value)));
}

/** Whether two references stand for the same place. */
function sameRef(a: Place, b: Place): boolean {
  if (a === b) return true;
  const [p, q] = [a.root, b.root];
  const root = p.kind === "local" ? q.kind === "local" && p.frame === q.frame && p.name === q.name : q.kind === "outer" && p.owner === q.owner && p.outer.kind === q.outer.kind && p.outer.name === q.outer.name;
  return root && a.path.length === b.path.length && a.path.every((s, k) => sameStep(s, b.path[k]));
}

function sameStep(a: Step, b: Step): boolean {
  if (a.kind === "field") return b.kind === "field" && a.name === b.name;
  if (a.kind === "index") return b.kind === "index" && a.index === b.index;
  if (a.kind === "inside") return b.kind === "inside";
  return a.kind === b.kind && a.id === b.id;
}

function join(path: string, part: string): string {
  return path ? `${path}/${part}` : part;
}

/** The values a call made as a value was given. */
function argValues(args: Args): V[] {
  return args instanceof Map ? [...args.keys()].sort().map((k) => args.get(k)!) : [];
}

function failureCode(f: unknown): string {
  return f instanceof Failure ? `failure.${String(f.kind).toLowerCase()}` : "failure.ir";
}

/** A change from outside that failed, as the host is told. */
function refused(f: unknown, site: string): Broken {
  return f instanceof Failure && f.rule ? { rule: f.rule, site } : { code: "contracts.outside", site };
}

function message(f: unknown): string {
  return f instanceof Error ? f.message : String(f);
}

/** The IR this runtime runs: a whole number of its own, apart from its release. */
export const IR_VERSION = 4;

/** A page still changing after this many passes is stopped (page 6, failures). */
export const MAX_PASSES = 8;

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
  /** Moves with every bump: what was found fresh at one is, until the next. */
  private stamp = 1;
  private epoch = 0;
  private memo = new Map<string, Memo>();
  private externMemo = new Map<string, { frame: Frame; node: ir.NodeExtern; path: string; owner: number | undefined; bindings: [string, Binding][]; epoch: number; drawn: Drawn[] }>();
  /** What each loop, branch, place and local of a view gave, with what it read. */
  private nodeMemo = new Map<string, { frame: Frame; node: ir.Node; path: string; owner: number | undefined; bindings: [string, Binding][]; epoch: number; drawn: Drawn[]; value?: V }>();
  /** The nodes drawn anew this redraw. */
  private nodeSeen = new Set<string>();
  /** The page's rules that held or didn't, with what they read. */
  private ruleMemo = new Map<ir.Require, { deps: Map<string, number>; epoch: number; holds: boolean }>();
  private frames: Frame[] = [];
  /** What is read on the way to a part, while it is. */
  private partDeps: Map<string, number> | undefined;
  /** Above 0 while what is read is told by name: a held fn's calls, and `$state`'s rules. */
  private whole = 0;
  private readonly memoOn: boolean;
  /** How many changes came from outside the layout: the reader's, the host's, the page's
   * values. A layout that didn't settle is held until the next one. */
  changes = 0;
  /** The change a page that didn't settle was stopped at. */
  private held: number | undefined;
  /** What of the data it started from didn't go back, as `place.name`; undefined when it started from none,
   * or its `$state` was refused. */
  readonly left: string[] | undefined;
  /** Own variables to put back as the page is drawn, by place: what is left after is what had no place. */
  private putBack: Map<string, Map<string, ir.Expr>> | undefined;
  /** How many times the page was drawn, for a test that counts them. */
  draws = 0;
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
  /** How many inputs' own calls are being drawn around here: a sentence's, or a view's. */
  private inInputs = 0;
  private readonly channels = new Map<string, ir.ExternChannel>();
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
      this.channels.set(c.name, c);
      for (const f of (c.values ?? [])) {
        if (f.default) this.externs.set(`${c.name}.${f.name}`, this.constant(f.default));
      }
    }
    for (const f of state?.fields ?? []) {
      if (f.default) this.state.set(f.name, this.constant(f.default));
    }
    for (const [c, values] of Object.entries(opts.externs ?? {})) {
      for (const [k, x] of Object.entries(values)) this.giveExtern(c, k, this.given(x, `${c}.${k}`));
    }
    if (opts.page) this.applyPage(opts.page);
    this.startFrom(opts.state ?? {});
    this.left = opts.data && this.takeState(opts.data.state ?? {}) ? this.drawWith(opts.data.own ?? {}) : undefined;
    if (!this.left) this.redraw();
  }

  /** `$state` as the host gives it, before the page is drawn: checked as a host's write is, all together, or
   * else each field that fits and keeps the rules, one at a time. What doesn't keeps its starting value. */
  private startFrom(given: Record<string, ir.Expr>): void {
    const values: Record<string, V> = {};
    for (const [k, x] of Object.entries(given)) {
      const v = this.given(x, `$state.${k}`);
      if (v) values[k] = v;
    }
    const told = this.outcome.refused.length;
    if (Object.keys(values).length === 0 || this.writeState(values)) return;
    this.outcome.refused.length = told;
    for (const [k, v] of Object.entries(values)) this.writeState({ [k]: v });
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
    const params = this.externWidgets.get(node.widget)?.events?.find((e) => e.name === name)?.params;
    if (!this.takesArgs(params ?? [], args, node.key)) return;
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
      const broken = this.refusal(value, f.type, node.key);
      if (broken) {
        this.outcome.refused.push(broken);
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
      this.writeInto(node, input, w, value);
      const broken = this.stateRefusal();
      if (broken) {
        this.restore(snap);
        this.outcome.refused.push(broken);
      }
    } catch (f) {
      this.restore(snap);
      this.outcome.refused.push(refused(f, node.key));
    }
    this.redraw();
  }

  /** A `mut` input's write to the variable it was given: checked against the input's type, as an engine
   * makes the value, and by the write against the variable's. */
  private writeInto(node: Drawn, input: string, w: { place: ir.Place; env: Env }, value: V): void {
    const x = this.externWidgets.get(node.widget);
    const f = x?.inputs?.find((f) => f.name === input);
    if (f && !this.machine.takes(value, f.type)) throw new Failure("Rule", `${input} is given what isn't its type`);
    this.writePlace(w.place, w.env, value);
    // an input of a type parameter's type takes any value: the variable's own type says which
    if (x?.generics?.length) {
      const root = this.rootOf(w.place, w.env);
      if (root && !this.machine.takes(root.value, root.type)) throw new Failure("Rule", `${input} is given what isn't its variable's type`);
    }
  }

  /** A pass's writes, as one change: a value equal to the one there is no change, and a
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
          this.writeInto(node, input, w, value);
          shared = true;
        } else {
          const f = this.externWidgets.get(node.widget)?.inputs?.find((f) => f.name === input && f.mut);
          if (!f) continue;
          if (!this.machine.takes(value, f.type)) throw new Failure("Rule", `${input} is given what isn't its type`);
          if (!this.own.has(node.key)) this.own.set(node.key, new Map());
          this.own.get(node.key)!.set(input, value);
      this.bump(`o:${node.key}`);
        }
        changed = true;
      } catch (f) {
        this.restore(snap);
        this.own = ownBefore;
        this.outcome.refused.push(refused(f, node.key));
        return false;
      }
    }
    if (!changed) return false;
    const broken = this.stateRefusal();
    if (broken) {
      this.restore(snap);
      this.own = ownBefore;
      this.outcome.refused.push(broken);
      return false;
    }
    this.redraw();
    return shared;
  }

  /** Runs passes until one changes nothing: each pass's writes are one change. A page still changing after
   * MAX_PASSES of them is stopped, and is a failure of the widgets it was for; stopped once, it stays so until
   * something else changes the page, as an engine that settles it on every resize would run it for ever. */
  settle(pass: () => { writes: { path: string; input: string; value: V }[]; of: string[] }): void {
    if (this.held === this.changes) return;
    this.held = undefined;
    let last: string[] = [];
    for (let k = 0; k < MAX_PASSES; k++) {
      const { writes, of } = pass();
      if (!of.length || !this.layoutWrite(writes)) return;
      last = of;
    }
    for (const id of last) this.layoutFailed(id);
    this.held = this.changes;
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
    if (this.writeState(changes)) this.redraw();
  }

  /** The same, with its values in the IR's form. */
  setGiven(changes: Record<string, ir.Expr>): void {
    this.changes++;
    this.machine.resetSteps();
    if (this.takeState(changes)) this.redraw();
  }

  /** `$state` fields written, checked: refused whole, or done. Nothing is drawn. */
  private writeState(changes: Record<string, V>): boolean {
    const snap = this.snapshot();
    const fields = new Map((this.stateDecl?.fields ?? []).map((f) => [f.name, f]));
    for (const [k, v] of Object.entries(changes)) {
      const f = fields.get(k);
      const broken = f ? this.refusal(v, f.type, `$state.${k}`) : { code: "contracts.outside", site: `$state.${k}` };
      if (broken) {
        this.restore(snap);
        this.outcome.refused.push(broken);
        return false;
      }
      this.state.set(k, v);
      this.bump(`s:${k}`);
    }
    const broken = this.stateRefusal();
    if (broken) {
      this.restore(snap);
      this.outcome.refused.push(broken);
      return false;
    }
    return true;
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
      if (!this.takesArgs(f.params, args, `${channel}.${name}`)) return;
      this.transaction(`${channel}.${name}`, () => this.runBody(f.params, f.body, { owner: undefined, bindings: [], path: "" }, args));
      return;
    }
  }

  /** The host giving a channel's values: one that doesn't fit its type is dropped for its default, and the
   * host is told. */
  setExtern(channel: string, values: Record<string, V | undefined>): void {
    this.changes++;
    this.machine.resetSteps();
    for (const [k, v] of Object.entries(values)) {
      this.giveExtern(channel, k, v);
      this.bump(`x:${channel}.${k}`);
    }
    this.redraw();
  }

  /** A channel's value as the host gives it, or its default for one that isn't of its type. A channel's
   * value this page doesn't read is no part of it. */
  private giveExtern(channel: string, name: string, v: V | undefined): void {
    const f = this.channels.get(channel)?.values?.find((f) => f.name === name);
    if (!f) return;
    const broken = v && this.refusal(v, f.type, `${channel}.${name}`);
    if (broken) this.outcome.refused.push(broken);
    const x = v && !broken ? v : f.default && this.constant(f.default);
    if (x) this.externs.set(`${channel}.${name}`, x);
  }

  /** The page's own values: its width and theme. */
  setPage(page: PageValues): void {
    this.changes++;
    this.applyPage(page);
    this.redraw();
  }

  /** Which extern widgets the engine draws: set by an engine before it first draws the page,
   * as @extended-mark/dom does from its registry. The page is drawn again, if any of its widgets is. */
  setSupplies(supplies: (widget: string) => boolean): void {
    const before = this.supplies;
    this.supplies = supplies;
    // what decides is the page's own extern widgets: the same for each, and nothing changes
    if ([...this.externWidgets.keys()].every((w) => before(w) === supplies(w))) return;
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
    return this.drawWith(values);
  }

  /** The page's data put back, as one change: `$state` as one checked write, then widgets' own variables
   * as `restoreOwn` puts them back. Undefined, and nothing is, when `$state` is refused. */
  restoreData(data: { state?: Record<string, ir.Expr>; own?: Record<string, Record<string, ir.Expr>> }): string[] | undefined {
    this.changes++;
    this.machine.resetSteps();
    return this.takeState(data.state ?? {}) ? this.drawWith(data.own ?? {}) : undefined;
  }

  /** `$state` written from the IR's form, whole or not at all. */
  private takeState(given: Record<string, ir.Expr>): boolean {
    const values: Record<string, V> = {};
    for (const [k, x] of Object.entries(given)) {
      const v = this.given(x, `$state.${k}`);
      if (!v) return false;
      values[k] = v;
    }
    return this.writeState(values);
  }

  /** The page drawn with widgets' own variables at their places: what had none, as `place.name`. */
  private drawWith(own: Record<string, Record<string, ir.Expr>>): string[] {
    const back = new Map(Object.entries(own).map(([path, vars]) => [path, new Map(Object.entries(vars))]));
    this.putBack = back;
    // every memo read the old values
    this.epoch++;
    try {
      this.redraw();
    } finally {
      this.putBack = undefined;
    }
    return [...back].flatMap(([path, vars]) => [...vars.keys()].map((n) => `${path}.${n}`));
  }

  /** What is put back at a place, as it is drawn: each variable that is there and whose value fits it. */
  private back(path: string, type: (name: string) => ir.Type | undefined, put: (name: string, v: V) => void): void {
    const vars = this.putBack?.get(path);
    if (!vars) return;
    for (const [name, x] of [...vars]) {
      const t = type(name);
      if (!t) continue;
      let v: V;
      try {
        v = this.value(x);
      } catch {
        continue;
      }
      if (this.refusal(v, t, path)) continue;
      put(name, v);
      vars.delete(name);
    }
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

  /** Why a value from outside is refused, or undefined when it is one of the type. */
  private refusal(v: V, t: ir.Type, site: string): Broken | undefined {
    try {
      return this.machine.takes(v, t) ? undefined : { code: "contracts.outside", site };
    } catch (f) {
      return refused(f, site);
    }
  }

  /** A value a host or an engine gives, in the IR's form: one that can't be made, such as a record whose
   * own rule is false, is refused, and undefined. */
  given(x: ir.Expr, site: string): V | undefined {
    try {
      return this.constant(x);
    } catch (f) {
      this.errors.push(f);
      this.outcome.refused.push(refused(f, site));
      return undefined;
    }
  }

  /** The same for the arguments of a call: all of them, or undefined. */
  givenAll(xs: readonly ir.Expr[], site: string): V[] | undefined {
    const out: V[] = [];
    for (const x of xs) {
      const v = this.given(x, site);
      if (!v) return undefined;
      out.push(v);
    }
    return out;
  }

  /** Whether a call from outside gives its params values of their types: told when it doesn't. */
  private takesArgs(params: readonly ir.Param[], args: readonly V[], site: string): boolean {
    for (const [k, p] of params.entries()) {
      const broken = args[k] && this.refusal(args[k], p.type, site);
      if (broken) {
        this.outcome.refused.push(broken);
        return false;
      }
    }
    return true;
  }

  /** A value in the IR's form, as the host and the checks give them. */
  value(x: ir.Expr): V {
    return this.constant(x);
  }

  private constant(x: ir.Expr): V {
    const m = new Machine(this.program, new Constants());
    // an item is known by its number, whoever makes its list: all of a page's are from one count
    m.nextId = this.machine.nextId;
    try {
      return m.eval(x);
    } finally {
      this.machine.nextId = m.nextId;
    }
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
      readAt(m: Machine, place: Place): V {
        const top = page.frames[page.frames.length - 1];
        if (!top || page.partDeps || page.whole) return m.valueAt(place);
        const deps = new Map<string, number>();
        const was = page.partAt(place, deps);
        (top.parts ??= []).push({ place, was, deps });
        return was;
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
          if (out === null) return null;
          const v = page.constant(out);
          const t = page.program.externs.get(name)?.returns;
          if (t && !page.machine.takes(v, t)) throw new Failure("Rule", `\`${name}\` gave what isn't its type`);
          return v;
        }
        if (!page.hostFns.has(name)) return undefined;
        page.soil();
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

  /** The variable a place is in, `$state`'s or a widget's own: its type, and its value now. */
  private rootOf(place: ir.Place, env: Env): { type: ir.Type; value: V } | undefined {
    const root = this.machine.rootOf(env.bindings, env.owner, place);
    if (root?.kind !== "outer") return undefined;
    const { outer, owner } = root;
    const inst = owner === undefined ? undefined : this.instances.get(owner);
    if (outer.kind === "input") {
      const p = inst?.places.get(outer.name);
      return p && this.rootOf(p.place, p.env);
    }
    const type = outer.kind === "state" ? this.stateDecl?.fields.find((f) => f.name === outer.name)?.type : inst?.template.cells?.find((c) => c.name === outer.name)?.type;
    const value = outer.kind === "state" ? this.state.get(outer.name) : inst?.cells.get(outer.name);
    return type && value && { type, value };
  }

  /** The message of the first of `$state`'s rules that no longer holds. */
  private stateRuleBroken(): string | undefined {
    for (const r of this.stateDecl?.requires ?? []) {
      if (!r.warn && !this.stateRule(r)) return r.message;
    }
    return undefined;
  }

  /** The same as a host is told: a rule that can't be worked out doesn't hold. */
  private stateRefusal(): Broken | undefined {
    try {
      const rule = this.stateRuleBroken();
      return rule === undefined ? undefined : { rule };
    } catch (f) {
      this.errors.push(f);
      return { code: failureCode(f), site: "$state" };
    }
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
      cells: new Map([...this.instances].filter(([, i]) => i.cells.size).map(([id, i]) => [id, new Map(i.cells)])),
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
    this.draws++;
    // each redraw has the step limit to itself, as each event does: steps never pile up across changes
    this.machine.resetSteps();
    this.missed.clear();
    this.claimed.clear();
    this.atPath = new Map();
    this.ownSeen.clear();
    this.nodeSeen.clear();
    for (const r of this.stateDecl?.requires ?? []) {
      if (!r.warn) continue;
      try {
        this.warned("$state", r.message, this.stateRule(r));
      } catch (f) {
        // one that can't be worked out is a failure, and stops nothing
        this.errors.push(f);
        this.outcome.failures.push({ code: failureCode(f), site: "$state" });
      }
    }
    const doc = this.templates.get("doc");
    this.drawn = doc ? this.drawNodes(doc.view, { owner: undefined, bindings: [], path: "" }) : [];
    // a widget that left the page leaves with its variables
    for (const [id, i] of this.instances) {
      if (i.seen !== this.draws) {
        this.instances.delete(id);
        // a widget of another kind may have taken its key this redraw: that one stays
        if (this.byPath.get(i.path) === i) {
          this.byPath.delete(i.path);
          this.memo.delete(i.path);
        }
      }
    }
    for (const k of this.own.keys()) if (!this.ownSeen.has(k)) this.own.delete(k);
    for (const [k, m] of this.externMemo) if (m.frame.seen !== this.draws) this.externMemo.delete(k);
    for (const [k, m] of this.nodeMemo) if (m.frame.seen !== this.draws) this.nodeMemo.delete(k);
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
        let m: { value?: V };
        try {
          m = this.hold(n, `let ${l.name}`, at, () => ({ drawn: [], value: this.evalIn(at, l.value) }));
        } catch (f) {
          if (!this.contains(env)) throw f;
          // what comes after may read it
          out.push(this.failed("doc", join(env.path, `let ${l.name}`), join(env.key ?? env.path, `let ${l.name}`), f));
          break;
        }
        env = { ...env, bindings: [...env.bindings, [l.name, { value: m.value! }]] };
      } else if ("extern" in n || "use" in n) {
        out.push(...this.drawNode(n, env));
      } else {
        const site = "for" in n ? n.for.site : "if" in n ? n.if.site : "match" in n ? n.match.site : n.place.site;
        const at = env;
        try {
          out.push(...this.hold(n, site, at, () => ({ drawn: this.drawNode(n, at) })).drawn);
        } catch (f) {
          if (!this.contains(env)) throw f;
          out.push(this.failed("doc", join(env.path, site), join(env.key ?? env.path, site), f));
        }
      }
    }
    return out;
  }

  /** Whether a failure here stays here: in the document's own code, which no widget is around to show it
   * for. In a sentence it is the failure of what holds the sentence. */
  private contains(env: Env): boolean {
    return env.owner === undefined && this.inInputs === 0;
  }

  /** What a loop, a branch, a place or a local gives: as it was, when it is the same one with the same
   * locals around it and nothing it read has changed. */
  private hold(node: ir.Node, site: string, env: Env, work: () => { drawn: Drawn[]; value?: V }): { drawn: Drawn[]; value?: V } {
    const key = `${env.key ?? env.path}|${site}`;
    // the same content placed twice in one redraw is two
    const m = this.nodeMemo.get(key);
    const twice = this.nodeSeen.has(key) || m?.frame.seen === this.draws;
    if (this.memoOn && m && !twice && m.node === node && m.epoch === this.epoch && m.path === env.path && m.owner === env.owner && this.unchanged(m.frame) && this.sameBindings(m.bindings, env.bindings)) {
      this.replay(m.frame);
      return m;
    }
    this.nodeSeen.add(key);
    const frame = this.enter();
    let made: { drawn: Drawn[]; value?: V };
    try {
      made = work();
    } finally {
      this.exit(frame);
    }
    if (this.memoOn && !frame.dirty && !twice) this.nodeMemo.set(key, { frame, node, path: env.path, owner: env.owner, bindings: env.bindings, epoch: this.epoch, drawn: made.drawn, value: made.value });
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
    if (this.memoOn && m && m.node === x && m.epoch === this.epoch && m.path === path && m.owner === env.owner && this.unchanged(m.frame) && this.sameBindings(m.bindings, env.bindings)) {
      this.replay(m.frame);
      return m.drawn;
    }
    const frame = this.enter();
    let drawn: Drawn[];
    try {
      drawn = this.drawExternNow(x, env, path, key);
    } catch (f) {
      if (!this.contains(env)) throw f;
      drawn = [this.failed(x.name, path, key, f)];
    } finally {
      this.exit(frame);
    }
    if (this.memoOn && !frame.dirty) this.externMemo.set(key, { frame, node: x, path, owner: env.owner, bindings: env.bindings, epoch: this.epoch, drawn });
    else this.externMemo.delete(key);
    return drawn;
  }

  /** Who a widget is, this redraw: the place it was written, or that and which time it is placed. */
  private claim(key: string): string {
    let id = key;
    for (let n = 2; this.taken(id); n++) id = `${key}~${n}`;
    this.claimed.add(id);
    return id;
  }

  private taken(id: string): boolean {
    return this.claimed.has(id) || this.externMemo.get(id)?.frame.seen === this.draws || this.byPath.get(id)?.seen === this.draws;
  }

  /** A part of the page that starts to be drawn. */
  private enter(): Frame {
    const frame = new Frame();
    frame.seen = this.draws;
    this.frames.push(frame);
    return frame;
  }

  /** And ends: it is inside the part around it. */
  private exit(frame: Frame): void {
    this.frames.pop();
    const around = this.frames[this.frames.length - 1];
    if (!around) return;
    (around.kids ??= []).push(frame);
    if (frame.dirty) around.dirty = true;
  }

  /** What is being drawn is drawn anew every time. */
  private soil(): void {
    const top = this.frames[this.frames.length - 1];
    if (top) top.dirty = true;
  }

  /** A reused part is inside the one being drawn, and its instances are seen again. */
  private replay(frame: Frame): void {
    this.see(frame);
    const around = this.frames[this.frames.length - 1];
    if (around) (around.kids ??= []).push(frame);
  }

  private see(frame: Frame): void {
    if (frame.seen === this.draws) return;
    frame.seen = this.draws;
    if (frame.nested) {
      for (const [inst, path] of frame.nested) {
        inst.seen = this.draws;
        this.atPath.set(path, inst);
      }
    }
    if (frame.own) for (const k of frame.own) this.ownSeen.add(k);
    if (frame.missed) for (const n of frame.missed) this.missed.add(n);
    if (frame.kids) for (const k of frame.kids) this.see(k);
  }

  /** Whether nothing a part read, or what is inside it, has changed since. */
  private unchanged(frame: Frame): boolean {
    if (frame.fresh < 0) return false;
    if (frame.fresh === this.stamp) return true;
    const ok = (!frame.deps || this.current(frame.deps)) && (!frame.parts || this.partsStand(frame.parts)) && (!frame.kids || frame.kids.every((k) => this.unchanged(k)));
    frame.fresh = ok ? this.stamp : -1;
    return ok;
  }

  /** What is at a place now, read for no one. */
  private partAt(place: Place, deps: Map<string, number>): V {
    const around = this.partDeps;
    this.partDeps = deps;
    try {
      return this.machine.valueAt(place);
    } finally {
      this.partDeps = around;
    }
  }

  private partsStand(parts: Part[]): boolean {
    try {
      return parts.every((p) => {
        if (this.current(p.deps)) return true;
        const deps = new Map<string, number>();
        if (!sameNow(p.was, this.partAt(p.place, deps))) return false;
        p.deps = deps;
        return true;
      });
    } catch {
      // its place is gone
      return false;
    }
  }

  private drawExternNow(x: ir.NodeExtern & { given?: ReadonlyMap<string, V> }, env: Env, path: string, key: string): Drawn[] {
    const props = x.given ? new Map(x.given) : new Map<string, V>();
    if (!x.given) for (const k in x.props) props.set(k, this.evalIn(env, x.props[k]));
    let writes: Drawn["writes"] = NONE;
    for (const k in x.writes) {
      const place = x.writes[k];
      props.set(k, this.machine.evalWith(env.bindings, env.owner, { place } as ir.Expr));
      if (writes === NONE) writes = new Map();
      writes.set(k, { place, env });
    }
    const w = this.externWidgets.get(x.name);
    for (const f of w?.inputs ?? []) {
      if (!props.has(f.name) && f.default) props.set(f.name, this.evalIn(env, f.default));
    }
    // a widget the engine doesn't draw shows its Markdown instead (page 6), drawn with the
    // blocks the engine does: its markdown block, in an instance holding its inputs
    if (w && !this.supplies(x.name)) {
      this.missed.add(x.name);
      (this.frames[this.frames.length - 1].missed ??= []).push(x.name);
      const inst = this.enterExtern(w, path, props, x.slots ?? {}, env);
      try {
        return this.drawNodes(w.markdown ?? [], { ...inst, path, key });
      } finally {
        this.leave(inst);
      }
    }
    // what it wrote to its own `mut` inputs stands until the caller gives them a variable
    if (w?.inputs?.some((f) => f.mut)) {
      this.track(`o:${key}`);
      this.back(path, (n) => (writes.has(n) ? undefined : w.inputs!.find((f) => f.name === n && f.mut)?.type), (n, v) => {
        if (!this.own.has(key)) this.own.set(key, new Map());
        this.own.get(key)!.set(n, v);
      });
      const own = this.own.get(key);
      if (own) {
        this.ownSeen.add(key);
        (this.frames[this.frames.length - 1].own ??= []).push(key);
        for (const [k, v] of own) if (!writes.has(k)) props.set(k, v);
      }
    }
    let slots: Drawn["slots"] = NONE;
    for (const s in x.slots) {
      if (slots === NONE) slots = new Map();
      slots.set(s, this.drawNodes(x.slots[s], env));
    }
    if (w?.requires?.length) {
      const inst = this.enterExtern(w, path, props, x.slots ?? {}, env);
      try {
        this.checkRules(w.requires, inst, path);
      } finally {
        this.leave(inst);
      }
    }
    this.inInputs++;
    try {
      this.drawInputs(w, props, path, key);
    } finally {
      this.inInputs--;
    }
    let events: Drawn["events"] = NONE;
    for (const e in x.events) {
      if (events === NONE) events = new Map();
      events.set(e, { handler: x.events[e], env });
    }
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
    const frame = this.enter();
    let drawn: Drawn[];
    try {
      const own = this.enterUse(t, path, args, slots, events, env, key);
      drawn = this.drawNodes(t.view, own);
    } catch (f) {
      drawn = [this.failed(name, path, key, f)];
    } finally {
      this.exit(frame);
    }
    const inst = this.byPath.get(key);
    if (this.memoOn && !frame.dirty && inst) {
      this.memo.set(key, { frame, template: name, path, owner: env.owner, args: argValues(args), bindings: env.bindings, slots, events, epoch: this.epoch, drawn });
    } else {
      this.memo.delete(key);
    }
    return drawn;
  }

  // ---- reusing what a widget drew (plan tab 2: only the bindings that read what changed)

  /** A read of an outside value, for the part being drawn. */
  track(key: string): void {
    const part = this.partDeps;
    if (part) {
      if (!part.has(key)) part.set(key, this.versions.get(key) ?? 0);
      return;
    }
    const f = this.frames[this.frames.length - 1];
    if (f && !f.deps?.has(key)) (f.deps ??= new Map()).set(key, this.versions.get(key) ?? 0);
  }

  /** An outside value written: every memo that read it is out of date. */
  bump(key: string): void {
    this.versions.set(key, (this.versions.get(key) ?? 0) + 1);
    this.stamp++;
  }

  /** What a fn's calls read, for whoever holds what they gave. */
  reading<T>(f: () => T, fresh = true): { value: T; deps: Map<string, number> } {
    const frame = this.enter();
    this.whole++;
    try {
      if (fresh) this.machine.resetSteps();
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
    if (!this.unchanged(m.frame)) return undefined;
    if (!this.sameBindings(m.bindings, env.bindings)) return undefined;
    // an input worked out here is the same while what it read and the locals around it are
    const now = argValues(args);
    if (now.length !== m.args.length || now.some((v, k) => !this.machine.equal(v, m.args[k]))) return undefined;
    this.replay(m.frame);
    return m.drawn;
  }

  private sameBindings(a: [string, Binding][], b: [string, Binding][]): boolean {
    if (a === b) return true;
    if (a.length !== b.length) return false;
    return a.every(([n, x], k) => {
      const [m, y] = b[k];
      if (n !== m) return false;
      if ("value" in x && "value" in y) return x.value === y.value || this.machine.equal(x.value, y.value);
      return "ref" in x && "ref" in y && sameRef(x.ref, y.ref);
    });
  }

  /** A widget that failed: reported once, and in its place (page 6). */
  private failed(name: string, path: string, key: string, f: unknown): Drawn {
    this.soil();
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
      inst = { id, path: key, template: t, inputs: new Map(), places: new Map(), cells: new Map(), slots: new Map(), events: new Map(), seen: this.draws };
      this.instances.set(id, inst);
      this.byPath.set(key, inst);
    }
    const it = inst!;
    it.seen = this.draws;
    this.atPath.set(path, it);
    const top = this.frames[this.frames.length - 1];
    if (top) (top.nested ??= []).push([it, path]);
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
    this.back(path, (n) => t.cells?.find((c) => c.name === n)?.type, (n, v) => it.cells.set(n, v));
    this.checkRules(t.requires ?? [], own, path);
    return own;
  }

  /** An extern widget's instance, for its markdown block: its inputs' values and its slots. */
  enterExtern(w: ir.ExternWidget, path: string, props: Map<string, V>, slots: Record<string, ir.Node[]>, env: Env): Env {
    const t = { name: w.name, generics: [], inputs: w.inputs, slots: w.slots, events: w.events, cells: [], requires: [], fns: [], view: [], markdown: w.markdown, levels: w.levels } as unknown as ir.Template;
    const id = this.machine.newId();
    const inst: Instance = { id, path: `${path}#md`, template: t, inputs: new Map(props), places: new Map(), cells: new Map(), slots: new Map(), events: new Map(), seen: this.draws };
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

function sameRecord<T>(a: Record<string, T>, b: Record<string, T>): boolean {
  if (a === b) return true;
  const ka = Object.keys(a);
  return ka.length === Object.keys(b).length && ka.every((k) => a[k] === b[k]);
}

const children = new WeakMap<Drawn, readonly (readonly Drawn[])[]>();

/** The widgets drawn inside one: its slots', and those its inputs hold. One drawn is never changed, so
 * they are worked out once. */
export function childrenOf(d: Drawn): readonly (readonly Drawn[])[] {
  const known = children.get(d);
  if (known) return known;
  const out: (readonly Drawn[])[] = [...d.slots.values()];
  const walk = (v: V): void => {
    if (v.kind === "nodes") out.push(v.nodes);
    else if (v.kind === "list") for (const i of v.items) walk(i.value);
    else if (v.kind === "record") for (const [, x] of v.fields) walk(x);
    else if (v.kind === "case") for (const [, x] of v.values) walk(x);
    else if (v.kind === "opt" && v.value) walk(v.value);
  };
  for (const v of d.props.values()) walk(v);
  children.set(d, out);
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
