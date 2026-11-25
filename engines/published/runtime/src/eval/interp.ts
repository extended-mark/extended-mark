// The evaluator: it walks the IR's statements and expressions, with the limits of page 6. A port
// of compiler/xtmd/src/eval/interp.rs, kept to its order of evaluation, its failures and messages,
// its step, item and depth counting, and its item identities.
//
// What lives outside a fn (a widget's cells and inputs, `$state`, the host's channels) is reached
// through a Host. Where Rust returns `Err(Failure)`, this throws a `Failure`.

import type * as ir from "../generated/ir.js";
import { Num } from "./num.js";
import { Failure, MAX_DEPTH, Ops, int, outerDebug, type Binding, type Outer, type Place, type PlaceRoot, type Step } from "./ops.js";
import * as V from "./value.js";
import type { CaseValue, Entry, FnValue, RecordValue, Value } from "./value.js";
import * as natives from "./natives.js";

/** Everything outside a fn: a widget instance, the document, the host, the engine. */
export interface Host {
  read(m: Machine, what: Outer, owner?: number): Value;
  /** A part of an outer value, at a place in it: `m.valueAt(place)`, for a host that minds what is read. */
  readAt?(m: Machine, place: Place): Value;
  write(m: Machine, what: Outer, owner: number | undefined, value: Value): void;
  /** An extern fn of a pack or of the host: its value, `null` when it returns nothing, and
   * `undefined` when nobody supplies it (Rust's `None`), so its default body runs if it has one.
   * Left out, nobody supplies any. */
  callExtern?(m: Machine, name: string, args: readonly Value[]): Value | undefined | null;
  /** `invoke event(args)`. Left out, it fails as NotConstant, as it does for the compiler. */
  invoke?(m: Machine, event: string, args: Value[], owner: number | undefined): void;
  /** The value a token case stands for, from the engine's tokens. Left out, the build's own:
   * `m.defaultTokenValue(c)`. */
  tokenValue?(m: Machine, c: CaseValue): Value;
}

/** A host for the compiler: only constants can be worked out. */
export class Constants implements Host {
  read(m: Machine, what: Outer, _owner?: number): Value {
    if (what.kind === "let") {
      const v = m.prog.lets.get(what.name);
      if (v !== undefined) {
        return v;
      }
    }
    throw new Failure("NotConstant", `${outerDebug(what)} is only known when the page runs`);
  }

  write(_m: Machine, what: Outer, _owner: number | undefined, _value: Value): void {
    throw new Failure("NotConstant", `${outerDebug(what)} can't change while a constant is worked out`);
  }
}

type Var = [name: string, binding: Binding];

interface Frame {
  vars: Var[];
  owner: number | undefined;
}

/** `null` goes on; an object returns, with a value or none. */
type Flow = null | { readonly value: Value | undefined };

export * from "./ops.js";

/** The declarations a program's code needs, by their full names. */
export class Program {
  readonly fns = new Map<string, ir.FnDecl>();
  readonly externs = new Map<string, ir.ExternFn>();
  readonly records = new Map<string, ir.Record>();
  readonly enums = new Map<string, ir.Enum>();
  readonly tokens = new Map<string, ir.Token>();
  readonly aliases = new Map<string, ir.Type>();
  readonly interfaces = new Map<string, ir.Interface>();
  /** Constant lets, already worked out. */
  readonly lets = new Map<string, Value>();

  add(d: ir.Decl): void {
    if ("fn" in d) {
      this.fns.set(d.fn.name, d.fn);
    } else if ("extern" in d) {
      if ("fn" in d.extern) {
        this.externs.set(d.extern.fn.name, d.extern.fn);
      } else if ("channel" in d.extern) {
        // a channel's fns are the host's: `doc/scores.saved`
        const c = d.extern.channel;
        for (const f of (c.fns ?? [])) {
          this.externs.set(`${c.name}.${f.name}`, { name: `${c.name}.${f.name}`, params: f.params, returns: f.returns, supplier: c.supplier });
        }
      }
    } else if ("record" in d) {
      this.records.set(d.record.name, d.record);
    } else if ("enum" in d) {
      this.enums.set(d.enum.name, d.enum);
    } else if ("token" in d) {
      this.tokens.set(d.token.name, d.token);
    } else if ("type" in d) {
      this.aliases.set(d.type.name, d.type.is);
    } else if ("interface" in d) {
      this.interfaces.set(d.interface.name, d.interface);
    }
  }

  static fromModule(m: ir.Module): Program {
    const p = new Program();
    for (const d of m.declarations) {
      p.add(d.declare);
    }
    return p;
  }
}

/** The single key of an IR union's object: `{ num: "1" }` is a "num". */
function tag(o: object): string {
  for (const k in o) {
    return k;
  }
  return "";
}

const INSIDE: Step = { kind: "inside" };

const EMPTY: readonly never[] = [];

export class Machine extends Ops {
  readonly prog: Program;
  host: Host;
  private frames: Frame[] = [{ vars: [], owner: undefined }];

  constructor(prog: Program, host: Host) {
    super();
    this.prog = prog;
    this.host = host;
  }

  private frame(): Frame {
    return this.frames[this.frames.length - 1];
  }

  private owner(): number | undefined {
    return this.frame().owner;
  }

  /** Works out an expression with these locals, inside the widget instance `owner`: how a runtime
   * or the Markdown writer reads a node's values. */
  evalIn(locals: readonly (readonly [string, Value])[], owner: number | undefined, e: ir.Expr): Value {
    const vars: Var[] = locals.map(([n, v]) => [n, { value: v }]);
    this.frames.push({ vars, owner });
    try {
      return this.eval(e);
    } finally {
      this.frames.pop();
    }
  }

  // ---- for a runtime: a view's nodes read with the locals around them, values or references
  // (not in interp.rs, whose Markdown writer reads values only; each runs as the fn statements do)

  /** How many of a view's expressions were worked out, for a test that counts them. */
  worked = 0;

  private withFrame<T>(bindings: readonly (readonly [string, Binding])[], owner: number | undefined, f: () => T): T {
    this.worked++;
    this.frames.push({ vars: bindings.map(([n, b]) => [n, b]), owner });
    try {
      return f();
    } finally {
      this.frames.pop();
    }
  }

  /** `evalIn`, with locals that may be references: a `mut for` row's item, an `if (mut …)`. */
  evalWith(bindings: readonly (readonly [string, Binding])[], owner: number | undefined, e: ir.Expr): Value {
    return this.withFrame(bindings, owner, () => this.eval(e));
  }

  /** Writes `value` where an IR place says, its locals and positions read with these bindings. */
  setPlace(bindings: readonly (readonly [string, Binding])[], owner: number | undefined, place: ir.Place, value: Value): void {
    this.withFrame(bindings, owner, () => this.writePlace(this.place(place), value));
  }

  /** The variable a place is in. */
  rootOf(bindings: readonly (readonly [string, Binding])[], owner: number | undefined, place: ir.Place): PlaceRoot {
    return this.withFrame(bindings, owner, () => this.place(place).root);
  }

  /** Opens an opt as `if (let …)` and `if (mut …)` do: the binding of its name, or `undefined`
   * when there is nothing inside. */
  openCond(bindings: readonly (readonly [string, Binding])[], owner: number | undefined, open: ir.CondOpen): Binding | undefined {
    return this.withFrame(bindings, owner, () => {
      if (!this.cond({ open })) {
        return undefined;
      }
      const vars = this.frame().vars;
      return vars[vars.length - 1][1];
    });
  }

  /** The rows a `for` walks, as the statement walks them (a step each): each row's locals (its
   * position, when it has one, then its name: the item, or for `mut for` a reference to the real
   * item) and its key: the item's identity for a list, the number for a range. */
  loopRows(bindings: readonly (readonly [string, Binding])[], owner: number | undefined, f: ir.NodeFor | ir.StmtFor): { key: number | string; bindings: [string, Binding][] }[] {
    return this.withFrame(bindings, owner, () => {
      const rows: { key: number | string; bindings: [string, Binding][] }[] = [];
      const row = (k: number, key: number | string, b: Binding): void => {
        this.step();
        const out: [string, Binding][] = [];
        if (f.position !== undefined) {
          out.push([f.position, { value: V.num(int(k)) }]);
        }
        out.push([f.name, b]);
        rows.push({ key, bindings: out });
      };
      if (f.mut) {
        const listPlace = this.placeOf(f.over);
        const lv = this.readPlace(listPlace);
        if (lv.kind !== "list") {
          throw Failure.ir("`mut for` over something that isn't a list");
        }
        lv.items.forEach((i, k) => row(k, i.id, { ref: { root: listPlace.root, path: [...listPlace.path, { kind: "itemOpt", id: i.id }] } }));
        return rows;
      }
      const over = this.eval(f.over);
      if (over.kind === "list") {
        over.items.forEach((i, k) => row(k, i.id, { value: i.value }));
      } else if (over.kind === "range") {
        this.walkRange(over).forEach((v, k) => row(k, V.asNum(v)?.show() ?? String(k), { value: v }));
      } else {
        throw Failure.ir("`for` over something that isn't a list or a range");
      }
      return rows;
    });
  }

  /** The calls some nodes make, through their loops and branches, each a view of its own: what a
   * list slot is when it is read. */
  calls(nodes: readonly ir.Node[], bindings: readonly (readonly [string, Binding])[], owner: number | undefined, key: string): Value[] {
    return this.withFrame(bindings, owner, () => {
      const out: Value[] = [];
      this.callsInto(nodes, key, out);
      return out;
    });
  }

  /** `key` is who the widget around the calls is: each call is who it is there. */
  private callsInto(nodes: readonly ir.Node[], key: string, out: Value[]): void {
    const at = (site: string): string => (key ? `${key}/${site}` : site);
    const depth = this.frame().vars.length;
    for (const n of nodes) {
      const here = this.frame().vars.length;
      if ("extern" in n || "use" in n) {
        const x = ("extern" in n ? n.extern : n.use) as ir.NodeExtern & ir.NodeUse;
        const given = ("extern" in n ? n.extern.props : n.use.args) as Record<string, ir.Expr>;
        const args = new Map<string, Value>();
        for (const [k, e] of V.sortedEntries(given)) args.set(k, this.eval(e));
        out.push({ kind: "view", widget: x.name, args, slots: x.slots ?? {}, events: x.events ?? {}, exprs: given, captured: this.frame().vars.slice(), owner: this.owner(), at: at(x.site) });
      } else if ("for" in n) {
        for (const row of this.loopRows(this.frame().vars, this.owner(), n.for)) {
          this.frame().vars.push(...row.bindings);
          this.callsInto(n.for.body, at(`${n.for.site}[${row.key}]`), out);
          this.truncate(here);
        }
      } else if ("if" in n) {
        let done = false;
        for (const b of n.if.branches) {
          if (this.cond(b.when)) {
            this.callsInto(b.then, key, out);
            done = true;
          }
          this.truncate(here);
          if (done) break;
        }
        if (!done) this.callsInto(n.if.else ?? [], key, out);
      } else if ("match" in n) {
        const v = this.eval(n.match.value);
        let done = false;
        for (const c of n.match.cases) {
          if (this.patterns(c.when, v)) {
            this.callsInto(c.then, key, out);
            done = true;
          }
          this.truncate(here);
          if (done) break;
        }
        if (!done) this.callsInto(n.match.else ?? [], key, out);
      } else if ("declare" in n) {
        // a `let` is seen by the nodes after it
        this.bind(n.declare.let.name, { value: this.eval(n.declare.let.value) });
      } else if ("place" in n) {
        this.callsOf(this.eval(n.place.view), at(n.place.site), out);
      }
    }
    this.truncate(depth);
  }

  private callsOf(v: Value, key: string, out: Value[]): void {
    if (v.kind === "opt") {
      if (v.value !== null) this.callsOf(v.value, key, out);
    } else if (v.kind === "list") {
      v.items.forEach((i, k) => this.callsOf(i.value, `${key}[${k}]`, out));
    } else if (v.kind === "view") {
      out.push(v.at === undefined ? { ...v, at: key } : v);
    } else if (v.kind === "content") {
      if ("parts" in v.of) v.of.parts.forEach((p, k) => this.callsOf(p, `${key}/j${k}`, out));
      else out.push(...this.calls(v.of.nodes, v.of.captured, v.of.owner, v.of.at?.key ?? key));
    }
  }

  /** A slot read as a value: a list slot is its calls, a widget's the one call it holds, and any
   * other what it was filled with. */
  slotValue(type: ir.Type | undefined, nodes: readonly ir.Node[], captured: readonly [string, Binding][], owner: number | undefined, at: { path: string; key: string }): Value {
    // a filled `opt` slot is what it holds, as a plain value where an opt is wanted is
    const ty = type?.name === "core/opt" ? type.args?.[0] : type;
    switch (ty?.name) {
      case "core/list":
        return this.makeList(this.calls(nodes, captured, owner, at.key));
      case undefined:
      case "core/view":
      case "core/rich_text":
        return { kind: "content", of: { nodes, captured, owner, at }, blocks: false };
      default:
        return this.calls(nodes, captured, owner, at.key)[0] ?? V.none();
    }
  }

  private bind(name: string, b: Binding): void {
    this.frame().vars.push([name, b]);
  }

  private truncate(mark: number): void {
    const vars = this.frame().vars;
    if (vars.length > mark) {
      vars.length = mark;
    }
  }

  /** The current frame's newest local of this name, by its index. */
  private lookup(name: string): number {
    const vars = this.frame().vars;
    for (let k = vars.length - 1; k >= 0; k--) {
      if (vars[k][0] === name) {
        return k;
      }
    }
    return -1;
  }

  protected override work(e: ir.Expr): Value {
    return this.eval(e);
  }

  protected override rule(r: ir.Require, record: RecordValue): Value | undefined {
    const args: Binding[] = record.fields.map(([, v]) => ({ value: v }));
    const params: ir.Param[] = record.fields.map(([n]) => ({ name: n, type: { name: "core/any" } }));
    return this.runFn(params, r.body, args, undefined);
  }

  protected override knows(name: string): boolean {
    return this.prog.fns.has(name) || natives.exists(name) || this.prog.externs.has(name);
  }

  protected override tokenValue(c: CaseValue): Value {
    return this.host.tokenValue !== undefined ? this.host.tokenValue(this, c) : this.defaultTokenValue(c);
  }

  protected override partOf(p: Place): Value {
    return this.host.readAt !== undefined ? this.host.readAt(this, p) : this.valueAt(p);
  }

  private hostRead(what: Outer, owner: number | undefined): Value {
    return this.host.read(this, what, owner);
  }

  // ---- places

  protected override readRoot(root: PlaceRoot): Value {
    if (root.kind === "outer") {
      return this.hostRead(root.outer, root.owner);
    }
    const frame = this.frames[root.frame];
    if (frame === undefined) {
      throw Failure.ir("a reference outlived its fn");
    }
    for (let k = frame.vars.length - 1; k >= 0; k--) {
      const [n, b] = frame.vars[k];
      if (n === root.name) {
        return "value" in b ? b.value : this.readPlace(b.ref);
      }
    }
    throw Failure.ir(`no local \`${root.name}\``);
  }

  protected override writeRoot(root: PlaceRoot, value: Value): void {
    if (root.kind === "outer") {
      this.host.write(this, root.outer, root.owner, value);
      return;
    }
    const frame = this.frames[root.frame];
    if (frame === undefined) {
      throw Failure.ir("a reference outlived its fn");
    }
    for (let k = frame.vars.length - 1; k >= 0; k--) {
      const [n, b] = frame.vars[k];
      if (n === root.name) {
        if ("value" in b) {
          // a fn given the variable writes it through a `mut` parameter, whose contract may be wider
          if (b.type !== undefined && root.frame < this.frames.length - 1 && !this.fits(value, b.type)) {
            throw new Failure("Rule", `${n} is outside its contract`);
          }
          frame.vars[k] = [n, { value, type: b.type }];
          return;
        }
        this.writePlace(b.ref, value);
        return;
      }
    }
    throw Failure.ir(`no local \`${root.name}\``);
  }

  private place(p: ir.Place): Place {
    let root: PlaceRoot;
    let path: Step[];
    const r = p.root;
    if ("local" in r) {
      const k = this.lookup(r.local);
      if (k < 0) {
        throw Failure.ir(`no local \`${r.local}\``);
      }
      const b = this.frame().vars[k][1];
      if ("ref" in b) {
        root = b.ref.root;
        path = b.ref.path.slice();
      } else {
        root = { kind: "local", frame: this.frames.length - 1, name: r.local };
        path = [];
      }
    } else if ("cell" in r) {
      root = { kind: "outer", outer: { kind: "cell", name: r.cell }, owner: this.owner() };
      path = [];
    } else if ("input" in r) {
      root = { kind: "outer", outer: { kind: "input", name: r.input }, owner: this.owner() };
      path = [];
    } else {
      root = { kind: "outer", outer: { kind: "state", name: r.$state }, owner: undefined };
      path = [];
    }
    for (const s of (p.path ?? [])) {
      if ("field" in s) {
        path.push({ kind: "field", name: s.field.name });
      } else {
        const at = this.eval(s.item.at);
        const n = V.asNum(at)?.toI64() ?? null;
        if (n === null) {
          throw new Failure("Position", "a position that isn't a whole number");
        }
        path.push({ kind: "index", index: n });
      }
    }
    return { root, path };
  }

  // ---- statements

  private block(stmts: readonly ir.Stmt[]): Flow {
    const mark = this.frame().vars.length;
    let flow: Flow = null;
    for (const s of stmts) {
      flow = this.stmt(s);
      if (flow !== null) {
        break;
      }
    }
    this.truncate(mark);
    return flow;
  }

  private stmt(s: ir.Stmt): Flow {
    this.step();
    switch (tag(s)) {
      case "declare": {
        const d = (s as { declare: ir.StmtDeclare }).declare;
        const l = "let" in d ? d.let : d.mut;
        const v = this.eval(l.value);
        this.bind(l.name, "mut" in d ? { value: v, type: l.type } : { value: v });
        break;
      }
      case "set": {
        const set = (s as { set: ir.StmtSet }).set;
        const v = this.eval(set.value);
        const p = this.place(set.place);
        this.writePlace(p, v);
        break;
      }
      case "return": {
        const r = (s as { return: ir.StmtReturn }).return;
        return { value: r.value !== undefined ? this.eval(r.value) : undefined };
      }
      case "if": {
        const i = (s as { if: ir.StmtIf }).if;
        for (const b of i.branches) {
          const mark = this.frame().vars.length;
          if (this.cond(b.when)) {
            try {
              return this.block(b.then);
            } finally {
              this.truncate(mark);
            }
          }
        }
        return this.block(i.else ?? EMPTY);
      }
      case "for":
        return this.forLoop((s as { for: ir.StmtFor }).for);
      case "match": {
        const m = (s as { match: ir.StmtMatch }).match;
        const v = this.eval(m.value);
        for (const c of m.cases) {
          const mark = this.frame().vars.length;
          if (this.patterns(c.when, v)) {
            try {
              return this.block(c.then);
            } finally {
              this.truncate(mark);
            }
          }
        }
        return this.block(m.else ?? EMPTY);
      }
      case "invoke": {
        const i = (s as { invoke: ir.StmtInvoke }).invoke;
        const args: Value[] = [];
        for (const a of i.args) {
          args.push(this.eval(a));
        }
        const owner = this.owner();
        if (this.host.invoke === undefined) {
          throw new Failure("NotConstant", `\`invoke ${i.event}\` runs only when the page runs`);
        }
        this.host.invoke(this, i.event, args, owner);
        break;
      }
      case "call":
        this.callExpr((s as { call: ir.Call }).call);
        break;
      case "apply": {
        const a = (s as { apply: ir.StmtApply }).apply;
        const f = this.eval(a.fn);
        const args = this.args(a.args);
        this.apply(f, args);
        break;
      }
      default:
        throw Failure.ir(`a statement \`${tag(s)}\` this evaluator doesn't know`);
    }
    return null;
  }

  private cond(c: ir.Cond): boolean {
    if ("cond" in c) {
      const b = V.asBool(this.eval(c.cond));
      if (b === undefined) {
        throw Failure.ir("a condition that isn't a bool");
      }
      return b;
    }
    const o = c.open;
    if (!o.mut) {
      const v = this.eval(o.opt);
      if (v.kind === "opt") {
        if (v.value === null) {
          return false;
        }
        this.bind(o.name, { value: v.value });
        return true;
      }
      this.bind(o.name, { value: v });
      return true;
    }
    let target = this.placeOf(o.opt);
    // `xs[i]` is an opt: none past the end, and otherwise the item itself, by its identity
    const last = target.path[target.path.length - 1];
    if (last !== undefined && last.kind === "index") {
      const list: Place = { root: target.root, path: target.path.slice(0, -1) };
      const lv = this.readPlace(list);
      const item = lv.kind === "list" && last.index >= 0 ? lv.items[last.index] : undefined;
      if (item === undefined) {
        return false;
      }
      target = { root: target.root, path: [...list.path, { kind: "item", id: item.id }] };
    }
    const tl = target.path[target.path.length - 1];
    let inside: Place;
    let now: Value;
    if (tl !== undefined && tl.kind === "itemOpt") {
      inside = { root: target.root, path: [...target.path.slice(0, -1), { kind: "item", id: tl.id }] };
      now = this.readPlace(target);
    } else {
      inside = { root: target.root, path: [...target.path, INSIDE] };
      now = this.readPlace(target);
    }
    if (now.kind === "opt") {
      if (now.value === null) {
        return false;
      }
      this.bind(o.name, { ref: inside });
      return true;
    }
    // a plain value where an opt is wanted is always there
    this.bind(o.name, { ref: { root: inside.root, path: inside.path.slice(0, -1) } });
    return true;
  }

  /** The place an expression stands for: a `place`, or a local that refers to one. */
  private placeOf(e: ir.Expr): Place {
    switch (tag(e)) {
      case "place":
        return this.place((e as { place: ir.Place }).place);
      case "get": {
        const g = (e as { get: ir.Get }).get;
        if ("local" in g) {
          return this.place({ root: { local: g.local }, path: [] });
        }
        if ("cell" in g) {
          return this.place({ root: { cell: g.cell }, path: [] });
        }
        if ("input" in g) {
          return this.place({ root: { input: g.input }, path: [] });
        }
        if ("$state" in g) {
          return this.place({ root: { $state: g.$state }, path: [] });
        }
        break;
      }
      case "field": {
        const f = (e as { field: ir.ExprField }).field;
        const p = this.placeOf(f.value);
        return { root: p.root, path: [...p.path, { kind: "field", name: f.name }] };
      }
    }
    throw Failure.ir("a mut place that isn't a variable");
  }

  private forLoop(f: ir.StmtFor): Flow {
    if (f.mut) {
      const listPlace = this.placeOf(f.over);
      const lv = this.readPlace(listPlace);
      if (lv.kind !== "list") {
        throw Failure.ir("`mut for` over something that isn't a list");
      }
      const ids = lv.items.map((i) => i.id);
      for (let k = 0; k < ids.length; k++) {
        this.step();
        const mark = this.frame().vars.length;
        if (f.position !== undefined) {
          this.bind(f.position, { value: V.num(int(k)) });
        }
        this.bind(f.name, { ref: { root: listPlace.root, path: [...listPlace.path, { kind: "itemOpt", id: ids[k] }] } });
        const flow = this.block(f.body);
        this.truncate(mark);
        if (flow !== null) {
          return flow;
        }
      }
      return null;
    }
    const over = this.eval(f.over);
    let values: readonly Value[];
    if (over.kind === "list") {
      values = over.items.map((i) => i.value);
    } else if (over.kind === "range") {
      values = this.walkRange(over);
    } else {
      throw Failure.ir("`for` over something that isn't a list or a range");
    }
    for (let k = 0; k < values.length; k++) {
      this.step();
      const mark = this.frame().vars.length;
      if (f.position !== undefined) {
        this.bind(f.position, { value: V.num(int(k)) });
      }
      this.bind(f.name, { value: values[k] });
      const flow = this.block(f.body);
      this.truncate(mark);
      if (flow !== null) {
        return flow;
      }
    }
    return null;
  }

  private patterns(pats: readonly ir.Pattern[], v: Value): boolean {
    for (const p of pats) {
      let hit = false;
      if ("case" in p) {
        const c = p.case;
        if (v.kind === "case") {
          if (v.ty === c.enum && v.name === c.name) {
            const binds = c.binds ?? EMPTY;
            const n = Math.min(binds.length, v.values.length);
            for (let k = 0; k < n; k++) {
              this.bind(binds[k], { value: v.values[k][1] });
            }
            hit = true;
          }
        } else if (v.kind === "opt") {
          if (c.name === "SOME" && v.value !== null) {
            const name = c.binds?.[0];
            if (name !== undefined) {
              this.bind(name, { value: v.value });
            }
            hit = true;
          } else if (c.name === "NONE" && v.value === null) {
            hit = true;
          }
        } else if (v.kind === "bool") {
          hit = c.name === (v.b ? "TRUE" : "FALSE");
        }
      } else if ("num" in p) {
        if (v.kind === "num") {
          const n = Num.literal(p.num);
          hit = n !== null && n.equals(v.n);
        }
      } else if ("text" in p) {
        hit = v.kind === "text" && v.s === p.text;
      } else if ("bool" in p) {
        hit = v.kind === "bool" && v.b === p.bool;
      }
      if (hit) {
        return true;
      }
    }
    return false;
  }

  // ---- expressions

  eval(e: ir.Expr): Value {
    switch (tag(e)) {
      case "num": {
        const s = (e as { num: string }).num;
        const n = Num.literal(s);
        if (n === null) {
          throw Failure.ir(`\`${s}\` is not a number`);
        }
        return V.num(n);
      }
      case "text":
        return V.text((e as { text: string }).text);
      case "bool":
        return V.bool((e as { bool: boolean }).bool);
      case "duration": {
        const n = Num.literal((e as { duration: string }).duration);
        if (n === null) {
          throw Failure.ir("a duration that isn't a number");
        }
        return V.duration(n);
      }
      case "list": {
        const items = (e as { list: ir.Expr[] }).list;
        const out: Value[] = new Array(items.length);
        for (let k = 0; k < items.length; k++) {
          out[k] = this.eval(items[k]);
        }
        return this.makeList(out);
      }
      case "record": {
        const r = (e as { record: ir.ExprRecord }).record;
        return this.makeRecord(r.type, this.given(r.fields ?? {}), false);
      }
      case "case": {
        const c = (e as { case: ir.ExprCase }).case;
        const values: Entry[] = [];
        for (const [k, x] of V.sortedEntries(c.values)) {
          values.push([k, this.eval(x)]);
        }
        return V.caseOf(c.enum, c.name, this.orderCaseValues(c.enum, c.name, values));
      }
      case "some":
        return V.some(this.eval((e as { some: ir.Expr }).some));
      case "none":
        return V.none();
      case "range": {
        const r = (e as { range: ir.ExprRange }).range;
        const low = r.low !== undefined ? this.eval(r.low) : null;
        const high = r.high !== undefined ? this.eval(r.high) : null;
        const step = r.step !== undefined ? this.eval(r.step) : null;
        return { kind: "range", low, high, step };
      }
      case "token": {
        const name = (e as { token: string }).token;
        const dot = name.lastIndexOf(".");
        if (dot < 0) {
          throw Failure.ir("a token without a case");
        }
        return V.caseOf(name.slice(0, dot), name.slice(dot + 1));
      }
      case "tokval": {
        const t = (e as { tokval: ir.ExprTokval }).tokval;
        const v = this.eval(t.value);
        const field = this.caseFields(t.type, t.name)[0] ?? "value";
        return V.caseOf(t.type, t.name, [[field, v]]);
      }
      case "rich": {
        const x = (e as { rich: ir.Rich }).rich;
        return { kind: "content", of: { nodes: x.nodes, captured: this.frame().vars.slice(), owner: this.owner() }, blocks: x.blocks === true };
      }
      case "sentence": {
        const x = this.eval((e as { sentence: ir.Expr }).sentence);
        return V.bool(!(x.kind === "content" && x.blocks));
      }
      case "template":
        return this.template((e as { template: ir.ExprTemplate }).template);
      case "view": {
        const v = (e as { view: ir.ViewValue }).view;
        const args = new Map<string, Value>();
        for (const [k, x] of V.sortedEntries(v.args)) {
          args.set(k, this.eval(x));
        }
        const captured = this.frame().vars.slice();
        return { kind: "view", widget: v.widget, args, slots: v.slots ?? {}, events: v.events ?? {}, exprs: v.args ?? {}, captured, owner: this.owner() };
      }
      case "get":
        return this.get((e as { get: ir.Get }).get);
      case "field": {
        const f = (e as { field: ir.ExprField }).field;
        const v = this.eval(f.value);
        return this.field(v, f.name);
      }
      case "call": {
        const c = (e as { call: ir.Call }).call;
        const v = this.callExpr(c);
        if (v === undefined) {
          throw Failure.ir(`\`${c.name}\` returns nothing`);
        }
        return v;
      }
      case "apply": {
        const a = (e as { apply: ir.ExprApply }).apply;
        const f = this.eval(a.fn);
        const args = this.args(a.args);
        const v = this.apply(f, args);
        if (v === undefined) {
          throw Failure.ir("a fn value that returns nothing");
        }
        return v;
      }
      case "op":
        return this.op((e as { op: ir.ExprOp }).op);
      case "show": {
        const v = this.eval((e as { show: ir.ExprShow }).show.value);
        return V.text(this.show(v));
      }
      case "fn": {
        const l = (e as { fn: ir.Lambda }).fn;
        return { kind: "fn", params: l.params ?? [], body: l.body, captured: this.frame().vars.slice(), owner: this.owner() };
      }
      case "place": {
        const p = this.place((e as { place: ir.Place }).place);
        return this.readPlace(p);
      }
      case "try":
        return this.tryValue((e as { try: ir.ExprTry }).try.value);
      case "with": {
        const w = (e as { with: ir.ExprWith }).with;
        const base = this.eval(w.value);
        return this.with(base, this.given(w.fields), false);
      }
    }
    throw Failure.ir(`an expression \`${tag(e)}\` this evaluator doesn't know`);
  }

  /** A record literal's fields, worked out in their names' order. */
  private given(fields: { [key: string]: ir.Expr }): [string, Value][] {
    const out: [string, Value][] = [];
    for (const [k, x] of V.sortedEntries(fields)) {
      out.push([k, this.eval(x)]);
    }
    return out;
  }

  private get(g: ir.Get): Value {
    let outer: Outer;
    if ("local" in g) {
      const k = this.lookup(g.local);
      if (k < 0) {
        throw Failure.ir(`no local \`${g.local}\``);
      }
      const b = this.frame().vars[k][1];
      return "value" in b ? b.value : this.readPlace(b.ref);
    } else if ("input" in g) {
      outer = { kind: "input", name: g.input };
    } else if ("cell" in g) {
      outer = { kind: "cell", name: g.cell };
    } else if ("$state" in g) {
      outer = { kind: "state", name: g.$state };
    } else if ("let" in g) {
      outer = { kind: "let", name: g.let };
    } else if ("slot" in g) {
      outer = { kind: "slot", name: g.slot };
    } else if ("extern" in g) {
      outer = { kind: "extern", name: g.extern };
    } else if ("token" in g) {
      return this.eval({ token: g.token });
    } else {
      throw Failure.ir(`a read \`${tag(g)}\` this evaluator doesn't know`);
    }
    return this.hostRead(outer, this.owner());
  }

  private args(args: readonly ir.Expr[]): Binding[] {
    const out: Binding[] = new Array(args.length);
    for (let k = 0; k < args.length; k++) {
      const a = args[k];
      out[k] = "place" in a ? { ref: this.place(a.place) } : { value: this.eval(a) };
    }
    return out;
  }

  private callExpr(c: ir.Call): Value | undefined {
    const args = this.args(c.args ?? []);
    return this.call(c.name, args);
  }

  /** Calls a fn by its full name: a native, an extern, one written in Extended Mark, or an interface's.
   * `undefined` when it returns nothing. */
  override call(name: string, args: readonly Binding[]): Value | undefined {
    this.step();
    const f = this.prog.fns.get(name);
    if (f !== undefined) {
      return this.runFn(f.params ?? [], f.body, args, undefined);
    }
    if (natives.exists(name)) {
      return natives.run(this, name, args);
    }
    const ext = this.prog.externs.get(name);
    if (ext !== undefined) {
      const values: Value[] = [];
      for (const a of args) {
        values.push(this.valueOf(a));
      }
      if (this.host.callExtern !== undefined) {
        const answer = this.host.callExtern(this, name, values);
        if (answer !== undefined) {
          return answer === null ? undefined : answer;
        }
      }
      if (ext.default !== undefined && ext.default.length > 0) {
        return this.runFn(ext.params ?? [], ext.default, args, undefined);
      }
      throw new Failure("NotConstant", `nobody supplies \`${name}\``);
    }
    const dot = name.lastIndexOf(".");
    if (dot >= 0) {
      const iface = name.slice(0, dot);
      if (this.prog.interfaces.has(iface)) {
        return this.dispatch(iface, name.slice(dot + 1), args);
      }
    }
    throw Failure.ir(`no fn \`${name}\``);
  }

  private runFn(params: readonly ir.Param[], body: readonly ir.Stmt[], args: readonly Binding[], closure: FnValue | undefined): Value | undefined {
    if (this.depth >= MAX_DEPTH) {
      throw Failure.ofLimit("Depth", "calls more than 64 deep");
    }
    const vars: Var[] = [];
    let owner = this.owner();
    if (closure !== undefined) {
      for (const c of closure.captured) {
        vars.push(c);
      }
      owner = closure.owner;
    }
    const n = Math.min(params.length, args.length);
    for (let k = 0; k < n; k++) {
      const p = params[k];
      const a = args[k];
      vars.push([p.name, p.mut === true ? a : { value: this.valueOf(a) }]);
    }
    this.depth += 1;
    this.frames.push({ vars, owner });
    let flow: Flow;
    try {
      flow = this.block(body);
    } finally {
      this.frames.pop();
      this.depth -= 1;
    }
    return flow === null ? undefined : flow.value;
  }

  /** Calls a fn value. `undefined` when it returns nothing. */
  apply(f: Value, args: readonly Binding[]): Value | undefined {
    this.step();
    if (f.kind === "fn") {
      return this.runFn(f.params, f.body, args, f);
    }
    throw Failure.ir("a call of something that isn't a fn");
  }

  private op(op: ir.ExprOp): Value {
    const a = op.args;
    switch (op.name) {
      case "&&": {
        const l = V.asBool(this.eval(a[0])) ?? false;
        return V.bool(l && (V.asBool(this.eval(a[1])) ?? false));
      }
      case "||": {
        const l = V.asBool(this.eval(a[0])) ?? false;
        return V.bool(l || (V.asBool(this.eval(a[1])) ?? false));
      }
      case "?": {
        const c = V.asBool(this.eval(a[0])) ?? false;
        return this.eval(c ? a[1] : a[2]);
      }
      case "!":
        return V.bool(!(V.asBool(this.eval(a[0])) ?? false));
      case "-": {
        if (a.length === 1) {
          const v = this.eval(a[0]);
          if (v.kind === "num") {
            return V.num(v.n.neg());
          }
          if (v.kind === "duration") {
            return V.duration(v.n.neg());
          }
          throw Failure.ir("`-` on something that isn't a number");
        }
        break;
      }
    }
    const l = this.eval(a[0]);
    const r = this.eval(a[1]);
    return this.arith(op.name, l, r);
  }

  private tryValue(e: ir.Expr): Value {
    let attempt: () => Value;
    switch (tag(e)) {
      case "record": {
        const r = (e as { record: ir.ExprRecord }).record;
        const given = this.given(r.fields ?? {});
        attempt = () => this.makeRecord(r.type, given, true);
        break;
      }
      case "with": {
        const w = (e as { with: ir.ExprWith }).with;
        const base = this.eval(w.value);
        const given = this.given(w.fields);
        attempt = () => this.with(base, given, true);
        break;
      }
      case "case": {
        const c = (e as { case: ir.ExprCase }).case;
        const v = this.eval(e);
        const fields = this.caseFieldTypes(c.enum, c.name);
        if (v.kind === "case") {
          const n = Math.min(v.values.length, fields.length);
          for (let k = 0; k < n; k++) {
            if (!this.fits(v.values[k][1], fields[k])) {
              return V.none();
            }
          }
        }
        return V.some(v);
      }
      case "tokval": {
        const t = (e as { tokval: ir.ExprTokval }).tokval;
        const v = this.eval(e);
        const ty = this.caseFieldTypes(t.type, t.name)[0];
        if (v.kind === "case" && ty !== undefined && v.values.length > 0) {
          if (!this.fits(v.values[0][1], ty)) {
            return V.none();
          }
        }
        return V.some(v);
      }
      default:
        attempt = () => this.eval(e);
    }
    try {
      return V.some(attempt());
    } catch (f) {
      if (f instanceof Failure && f.kind === "Rule") {
        this.lastRule = f.rule;
        return V.none();
      }
      throw f;
    }
  }

  // ---- prose values

  private template(t: ir.ExprTemplate): Value {
    if (t.type.name === "core/text") {
      let out = "";
      for (let k = 0; k < t.pieces.length; k++) {
        out += t.pieces[k];
        const h = t.holes[k];
        if (h !== undefined) {
          const v = this.eval(h);
          out += this.show(v);
        }
      }
      return V.text(out);
    }
    const pieces = this.makeList(t.pieces.map((p) => V.text(p)));
    const holes: Value[] = [];
    for (const h of t.holes) {
      let v = this.eval(h);
      const isView = v.kind === "view" || v.kind === "content";
      if (!isView) {
        v = V.text(this.show(v));
      }
      holes.push(V.caseOf("core/hole", isView ? "VIEW" : "TEXT", [[isView ? "child" : "value", v]]));
    }
    const holeList = this.makeList(holes);
    return V.record("core/template", [
      ["pieces", pieces],
      ["holes", holeList],
    ]);
  }
}

