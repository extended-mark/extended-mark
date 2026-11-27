// What is drawn, as the nodes a widget is given (schemas/tree.schema.json): one that nothing changed in is the one
// it was.

import { Failure } from "@extended-mark/runtime/ops";
import type { Value } from "@extended-mark/runtime/values";
import type { Input, Node } from "@extended-mark/runtime";
import type { Contracts, Drawn, Kit, Widget } from "./kit.js";

interface Made {
  node: Node;
  fns: string[];
}

interface HeldFn {
  fn: Value;
  deps: Map<string, number>;
  version: number;
}

export class Nodes {
  private made = new WeakMap<Drawn, Made>();
  private fns = new Map<string, HeldFn>();

  constructor(private readonly m: Kit) {
    m.live = (v, at, fns) => {
      if (v.kind === "nodes") return { nodes: (v.nodes as unknown as Drawn[]).map((d) => this.inside(d, fns)) };
      fns.push(at);
      return { fn: { version: this.held(at, v).version } };
    };
  }

  all(): Node[] {
    return this.m.drawn.map((d) => this.node(d));
  }

  of(id: string): Node | undefined {
    const d = this.m.find(id);
    return d && this.made.get(d)?.node;
  }

  node(d: Drawn): Node {
    let m = this.made.get(d);
    if (!m || !m.fns.every((k) => this.fresh(k))) {
      const fns: string[] = [];
      m = { node: this.make(d, fns), fns };
      this.made.set(d, m);
    }
    return m.node;
  }

  private make(d: Drawn, fns: string[]): Node {
    if (d.failed !== undefined) return { id: d.k, widget: d.w.n, inputs: {}, failed: d.failed };
    const inputs: Record<string, Input> = {};
    for (const [k, v] of d.props) inputs[k] = this.m.input(v, `${d.k}\n${k}`, fns);
    for (const [k, kids] of d.slots) inputs[k] = { nodes: kids.map((n) => this.inside(n, fns)) };
    const node: Node = { id: d.k, widget: d.w.n, inputs };
    const events = [...d.ev.keys()].filter((e) => listens(d, e));
    if (events.length) node.events = events;
    const writes: Node["writes"] = {};
    for (const [name, , c] of (d.w as Widget).mu ?? []) writes[name] = this.contract(d.wr.get(name)?.c ?? c);
    if (Object.keys(writes).length) node.writes = writes;
    return node;
  }

  private contract(c: Contracts): NonNullable<Node["writes"]>[string] {
    return (typeof c === "function" ? c(this.m) : c) as NonNullable<Node["writes"]>[string];
  }

  private inside(d: Drawn, fns: string[]): Node {
    const node = this.node(d);
    fns.push(...this.made.get(d)!.fns);
    return node;
  }

  /** A fn input called, as a shade calls its shader. */
  call(id: string, input: string, args: Input[]): Input {
    const x = this.m.find(id)?.props.get(input);
    const f = x?.kind === "opt" ? x.value : x;
    if (f?.kind !== "fn") throw new Failure("Ir", `${id} has no fn \`${input}\``);
    const held = this.held(`${id}\n${input}`, f);
    const { value, deps } = this.m.reading(() => this.m.apply(f, args.map((a) => this.m.fromInput(a))));
    for (const [k, v] of deps) held.deps.set(k, v);
    return value ? this.m.plain(value) : { none: {} };
  }

  /** A fn as it is held: another when it was given other values, or something its calls read changed. */
  private held(at: string, fn: Value): HeldFn {
    let h = this.fns.get(at);
    if (!h) this.fns.set(at, (h = { fn, deps: new Map(), version: 0 }));
    else if (!this.m.sameFn(h.fn, fn) || !this.m.current(h.deps)) this.fns.set(at, (h = { fn, deps: new Map(), version: h.version + 1 }));
    return h;
  }

  private fresh(at: string): boolean {
    const h = this.fns.get(at);
    return !!h && this.m.current(h.deps);
  }
}

/** Whether an event does anything. A handler that only passes the event on counts when the caller of the widget it
 * passes it to set that event in turn. */
function listens(d: Drawn, event: string): boolean {
  let ev = d.ev.get(event);
  for (let depth = 0; ev && depth < 64; depth++) {
    if (!ev.h.only) return true;
    ev = ev.e.o?.ev.get(ev.h.only);
  }
  return false;
}
