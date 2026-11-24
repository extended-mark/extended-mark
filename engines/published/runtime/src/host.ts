// A page as a host holds it (page 6, The host): a tree to draw, plain JSON, with what changed in it;
// the reader's events, the widgets' writes and the fns a widget calls, by the node's id; and the page's
// data, to keep and to put back. What it is made on is never part of it.

import type * as ir from "./generated/ir.js";
import type { Change, Child, Contract, Data, Ends, Input, Node, Tree } from "./generated/tree.js";
import { Failure } from "./eval/index.js";
import { toIr, type Value as V } from "./eval/value.js";
import { pageBlocks, type Block } from "./markdown.js";
import { Core, IR_VERSION, childrenOf, type Broken, type Drawn, type CoreOptions, type Outcome, type PageValues } from "./page.js";

export { IR_VERSION };
export type { Broken, Outcome, PageValues };

/** A fn of the host's the page called. */
export interface Call {
  call: { name: string; args: Input[] };
}

/** What a host gives a page as it is mounted. */
export interface PageOptions extends Omit<CoreOptions, "fns" | "json" | "memo" | "state" | "data" | "externs"> {
  /** `$state` to start from, over its starting values. */
  state?: Record<string, Input>;
  /** The page's data to start from, as `data()` gave it: drawn once, with it. `$state` is one checked write,
   * refused whole, and widgets' own variables go back where they fit. */
  data?: Partial<Data>;
  /** Each channel's values, by its full name (`doc/scores`). */
  externs?: Record<string, Record<string, Input>>;
  /** The value fns the host or the engine supplies, by full name (`acme/rate`). */
  fns?: Record<string, (args: readonly Input[]) => Input | null>;
}

const cores = new WeakMap<Page, Core>();

/** What a page runs on: the runtime's own. */
export function coreOf(page: Page): Core {
  return cores.get(page)!;
}

/** A fn a host calls: what it is, what its calls read, and how many times that has changed. */
interface HeldFn {
  fn: V & { kind: "fn" };
  deps: Map<string, number>;
  version: number;
}

/** A drawn widget as a node, and the fns in it. */
interface Made {
  node: Node;
  fns: string[];
}

export class Page {
  /** The IR it runs, which never changes. */
  readonly ir: ir.Module;
  /** Every failure caught, with its message and stack, for a host's log. */
  readonly errors: unknown[];
  private readonly core: Core;
  /** The token types, for a value the page gives while it is being made. */
  private readonly tokens: ReadonlySet<string>;
  private now: Tree = { nodes: [] };
  private byId: Map<string, { node: Node; drawn: Drawn }> | undefined;
  private made = new WeakMap<Drawn, Made>();
  private fns = new Map<string, HeldFn>();
  private contracts = new WeakMap<ir.Type, Contract>();
  private readonly listeners = new Set<(changes: Change[]) => void>();

  /** Mounts the page: its IR, never a document's text. */
  constructor(file: ir.File | ir.Module, opts: PageOptions = {}) {
    this.tokens = new Set(("ir" in file ? file.ir : file).declarations.flatMap((d) => ("token" in d.declare ? [d.declare.token.name] : [])));
    this.core = new Core(file, { ...(opts as CoreOptions), json: (v) => this.plain(v) as ir.Expr });
    cores.set(this, this.core);
    this.ir = this.core.module;
    this.errors = this.core.errors;
    this.now = this.draw();
    this.core.onChange(() => {
      const before = this.now;
      this.now = this.draw();
      const changes = diffNodes("", "nodes", before.nodes, this.now.nodes);
      for (const l of [...this.listeners]) l(changes);
    });
  }

  // ---- the tree

  /** The page now. Its nodes are never changed: one that is the same as before is the same object. */
  tree(): Tree {
    return this.now;
  }

  /** The node of an id, if it is on the page. */
  node(id: string): Node | undefined {
    return this.ids().get(id)?.node;
  }

  /** How many changes came from outside the layout: a layout that didn't settle is held until the next. */
  get changes(): number {
    return this.core.changes;
  }

  /** Called after every change, with what changed in the tree (nothing, when only data did). */
  onChange(listener: (changes: Change[]) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  // ---- what the reader and the widgets do

  /** The reader's event on a widget: what the page does with it, as one change. */
  event(id: string, name: string, args: Input[] = []): void {
    const d = this.drawn(id);
    const given = d && this.core.givenAll(args.map(fromInput), id);
    if (d && given) this.core.event(d.path, name, given);
  }

  /** A widget writing one of its `mut` inputs, as a slider does: checked, then one change. */
  write(id: string, input: string, value: Input): void {
    const d = this.drawn(id);
    const given = d && this.core.given(fromInput(value), id);
    if (d && given) this.core.write(d.path, input, given);
  }

  /** A fn input called, as a shade calls its shader. */
  call(id: string, input: string, args: Input[] = []): Input {
    const fn = this.drawn(id)?.props.get(input);
    const f = fn?.kind === "opt" ? fn.value : fn;
    if (f?.kind !== "fn") throw new Failure("Ir", `${id} has no fn \`${input}\``);
    const held = this.held(`${id}\n${input}`, f);
    const { value, deps } = this.core.reading(() => this.core.machine.apply(f, args.map((a) => ({ value: this.core.value(fromInput(a)) }))));
    for (const [k, v] of deps) held.deps.set(k, v);
    return value ? this.plain(value) : { none: {} };
  }

  // ---- what the host does

  /** Writes `$state` fields as one change, checked: refused whole, or done. */
  set(changes: Record<string, Input>): void {
    this.core.setGiven(Object.fromEntries(Object.entries(changes).map(([k, x]) => [k, fromInput(x)])));
  }

  /** Calls a fn the document exposes. */
  expose(channel: string, name: string, args: Input[] = []): void {
    const given = this.core.givenAll(args.map(fromInput), `${channel}.${name}`);
    if (given) this.core.expose(channel, name, given);
  }

  /** Gives a channel's values. */
  setExtern(channel: string, values: Record<string, Input>): void {
    this.core.setExtern(channel, Object.fromEntries(Object.entries(values).map(([k, x]) => [k, this.core.given(fromInput(x), `${channel}.${k}`)])));
  }

  /** Gives the page's own values: its width and theme. */
  setPage(page: PageValues): void {
    this.core.setPage(page);
  }

  /** The values the host has given, by full name: `core/page.theme`. */
  given(): Record<string, Input> {
    return Object.fromEntries([...this.core.externsNow()].map(([k, v]) => [k, this.plain(v)]));
  }

  /** Which extern widgets the host draws: the rest show their Markdown. */
  setSupplies(supplies: (widget: string) => boolean): void {
    this.core.setSupplies(supplies);
  }

  /** `$state` now. */
  state(): Record<string, Input> {
    return Object.fromEntries([...this.core.stateNow()].map(([k, v]) => [k, this.plain(v)]));
  }

  /** The page's data: `$state`, and every widget's own variables. */
  data(): Data {
    return { state: this.state(), own: this.core.observeOwn((v) => this.plain(v)) };
  }

  /** Widgets' own variables put back, as one change: the ones that don't fit, or whose widget isn't on
   * the page, are left out and returned. `$state` goes back with `set`. */
  restoreOwn(own: NonNullable<Data["own"]>): string[] {
    return this.core.restoreOwn(own as Record<string, Record<string, ir.Expr>>);
  }

  /** The page's data put back, as one change: `$state` as one checked write, then widgets' own variables.
   * Gives what of those didn't fit, or whose widget isn't on the page; undefined, and nothing is put
   * back, when `$state` is refused. */
  restore(data: Partial<Data>): string[] | undefined {
    return this.core.restoreData(data as CoreOptions["data"] & object);
  }

  /** What of the `data` the page started from didn't fit; undefined when it started from none, or its
   * `$state` was refused. */
  get left(): string[] | undefined {
    return this.core.left;
  }

  /** The page as Markdown, with the values it has now. */
  markdown(): string {
    return this.core.markdown();
  }

  /** The page's Markdown as blocks. */
  blocks(): Block[] {
    return pageBlocks(this.core);
  }

  /** What a token stands for, as the IR says: a host may draw it with its own. */
  token(of: Input): Input | undefined {
    if ("token" in of) return this.plain(this.core.machine.defaultTokenValue(this.core.value(of as ir.Expr) as V & { kind: "case" }));
    return "tokval" in of ? of.tokval.value : undefined;
  }

  /** The extern widgets the host doesn't draw, each shown as its Markdown. */
  missing(): string[] {
    return this.core.missing();
  }

  /** What the page takes (its IR's `needs`) and nobody gives yet. */
  lacking(): string[] {
    return this.core.lacking();
  }

  /** The host's extern fns called since the last time they were taken. */
  takeCalls(): Call[] {
    return this.core.takeCalls() as Call[];
  }

  /** Refused writes and failures since the last time they were taken. */
  takeOutcome(): Outcome {
    return this.core.takeOutcome();
  }

  /** The `warn` rules found false since the last time they were taken. */
  takeWarnings(): Broken[] {
    return this.core.takeWarnings();
  }

  // ---- inside

  private drawn(id: string): Drawn | undefined {
    return this.ids().get(id)?.drawn;
  }

  /** Every node by its id, once something asks for one. */
  private ids(): Map<string, { node: Node; drawn: Drawn }> {
    if (!this.byId) {
      this.byId = new Map();
      this.core.tree().forEach((d, k) => this.index(d, this.now.nodes[k]));
    }
    return this.byId;
  }

  /** The page drawn, as nodes: the ones nothing changed in are the ones there were. */
  private draw(): Tree {
    this.byId = undefined;
    return { nodes: this.core.tree().map((d) => this.nodeOf(d)) };
  }

  private nodeOf(d: Drawn): Node {
    let m = this.made.get(d);
    if (!m || !m.fns.every((k) => this.fresh(k))) {
      const fns: string[] = [];
      m = { node: this.make(d, fns), fns };
      this.made.set(d, m);
    }
    return m.node;
  }

  /** A node and everything in it, found by id. */
  private index(d: Drawn, node: Node): void {
    this.byId!.set(node.id, { node, drawn: d });
    const inside = Object.values(node.inputs).flatMap((x) => nodesIn(x));
    const by = new Map(inside.map((n) => [n.id, n]));
    for (const kids of childrenOf(d)) for (const k of kids) this.index(k, by.get(k.key)!);
  }

  private make(d: Drawn, fns: string[]): Node {
    if (d.failed !== undefined) return { id: d.key, widget: d.widget, inputs: {}, failed: d.failed };
    const inputs: Record<string, Input> = {};
    for (const [k, v] of d.props) inputs[k] = this.input(v, `${d.key}\n${k}`, fns);
    for (const [k, kids] of d.slots) inputs[k] = { nodes: kids.map((n) => this.inside(n, fns)) };
    const node: Node = { id: d.key, widget: d.widget, inputs };
    const events = [...d.events.keys()].filter((e) => this.core.listens(d, e));
    if (events.length) node.events = events;
    const writes: Record<string, Contract> = {};
    for (const f of this.core.externWidget(d.widget)?.inputs ?? []) {
      if (f.mut) writes[f.name] = this.contract(this.core.writeType(d, f.name) ?? f.type);
    }
    if (Object.keys(writes).length) node.writes = writes;
    return node;
  }

  private inside(d: Drawn, fns: string[]): Node {
    const node = this.nodeOf(d);
    fns.push(...this.made.get(d)!.fns);
    return node;
  }

  /** A value as the host is given it: the IR's form, its widgets nodes and its fns to call. */
  private input(v: V, at: string, fns: string[]): Input {
    switch (v.kind) {
      case "nodes":
        return { nodes: v.nodes.map((n) => this.inside(n, fns)) };
      case "fn": {
        fns.push(at);
        return { fn: { version: this.held(at, v).version } };
      }
      case "list":
        return { list: v.items.map((i, k) => this.input(i.value, `${at}[${k}]`, fns)) };
      case "record":
        return { record: { type: v.ty, fields: Object.fromEntries(v.fields.map(([k, x]) => [k, this.input(x, `${at}.${k}`, fns)])) } };
      case "case":
        if (this.tokens.has(v.ty)) {
          const held = v.values[0]?.[1];
          return held ? { tokval: { type: v.ty, name: v.name, value: this.input(held, at, fns) } } : { token: `${v.ty}.${v.name}` };
        }
        if (!v.values.length) return { case: { enum: v.ty, name: v.name } };
        return { case: { enum: v.ty, name: v.name, values: Object.fromEntries(v.values.map(([k, x]) => [k, this.input(x, `${at}.${k}`, fns)])) } };
      case "opt":
        return v.value === null ? { none: {} } : { some: this.input(v.value, at, fns) };
      case "range": {
        const ends = Object.entries({ low: v.low, high: v.high, step: v.step }).flatMap(([k, x]) => (x ? [[k, this.input(x, at, fns)]] : []));
        return { range: Object.fromEntries(ends) };
      }
      default:
        return toIr(v) as Input;
    }
  }

  /** A value that holds no widgets and no fn. */
  private plain(v: V): Input {
    return this.input(v, "", []);
  }

  /** A fn as it is held: another when it was given other values, or something its calls read changed. */
  private held(at: string, fn: V & { kind: "fn" }): HeldFn {
    let h = this.fns.get(at);
    if (!h) this.fns.set(at, (h = { fn, deps: new Map(), version: 0 }));
    else if (!this.core.sameFn(h.fn, fn) || !this.core.current(h.deps)) this.fns.set(at, (h = { fn, deps: new Map(), version: h.version + 1 }));
    return h;
  }

  private fresh(at: string): boolean {
    const h = this.fns.get(at);
    return !!h && this.core.current(h.deps);
  }

  /** What a value of a type has to fit, its aliases' contracts among them. */
  private contract(t: ir.Type): Contract {
    let c = this.contracts.get(t);
    if (!c) {
      c = {};
      const worked = (e: ir.Expr) => this.plain(this.core.value(e));
      const ends = (b: ir.Bounds): Ends => ({ ...(b.low ? { low: worked(b.low) } : {}), ...(b.high ? { high: worked(b.high) } : {}) });
      for (let ty: ir.Type | undefined = t, k = 0; ty && k < 16; ty = this.core.program.aliases.get(ty.name), k++) {
        const x: ir.Contract | undefined = ty.contract;
        if (x?.range) c.range = { ...ends(x.range), ...c.range };
        if (x?.length) c.length = { ...ends(x.length), ...c.length };
        if (x?.step) c.step ??= worked(x.step);
        if (x?.only?.length) c.only ??= x.only.map(worked);
      }
      this.contracts.set(t, c);
    }
    return c;
  }
}

/** The widgets directly in a value: a slot's, or those of a list's sentences. */
export function nodesIn(x: Input, out: Node[] = []): Node[] {
  if ("nodes" in x) out.push(...x.nodes);
  else if ("list" in x) x.list.forEach((y) => nodesIn(y, out));
  else if ("record" in x) Object.values(x.record.fields).forEach((y) => nodesIn(y, out));
  else if ("case" in x) Object.values(x.case.values ?? {}).forEach((y) => nodesIn(y, out));
  else if ("some" in x) nodesIn(x.some, out);
  return out;
}

/** Every node of a tree, each before what is in it. */
export function everyNode(nodes: readonly Node[], out: Node[] = []): Node[] {
  for (const n of nodes) {
    out.push(n);
    for (const x of Object.values(n.inputs)) everyNode(nodesIn(x), out);
  }
  return out;
}

/** An input in the IR's form. Widgets and fns are the page's own, and are never given to it. */
export function fromInput(x: Input): ir.Expr {
  return x as ir.Expr;
}

// ---- what changed

function same(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== "object" || typeof b !== "object" || !a || !b) return false;
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const ka = Object.keys(a);
  return ka.length === Object.keys(b).length && ka.every((k) => Object.hasOwn(b, k) && same((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]));
}

/** Whether a node is the one it was, whatever changed in it. */
function kept(a: Node, b: Node): boolean {
  return a.id === b.id && a.widget === b.widget && (a.failed === undefined) === (b.failed === undefined);
}

/** What changed in the widgets of one input: which they are, or what is in them. */
function diffNodes(id: string, input: string, a: readonly Node[], b: readonly Node[]): Change[] {
  if (a === b) return [];
  const before = new Map(a.map((n) => [n.id, n]));
  const out: Change[] = [];
  const children: Child[] = [];
  for (const n of b) {
    const was = before.get(n.id);
    if (!was || !kept(was, n)) children.push({ node: n });
    else {
      children.push({ keep: n.id });
      out.push(...diff(was, n));
    }
  }
  const as_was = a.length === b.length && children.every((c, k) => "keep" in c && c.keep === a[k].id);
  return as_was ? out : [{ id, children: { [input]: children } }, ...out];
}

/** Whether two values differ only in what is in their widgets, whose changes are told apart. */
function diffInput(id: string, a: Input, b: Input, out: Change[]): boolean {
  if (a === b) return true;
  if ("nodes" in a && "nodes" in b) {
    if (a.nodes.length !== b.nodes.length || !a.nodes.every((n, k) => kept(n, b.nodes[k]))) return false;
    a.nodes.forEach((n, k) => out.push(...diff(n, b.nodes[k])));
    return true;
  }
  if ("list" in a && "list" in b) return a.list.length === b.list.length && a.list.every((x, k) => diffInput(id, x, b.list[k], out));
  if ("some" in a && "some" in b) return diffInput(id, a.some, b.some, out);
  if ("record" in a && "record" in b) {
    const [x, y] = [a.record, b.record];
    return x.type === y.type && Object.keys(x.fields).length === Object.keys(y.fields).length && Object.keys(x.fields).every((k) => k in y.fields && diffInput(id, x.fields[k], y.fields[k], out));
  }
  return same(a, b);
}

/** What changed in a node that is still there. */
function diff(a: Node, b: Node): Change[] {
  if (a === b) return [];
  const own: Change = { id: b.id };
  const inside: Change[] = [];
  for (const [k, x] of Object.entries(b.inputs)) {
    const was = a.inputs[k];
    if (was && "nodes" in was && "nodes" in x) {
      for (const c of diffNodes(b.id, k, was.nodes, x.nodes)) {
        if (c.id === b.id) own.children = { ...own.children, ...c.children };
        else inside.push(c);
      }
      continue;
    }
    const deeper: Change[] = [];
    if (was && diffInput(b.id, was, x, deeper)) inside.push(...deeper);
    else own.inputs = { ...own.inputs, [k]: x };
  }
  if (!same(a.events ?? [], b.events ?? [])) own.events = b.events ?? [];
  if (!same(a.writes ?? {}, b.writes ?? {})) own.writes = b.writes ?? {};
  return Object.keys(own).length > 1 ? [own, ...inside] : inside;
}

/** A tree with changes made in it: what a host that keeps its own copy does with them. */
export function apply(tree: Tree, changes: readonly Change[]): Tree {
  const by = new Map<string, Change>();
  for (const c of changes) by.set(c.id, c);
  const old = new Map<string, Node>();
  for (const n of everyNode(tree.nodes)) old.set(n.id, n);
  const child = (c: Child): Node => ("keep" in c ? node(old.get(c.keep)!) : c.node);
  const input = (x: Input): Input => {
    if ("nodes" in x) return { nodes: x.nodes.map(node) };
    if ("list" in x) return { list: x.list.map(input) };
    if ("record" in x) return { record: { type: x.record.type, fields: Object.fromEntries(Object.entries(x.record.fields).map(([k, y]) => [k, input(y)])) } };
    return "some" in x ? { some: input(x.some) } : x;
  };
  const node = (n: Node): Node => {
    const c = by.get(n.id);
    const inputs: Record<string, Input> = {};
    for (const k of new Set([...Object.keys(n.inputs), ...Object.keys(c?.inputs ?? {}), ...Object.keys(c?.children ?? {})])) {
      const given = c?.inputs?.[k];
      const kids = c?.children?.[k];
      inputs[k] = given ?? (kids ? { nodes: kids.map(child) } : input(n.inputs[k]));
    }
    const out: Node = { ...n, inputs };
    if (c?.events) out.events = c.events;
    if (c?.writes) out.writes = c.writes;
    if (!out.events?.length) delete out.events;
    if (!Object.keys(out.writes ?? {}).length) delete out.writes;
    return out;
  };
  const top = by.get("")?.children?.nodes;
  return { nodes: top ? top.map(child) : tree.nodes.map(node) };
}
