// IR to a page's code. Everything the page can run is a fn of JavaScript; a page holds what its own parts reach.

import { Num } from "@extended-mark/runtime/num";
import type { Input, ir } from "@extended-mark/runtime";
import { NATIVES } from "@extended-mark/runtime/natives";

/** A local in a frame: where it is, by its place among them. */
interface Local {
  name: string;
  /** It is a reference to a place. */
  ref?: boolean;
  /** A `mut` local's type, checked when a fn it is given to writes it. */
  type?: ir.Type;
}

/** Where code is: the locals around it, and the widget it is in. */
interface At {
  locals: Local[];
  t?: ir.Template;
}

const str = JSON.stringify;

/** A record of entries as JavaScript, or nothing for none. */
const opt = (name: string, entries: string[], open = "{", close = "}"): string => (entries.length ? `${name}:${open}${entries.join(",")}${close},` : "");

export interface Generated {
  code: string;
  /** Which core widgets it draws with, by full name. */
  widgets: string[];
  /** Whether it has planes to lay out. */
  planes: boolean;
  /** Whether it takes the reader's time zone. */
  local: boolean;
  /** Whether it reads a slot as calls. */
  calls: boolean;
}

/** What is drawn by whom, and which of the document's own parts to write code for. */
export interface Choice {
  supplies: (widget: string) => boolean;
  /** The document's parts code is written for, by their places among them. Left out, all. */
  parts?: ReadonlySet<number>;
}

export function generate(m: ir.Module, choice: Choice): Generated {
  return new Generator(m, choice).run();
}

const SENTENCES: Record<string, string[]> = {
  paragraph: ["value"],
  heading: ["value"],
  table: ["head", "rows"],
  bold: ["body"],
  italic: ["body"],
  strike: ["body"],
  link: ["body"],
};

class Generator {
  private readonly decls = new Map<string, ir.Decl>();
  private readonly templates = new Map<string, ir.Template>();
  private readonly fns = new Map<string, ir.FnDecl>();
  private readonly externs = new Map<string, ir.ExternFn>();
  private readonly widgets = new Map<string, ir.ExternWidget>();
  private readonly records = new Map<string, ir.Record>();
  private readonly enums = new Map<string, ir.Enum>();
  private readonly tokens = new Map<string, ir.Token>();
  private readonly aliases = new Map<string, ir.Type>();
  private readonly interfaces = new Map<string, ir.Interface>();
  private readonly channels: ir.ExternChannel[] = [];
  private readonly lets = new Map<string, ir.LetDecl>();
  private readonly handlers = new Map<string, ir.Handler>();
  private readonly natives = new Set(NATIVES);
  private state: ir.State | undefined;
  private exposes: ir.Expose[] = [];

  /** What is written, by kind: the shells first, as they name one another. */
  private readonly consts: string[] = [];
  private readonly shells: string[] = [];
  private readonly later: string[] = [];
  private readonly named = new Map<string, string>();
  private readonly kit = new Set<string>();
  private readonly used = new Set<string>();
  private readonly drawnBy = new Set<string>();
  private n = 0;
  private planes = false;
  private calls = false;
  private ext = new Map<string, string>();
  /** The declarations values are made and checked by. */
  private readonly types = new Set<string>();

  constructor(private readonly m: ir.Module, private readonly choice: Choice) {
    for (const d of m.declarations) {
      const x = d.declare;
      if ("fn" in x) this.fns.set(x.fn.name, x.fn);
      else if ("record" in x) this.records.set(x.record.name, x.record);
      else if ("enum" in x) this.enums.set(x.enum.name, x.enum);
      else if ("token" in x) this.tokens.set(x.token.name, x.token);
      else if ("type" in x) this.aliases.set(x.type.name, x.type.is);
      else if ("interface" in x) this.interfaces.set(x.interface.name, x.interface);
      else if ("let" in x) this.lets.set(x.let.name, x.let);
      else if ("$state" in x) this.state = x.$state;
      else if ("expose" in x) this.exposes.push(x.expose);
      else if ("extern" in x) {
        const e = x.extern;
        if ("fn" in e) this.externs.set(e.fn.name, e.fn);
        else if ("widget" in e) this.widgets.set(e.widget.name, e.widget);
        else if ("channel" in e) {
          this.channels.push(e.channel);
          for (const f of e.channel.fns ?? []) this.externs.set(`${e.channel.name}.${f.name}`, { name: `${e.channel.name}.${f.name}`, params: f.params, returns: f.returns, supplier: e.channel.supplier });
        }
      }
    }
    for (const t of m.templates) {
      this.templates.set(t.name, t);
      for (const f of t.fns ?? []) this.fns.set(f.name, f);
    }
    for (const h of m.handlers ?? []) this.handlers.set(h.id, h);
  }

  run(): Generated {
    const doc = this.templates.get("doc")!;
    const at: At = { locals: [], t: undefined };
    const parts: string[] = [];
    doc.view.forEach((n, k) => {
      // a local is there for what comes after it
      if (!this.choice.parts || this.choice.parts.has(k) || "declare" in n) parts.push(this.part(n, at));
      if ("declare" in n) at.locals = [...at.locals, { name: n.declare.let.name }];
    });
    const tokens = [...this.tokens.values()].map((t) => `${str(t.name)}:{${Object.entries(t.values).map(([k, x]) => `${str(k)}:m.plain(${this.thunk(x)}(m,[]))`).join(",")}}`);
    const program = this.program(parts);
    const head: string[] = [];
    const kit = [...this.kit].sort();
    if (kit.length) head.push(`import{${kit.map((k) => `${k} as ${k}$`).join(",")}}from"@extended-mark/web/kit";`);
    const natives = [...this.used].sort();
    if (natives.length) head.push(`import{${natives.join(",")}}from"@extended-mark/runtime/natives";`);
    const code = [...head, ...this.consts, ...this.shells, ...this.later, `export const program=${program};`, `export const tokens=m=>({${tokens.join(",")}});`].join("\n");
    return { code, widgets: [...this.drawnBy].sort(), planes: this.planes, local: this.ext.has("core/moment.local"), calls: this.calls };
  }

  private k(name: string): string {
    this.kit.add(name);
    return `${name}$`;
  }

  private name(prefix: string): string {
    return `${prefix}${this.n++}`;
  }

  /** A constant, written once. */
  private constant(code: string, prefix = "k"): string {
    let n = this.named.get(code);
    if (!n) {
      this.named.set(code, (n = this.name(prefix)));
      this.consts.push(`const ${n}=${code};`);
    }
    return n;
  }

  // ---- the program

  private program(parts: string[]): string {
    const st = (this.state?.fields ?? []).map((f) => `[${str(f.name)},${this.fnOr(f.default)},${this.type(f.type)}]`);
    const q = (this.state?.requires ?? []).map((r) => this.rule(r, {}));
    const ch = this.channels.flatMap((c) => (c.values?.length ? [`${str(c.name)}:{${c.values.map((f) => `${str(f.name)}:[${this.type(f.type)},${this.fnOr(f.default)}]`).join(",")}}`] : []));
    const ex = this.exposes.map((e) => `${str(e.channel)}:{${e.fns.map((f) => `${str(f.name)}:[${this.fnOf(f.params, f.body)},[${f.params.map((p) => this.type(p.type)).join(",")}]]`).join(",")}}`);
    const fns = this.byName();
    // writing the declarations reaches fns, and their types declarations
    this.declared();
    const ext = [...this.ext].map(([n, x]) => `${str(n)}:${x}`);
    return `{${this.declared()}fns:{${fns.join(",")}},${opt("ext", ext)}${opt("st", st, "[", "]")}${opt("q", q, "[", "]")}${opt("ch", ch)}${opt("ex", ex)}v:[${parts.join(",")}]}`;
  }

  /** The fns code may call by the name of a value's own type: those of interfaces, and of every type. */
  private byName(): string[] {
    const of = new Set(["show", "before", "same", ...[...this.interfaces.values()].flatMap((i) => i.fns ?? [])]);
    const out: string[] = [];
    const ends = (name: string) => of.has(name.slice(name.lastIndexOf(".") + 1));
    for (const f of this.fns.values()) if (ends(f.name) || [...this.records.values()].some((r) => r.same === f.name)) out.push(`${str(f.name)}:${this.fn(f.name)}`);
    const native = [...this.interfaces.values()].flatMap((i) => (i.fns ?? []).map((f) => `.${f}`));
    for (const n of NATIVES) if (native.some((f) => n.endsWith(f)) && !this.fns.has(n)) out.push(`${str(n)}:${this.native(n)}`);
    for (const x of this.externs.keys()) if (ends(x) && !this.natives.has(x)) this.extern(x);
    return out;
  }

  /** The declarations values are made and checked by, as they were reached, in the IR's order. */
  private declared(): string {
    const done = new Map<string, string>();
    // writing one reaches others
    for (let was = -1; was !== this.types.size; ) {
      was = this.types.size;
      for (const n of [...this.types]) {
        if (done.has(n)) continue;
        const [r, e, t, a] = [this.records.get(n), this.enums.get(n), this.tokens.get(n), this.aliases.get(n)];
        if (r) done.set(n, `{name:${str(n)},fields:[${r.fields.map((f) => this.fieldDecl(f)).join(",")}],${r.params?.length ? `params:${str(r.params.map((p) => ({ name: p.name })))},` : ""}${opt("requires", (r.requires ?? []).map((q) => this.recordRule(q, r)), "[", "]")}${r.same ? `same:${str(r.same)}` : ""}}`);
        else if (e) done.set(n, `{name:${str(n)},${e.params?.length ? `params:${str(e.params.map((p) => ({ name: p.name })))},` : ""}cases:[${e.cases.map((c) => this.caseDecl(c)).join(",")}]}`);
        else if (t) done.set(n, `{name:${str(n)},cases:[${t.cases.map((c) => this.caseDecl(c)).join(",")}],values:{${Object.entries(t.values).map(([k, x]) => `${str(k)}:${this.fnOr(x)}`).join(",")}}}`);
        else if (a) done.set(n, this.type(a));
        else done.set(n, "");
      }
    }
    const map = (of: Map<string, unknown>) => `new Map([${[...of.keys()].filter((n) => done.get(n)).map((n) => `[${str(n)},${done.get(n)}]`).join(",")}])`;
    return `records:${map(this.records)},enums:${map(this.enums)},tokens:${this.tokenMap(done)},aliases:${map(this.aliases)},`;
  }

  /** A token's type is told from an enum's by its name, whether its values are read or not. */
  private tokenMap(done: Map<string, string>): string {
    return `new Map([${[...this.tokens.keys()].map((n) => `[${str(n)},${done.get(n) || `{name:${str(n)},cases:[],values:{}}`}]`).join(",")}])`;
  }

  private caseDecl(c: ir.CaseDecl): string {
    return c.fields?.length ? `{name:${str(c.name)},fields:[${c.fields.map((f) => this.fieldDecl(f)).join(",")}]}` : `{name:${str(c.name)}}`;
  }

  private fieldDecl(f: ir.Field): string {
    return `{name:${str(f.name)},type:${this.type(f.type)}${f.default ? `,default:${this.thunk(f.default)}` : ""}}`;
  }

  private recordRule(r: ir.Require, of: ir.Record): string {
    return `{message:${str(r.message)},${r.warn ? "warn:true," : ""}f:${this.body(r.body, { locals: of.fields.map((f) => ({ name: f.name })) })}}`;
  }

  private rule(r: ir.Require, at: Partial<At>): string {
    return `{message:${str(r.message)},${r.warn ? "warn:true," : ""}f:${this.body(r.body, { locals: [], ...at })}}`;
  }

  // ---- types

  private type(t: ir.Type): string {
    return this.constant(this.typeCode(t), "y");
  }

  private typeCode(t: ir.Type): string {
    this.types.add(t.name);
    const c = t.contract;
    const ends = (n: string, b: ir.Bounds | undefined) => (b && (b.low || b.high) ? [`${n}:{${[...(b.low ? [`low:${this.thunk(b.low)}`] : []), ...(b.high ? [`high:${this.thunk(b.high)}`] : [])].join(",")}}`] : []);
    const contract = c ? [...ends("range", c.range), ...ends("length", c.length), ...(c.step ? [`step:${this.thunk(c.step)}`] : []), ...(c.only?.length ? [`only:[${c.only.map((x) => this.thunk(x)).join(",")}]`] : [])] : [];
    return `{name:${str(t.name)}${t.args?.length ? `,args:[${t.args.map((a) => this.type(a)).join(",")}]` : ""}${t.fields?.length ? `,fields:[${t.fields.map((f) => this.fieldDecl(f)).join(",")}]` : ""}${contract.length ? `,contract:{${contract.join(",")}}` : ""}}`;
  }

  /** Whether a value of a type can be outside it, through aliases, items and opts. */
  private narrowed(t: ir.Type, depth = 0): boolean {
    if (depth > 16) return true;
    const a = this.aliases.get(t.name);
    return !!t.contract || (a !== undefined && this.narrowed(a, depth + 1)) || (t.args ?? []).some((x) => this.narrowed(x, depth + 1));
  }

  /** A constant expression as a fn of no locals. */
  private thunk(e: ir.Expr): string {
    return this.constant(fnCode(this.x(e, { locals: [] })), "x");
  }

  private fnOr(e: ir.Expr | undefined): string {
    return e ? this.thunk(e) : "undefined";
  }

  /** What a value of a type has to fit, as whoever draws a widget is told. */
  private contract(t: ir.Type | undefined): string {
    const c: Record<string, any> = {};
    let dynamic = false;
    const worked = (e: ir.Expr): unknown => {
      const x = literal(e);
      if (x !== undefined) return x;
      dynamic = true;
      return { $: this.thunk(e) };
    };
    const ends = (b: ir.Bounds) => ({ ...(b.low ? { low: worked(b.low) } : {}), ...(b.high ? { high: worked(b.high) } : {}) });
    for (let ty = t, k = 0; ty && k < 16; ty = this.aliases.get(ty.name), k++) {
      const x = ty.contract;
      if (x?.range) c.range = { ...ends(x.range), ...c.range };
      if (x?.length) c.length = { ...ends(x.length), ...c.length };
      if (x?.step) c.step ??= worked(x.step);
      if (x?.only?.length) c.only ??= x.only.map(worked);
    }
    const code = str(c).replace(/\{"\$":"(\w+)"\}/g, "m.plain($1(m,[]))");
    return this.constant(dynamic ? `m=>(${code})` : code, "c");
  }

  /** The type of the variable a place is, where the IR says it. */
  private placeType(p: ir.Place, at: At): ir.Type | undefined {
    const r = p.root;
    let t: ir.Type | undefined;
    if ("$state" in r) t = this.state?.fields.find((f) => f.name === r.$state)?.type;
    else if ("cell" in r) t = at.t?.cells?.find((c) => c.name === r.cell)?.type;
    else if ("input" in r) t = at.t?.inputs?.find((f) => f.name === r.input)?.type;
    for (const s of p.path ?? []) {
      let base = t;
      for (let k = 0; base && k < 16 && this.aliases.has(base.name); k++) base = this.aliases.get(base.name);
      if (!base) return undefined;
      if ("field" in s) t = (base.fields ?? this.records.get(base.name)?.fields)?.find((f) => f.name === s.field.name)?.type;
      else t = base.args?.[0];
    }
    return t;
  }

  // ---- fns

  private native(name: string): string {
    const id = name.slice("core/".length).replace(/\./g, "_");
    this.used.add(id);
    return id;
  }

  private fn(name: string): string {
    const key = `fn ${name}`;
    let n = this.named.get(key);
    if (!n) {
      this.named.set(key, (n = this.name("f")));
      const f = this.fns.get(name)!;
      this.shells.push(`const ${n}=${this.fnOf(f.params ?? [], f.body)};`);
    }
    return n;
  }

  private fnOf(params: readonly ir.Param[], body: readonly ir.Stmt[], around: At = { locals: [] }): string {
    const locals: Local[] = [...around.locals, ...params.map((p): Local => ({ name: p.name, ref: p.mut }))];
    const mu = params.reduce((x, p, k) => x | (p.mut ? 1 << k : 0), 0);
    return `{n:${params.length},${mu ? `mu:${mu},` : ""}f:${this.body(body, { ...around, locals })}}`;
  }

  /** A fn somebody outside gives. */
  private extern(name: string): void {
    if (this.ext.has(name)) return;
    this.ext.set(name, "");
    const x = this.externs.get(name)!;
    const host = this.m.needs.host.includes(name);
    const d = x.default?.length ? `d:${this.fnOf(x.params ?? [], x.default)},` : "";
    this.ext.set(name, `{${d}${x.returns ? `r:${this.type(x.returns)},` : ""}${host ? "host:true" : ""}}`);
  }

  private handler(id: string, at: At): string {
    const h = this.handlers.get(id)!;
    const only = h.body.length === 1 && "invoke" in h.body[0] ? h.body[0].invoke.event : undefined;
    const locals: Local[] = [...at.locals, ...(h.params ?? []).map((p): Local => ({ name: p.name }))];
    return this.constantLater(`{n:${(h.params ?? []).length},${only ? `only:${str(only)},` : ""}f:${this.body(h.body, { ...at, locals })}}`, "h");
  }

  /** Code that may name what is written later than the constants. */
  private constantLater(code: string, prefix: string): string {
    let n = this.named.get(`later ${code}`);
    if (!n) {
      this.named.set(`later ${code}`, (n = this.name(prefix)));
      this.shells.push(`const ${n}=${code};`);
    }
    return n;
  }

  // ---- statements

  private body(stmts: readonly ir.Stmt[], at: At): string {
    return `(m,b,o)=>{${this.block(stmts, at)}}`;
  }

  private block(stmts: readonly ir.Stmt[], at0: At): string {
    const at = { ...at0 };
    let out = "";
    for (const s of stmts) {
      out += "m.step();";
      if ("declare" in s) {
        const l = "let" in s.declare ? s.declare.let : s.declare.mut;
        out += `b[${at.locals.length}]=${this.x(l.value, at)};`;
        at.locals = [...at.locals, { name: l.name, type: "mut" in s.declare && this.narrowed(l.type) ? l.type : undefined }];
      } else if ("set" in s) out += `{const v=${this.x(s.set.value, at)};m.writePlace(${this.place(s.set.place, at)},v);}`;
      else if ("return" in s) out += `return${s.return.value ? ` ${this.x(s.return.value, at)}` : ""};`;
      else if ("if" in s) {
        s.if.branches.forEach((br, k) => {
          const inner = { ...at };
          const when = this.when(br.when, inner);
          out += `${k ? "else " : ""}if(${when}){${this.block(br.then, inner)}}`;
        });
        if (s.if.else?.length) out += `else{${this.block(s.if.else, at)}}`;
      } else if ("for" in s) out += this.loop(s.for, at);
      else if ("match" in s) {
        const v = `v${at.locals.length}`;
        out += `{const ${v}=${this.x(s.match.value, at)};`;
        s.match.cases.forEach((c, k) => {
          const inner = { ...at };
          out += `${k ? "else " : ""}if(${this.patterns(v, c.when, inner, false)}){${this.block(c.then, inner)}}`;
        });
        const other = this.block(s.match.else ?? [], at);
        out += s.match.cases.length ? (other ? `else{${other}}` : "") : other;
        out += "}";
      } else if ("invoke" in s) out += `m.invoke(o,${str(s.invoke.event)},[${s.invoke.args.map((a) => this.x(a, at)).join(",")}]);`;
      else if ("call" in s) out += `${this.call(s.call, at)};`;
      else out += `m.apply(${this.x(s.apply.fn, at)},${this.args(s.apply.args, at)});`;
    }
    return out;
  }

  private loop(f: ir.StmtFor, at: At): string {
    const d = at.locals.length;
    const locals: Local[] = [...at.locals, ...(f.position !== undefined ? [{ name: f.position }] : []), { name: f.name, ref: f.mut }];
    const over = f.mut ? `${this.k("refs")}(m,${this.placeOf(f.over, at)})` : `${this.k("walked")}(m,${this.x(f.over, at)})`;
    const i = `i${d}`;
    const pos = f.position !== undefined ? `b[${d}]=${this.k("int")}(${i});` : "";
    return `{const l${d}=${over};for(let ${i}=0;${i}<l${d}.length;${i}++){m.step();${pos}b[${locals.length - 1}]=l${d}[${i}];${this.block(f.body, { ...at, locals })}}}`;
  }

  /** A condition, with what it binds added to the locals. */
  private when(c: ir.Cond, at: At): string {
    if ("cond" in c) return this.test(c.cond, at);
    const d = at.locals.length;
    const o = c.open;
    // the open's type only when it can refuse a value: its contract makes the opening a proof
    const t = this.narrowed(o.type) ? `,${this.type(o.type)}` : "";
    const code = o.mut ? `${this.k("openMut")}(m,b,${d},${this.placeOf(o.opt, at)}${t})` : `${this.k("open")}(m,b,${d},${this.x(o.opt, at)}${t})`;
    at.locals = [...at.locals, { name: o.name, ref: o.mut }];
    return code;
  }

  private patterns(v: string, pats: readonly ir.Pattern[], at: At, view: boolean): string {
    const d = at.locals.length;
    let names: string[] = [];
    const code = pats.map((p) => {
      if ("case" in p) {
        const binds = p.case.binds ?? [];
        if (binds.length) names = binds;
        const is = view ? `${this.k("isCaseNamed")}(${v},${str(p.case.name)})` : `${this.k("isCase")}(${v},${str(p.case.enum)},${str(p.case.name)})`;
        return binds.length ? `${this.k("binds")}(b,${d},${binds.length},${is})` : `${is}!==undefined`;
      }
      if ("num" in p) return `${this.k("isNum")}(${v},${str(p.num)})`;
      if ("text" in p) return `${this.k("isText")}(${v},${str(p.text)})`;
      return `${this.k("isBool")}(${v},${p.bool})`;
    });
    at.locals = [...at.locals, ...names.map((name) => ({ name }))];
    return code.join("||") || "false";
  }

  // ---- expressions

  private local(name: string, at: At): number {
    for (let k = at.locals.length - 1; k >= 0; k--) if (at.locals[k].name === name) return k;
    throw new Error(`no local ${name}`);
  }

  private around(at: At): string {
    return at.locals.length ? `b.slice(0,${at.locals.length})` : "[]";
  }

  private num(s: string): string {
    return this.constant(`${this.k("num")}(${str(s)})`);
  }

  private x(e: ir.Expr, at: At): string {
    if ("num" in e) return this.num(e.num);
    if ("text" in e) return this.constant(`${this.k("text")}(${str(e.text)})`);
    if ("bool" in e) return this.constant(`${this.k("bool")}(${e.bool})`);
    if ("duration" in e) return this.constant(`${this.k("dur")}(${str(e.duration)})`);
    if ("none" in e) return this.constant(`${this.k("none")}()`);
    if ("token" in e) {
      const dot = e.token.lastIndexOf(".");
      return this.constant(`${this.k("kase")}(${str(e.token.slice(0, dot))},${str(e.token.slice(dot + 1))})`);
    }
    if ("list" in e) return `m.makeList([${e.list.map((x) => this.x(x, at)).join(",")}])`;
    if ("record" in e) {
      this.types.add(e.record.type);
      return `m.makeRecord(${str(e.record.type)},${this.entries(e.record.fields, at)},false)`;
    }
    if ("case" in e) {
      const of = sorted(e.case.values);
      if (!of.length) return this.constant(`${this.k("kase")}(${str(e.case.enum)},${str(e.case.name)})`);
      const decl = (this.enums.get(e.case.enum) ?? this.tokens.get(e.case.enum))?.cases.find((c) => c.name === e.case.name)?.fields?.map((f) => f.name) ?? [];
      const by = decl.length ? decl.map((f) => of.findIndex(([k]) => k === f)).filter((k) => k >= 0) : of.map((_, k) => k);
      const values = this.entries(e.case.values, at);
      const inOrder = by.length === of.length && by.every((k, i) => k === i);
      return `${this.k("kase")}(${str(e.case.enum)},${str(e.case.name)},${inOrder ? values : `${this.k("order")}(${values},${str(by)})`})`;
    }
    if ("some" in e) return `${this.k("some")}(${this.x(e.some, at)})`;
    if ("range" in e) return `${this.k("range")}(${[e.range.low, e.range.high, e.range.step].map((x) => (x ? this.x(x, at) : "null")).join(",")})`;
    if ("tokval" in e) {
      const field = this.tokens.get(e.tokval.type)?.cases.find((c) => c.name === e.tokval.name)?.fields?.[0]?.name ?? "value";
      return `${this.k("kase")}(${str(e.tokval.type)},${str(e.tokval.name)},[[${str(field)},${this.x(e.tokval.value, at)}]])`;
    }
    if ("rich" in e) {
      const n = this.content(e.rich.nodes, at);
      return `m.content(${n},${this.around(at)},o,${e.rich.blocks === true})`;
    }
    if ("sentence" in e) return `${this.k("sentence")}(${this.x(e.sentence, at)})`;
    if ("template" in e) return this.template(e.template, at);
    if ("view" in e) {
      const v = e.view;
      const of = this.templates.has(v.widget) ? this.tpl(v.widget) : this.widget(v.widget);
      const n = this.constantLater(`{of:${of},${this.slots(v.slots, at)}${this.events(v.events, at)}}`, "v");
      return `m.view(${n},${this.entries(v.args, at)},${this.around(at)},o)`;
    }
    if ("get" in e) return this.get(e.get, at);
    if ("field" in e) {
      // a token's record lets its fields through
      for (const t of this.tokens.values()) if (this.records.get(t.value.name)?.fields.some((f) => f.name === e.field.name)) this.types.add(t.name);
      return `m.field(${this.x(e.field.value, at)},${str(e.field.name)})`;
    }
    if ("call" in e) return this.call(e.call, at);
    if ("apply" in e) return `m.apply(${this.x(e.apply.fn, at)},${this.args(e.apply.args, at)})`;
    if ("op" in e) return this.op(e.op, at);
    if ("show" in e) return `${this.k("text")}(m.show(${this.x(e.show.value, at)}))`;
    if ("fn" in e) {
      const f = this.constantLater(this.fnOf(e.fn.params ?? [], e.fn.body, at), "g");
      return `m.closure(${f},${this.around(at)},o)`;
    }
    if ("place" in e) return `m.readPlace(${this.place(e.place, at)})`;
    if ("try" in e) return this.tried(e.try.value, at);
    return `m.changed(${this.x(e.with.value, at)},${this.entries(e.with.fields, at)},false)`;
  }

  private entries(of: { [key: string]: ir.Expr } | undefined, at: At): string {
    return `[${sorted(of).map(([k, x]) => `[${str(k)},${this.x(x, at)}]`).join(",")}]`;
  }

  private template(t: ir.ExprTemplate, at: At): string {
    if (t.type.name !== "core/text") return `${this.k("template")}(m,${str(t.pieces)},[${t.holes.map((h) => this.x(h, at)).join(",")}])`;
    const out: string[] = [];
    t.pieces.forEach((p, k) => {
      if (p || k === 0) out.push(str(p));
      if (t.holes[k]) out.push("show" in t.holes[k] ? `m.show(${this.k("text")}(m.show(${this.x((t.holes[k] as { show: ir.ExprShow }).show.value, at)})))` : `m.show(${this.x(t.holes[k], at)})`);
    });
    return `${this.k("text")}(${out.map((x) => x.replace(/^m\.show\(text\$\((m\.show\(.*\))\)\)$/s, "$1")).join("+")})`;
  }

  private get(g: ir.Get, at: At): string {
    if ("local" in g) {
      const k = this.local(g.local, at);
      return at.locals[k].ref ? `m.valueOf(b[${k}])` : `b[${k}]`;
    }
    if ("input" in g) return `m.in(o,${str(g.input)})`;
    if ("cell" in g) return `m.cell(o,${str(g.cell)})`;
    if ("$state" in g) return `m.st(${str(g.$state)})`;
    if ("token" in g) return this.x({ token: g.token }, at);
    if ("slot" in g) {
      const kind = at.t?.slots?.find((s) => s.name === g.slot)?.type;
      if (kind && !slotKind(kind).endsWith("view")) this.calls = true;
      return `m.slot(o,${str(g.slot)})`;
    }
    if ("extern" in g) return `m.x(${str(g.extern)})`;
    const key = `let ${g.let}`;
    let n = this.named.get(key);
    if (!n) {
      this.named.set(key, (n = this.name("l")));
      this.shells.push(`const ${n}=(m,b=[],o)=>${this.x(this.lets.get(g.let)!.value, { locals: [] })};`);
    }
    return `${n}(m)`;
  }

  private args(args: readonly ir.Expr[], at: At): string {
    return `[${args.map((a) => ("place" in a ? `{ref:${this.place(a.place, at)}}` : this.x(a, at))).join(",")}]`;
  }

  private call(c: ir.Call, at: At): string {
    const args = this.args(c.args ?? [], at);
    if (this.fns.has(c.name)) return `m.fn(${this.fn(c.name)},${args},o)`;
    if (this.natives.has(c.name)) return `m.nat(${this.native(c.name)},${args})`;
    if (this.externs.has(c.name)) {
      this.extern(c.name);
      return `m.ext(${str(c.name)},${args}.map(a=>m.valueOf(a)))`;
    }
    const dot = c.name.lastIndexOf(".");
    return `m.iface(${str(c.name.slice(0, dot))},${str(c.name.slice(dot + 1))},${args})`;
  }

  /** A bool's own value. */
  private test(e: ir.Expr, at: At): string {
    if ("bool" in e) return String(e.bool);
    if ("op" in e) {
      const b = this.raw(e.op, at);
      if (b) return b;
    }
    return `m.bool(${this.x(e, at)})`;
  }

  /** An operator that gives a bool, as JavaScript's. */
  private raw(op: ir.ExprOp, at: At): string | undefined {
    const a = op.args;
    const two = () => [this.x(a[0], at), this.x(a[1], at)] as const;
    switch (op.name) {
      case "&&":
      case "||":
        return `(${this.test(a[0], at)}${op.name}${this.test(a[1], at)})`;
      case "!":
        return `!${this.test(a[0], at)}`;
      case "==":
        return `m.equal(${two().join(",")})`;
      case "!=":
        return `!m.equal(${two().join(",")})`;
      case "<":
        return `m.before(${two().join(",")})`;
      case ">":
        return `m.after(${two().join(",")})`;
      case "<=":
        return `!m.after(${two().join(",")})`;
      case ">=":
        return `!m.before(${two().join(",")})`;
    }
    return undefined;
  }

  private op(op: ir.ExprOp, at: At): string {
    if (["<", ">", "<=", ">="].includes(op.name)) this.ordered();
    const raw = this.raw(op, at);
    if (raw) return `${this.k("bool")}(${raw})`;
    if (op.name === "?") return `(${this.test(op.args[0], at)}?${this.x(op.args[1], at)}:${this.x(op.args[2], at)})`;
    if (op.name === "-" && op.args.length === 1) return `${this.k("neg")}(${this.x(op.args[0], at)})`;
    return `m.arith(${str(op.name)},${this.x(op.args[0], at)},${this.x(op.args[1], at)})`;
  }

  /** Enums are ordered as they are written: any may be compared. */
  private ordered(): void {
    for (const e of this.enums.values()) if (e.ordered && e.cases.length > 1) this.types.add(e.name);
  }

  private tried(e: ir.Expr, at: At): string {
    if ("record" in e) {
      this.types.add(e.record.type);
      return `(g=>m.tried(()=>m.makeRecord(${str(e.record.type)},g,true)))(${this.entries(e.record.fields, at)})`;
    }
    if ("with" in e) return `((v,g)=>m.tried(()=>m.changed(v,g,true)))(${this.x(e.with.value, at)},${this.entries(e.with.fields, at)})`;
    if ("case" in e) {
      const types = (this.enums.get(e.case.enum) ?? this.tokens.get(e.case.enum))?.cases.find((c) => c.name === e.case.name)?.fields?.map((f) => this.type(f.type)) ?? [];
      return `${this.k("fitting")}(m,${this.x(e, at)},[${types.join(",")}])`;
    }
    if ("tokval" in e) {
      const t = this.tokens.get(e.tokval.type)?.cases.find((c) => c.name === e.tokval.name)?.fields?.[0]?.type;
      return `${this.k("fitting")}(m,${this.x(e, at)},[${t ? this.type(t) : ""}])`;
    }
    return `m.tried(()=>${this.x(e, at)})`;
  }

  // ---- places

  private place(p: ir.Place, at: At): string {
    const r = p.root;
    let root: string;
    if ("local" in r) {
      const k = this.local(r.local, at);
      const t = at.locals[k].type;
      root = `${this.k("localAt")}(b,${k},${str(r.local)}${t ? `,${this.type(t)}` : ""})`;
    } else if ("cell" in r) root = `${this.k("cellAt")}(o,${str(r.cell)})`;
    else if ("input" in r) root = `${this.k("inputAt")}(o,${str(r.input)})`;
    else root = `${this.k("state")}(${str(r.$state)})`;
    const path = (p.path ?? []).map((s) => ("field" in s ? `${this.k("field")}(${str(s.field.name)})` : `${this.k("item")}(${this.x(s.item.at, at)})`));
    return path.length ? `${this.k("at")}(${root},${path.join(",")})` : root;
  }

  private placeOf(e: ir.Expr, at: At): string {
    if ("place" in e) return this.place(e.place, at);
    if ("get" in e) {
      const g = e.get;
      if ("local" in g) return this.place({ root: { local: g.local } }, at);
      if ("cell" in g) return this.place({ root: { cell: g.cell } }, at);
      if ("input" in g) return this.place({ root: { input: g.input } }, at);
      if ("$state" in g) return this.place({ root: { $state: g.$state } }, at);
    }
    if ("field" in e) return `${this.k("at")}(${this.placeOf(e.field.value, at)},${this.k("field")}(${str(e.field.name)}))`;
    throw new Error("a mut place that isn't a variable");
  }

  // ---- views

  /** Parts as a list of their own, the same one each time. */
  private content(nodes: readonly ir.Node[], at: At): string {
    const n = this.name("p");
    // named before it is written: a fn in it may hold it
    this.shells.push(`const ${n}=[];`);
    this.later.push(`${n}.push(${this.partsOf(nodes, at).join(",")});`);
    return n;
  }

  private partsOf(nodes: readonly ir.Node[], at0: At): string[] {
    const at = { ...at0 };
    return nodes.map((n) => {
      const code = this.part(n, at);
      if ("declare" in n) at.locals = [...at.locals, { name: n.declare.let.name }];
      return code;
    });
  }

  private parts(nodes: readonly ir.Node[], at: At): string {
    return `[${this.partsOf(nodes, at).join(",")}]`;
  }

  private slots(of: { [key: string]: ir.Node[] } | undefined, at: At): string {
    return opt("sl", Object.entries(of ?? {}).map(([k, n]) => `${str(k)}:${this.parts(n, at)}`));
  }

  private events(of: { [key: string]: string } | undefined, at: At): string {
    return opt("ev", Object.entries(of ?? {}).map(([k, h]) => `${str(k)}:${this.handler(h, at)}`));
  }

  private part(n: ir.Node, at: At): string {
    if ("declare" in n) return `{d:${this.k("local")},s:${str(`let ${n.declare.let.name}`)},x:${this.fnAt(n.declare.let.value, at)}}`;
    if ("extern" in n) {
      const x = n.extern;
      const p = Object.entries(x.props ?? {}).map(([k, e]) => `[${str(k)},${this.fnAt(e, at)}]`);
      const decl = this.widgets.get(x.name);
      const wr = Object.entries(x.writes ?? {}).map(([k, place]) => `[${str(k)},(m,b,o)=>${this.place(place, at)},${this.contract(this.placeType(place, at) ?? decl?.inputs?.find((f) => f.name === k)?.type)}]`);
      return `{d:${this.k("extern")},s:${str(x.site)},w:${this.widget(x.name)},${opt("p", p, "[", "]")}${opt("wr", wr, "[", "]")}${this.slots(x.slots, at)}${this.events(x.events, at)}}`;
    }
    if ("use" in n) {
      const u = n.use;
      const a = Object.entries(u.args ?? {}).map(([k, e]) => ("place" in e ? `[${str(k)},(m,b,o)=>${this.place(e.place, at)},true]` : `[${str(k)},${this.fnAt(e, at)}]`));
      if (!this.templates.has(u.name)) throw new Error(`no widget ${u.name}`);
      return `{d:${this.k("use")},s:${str(u.site)},t:${this.tpl(u.name)},${opt("a", a, "[", "]")}${this.slots(u.slots, at)}${this.events(u.events, at)}}`;
    }
    if ("for" in n) {
      const f = n.for;
      const locals: Local[] = [...at.locals, ...(f.position !== undefined ? [{ name: f.position }] : []), { name: f.name, ref: f.mut }];
      const pos = f.position !== undefined;
      const r = f.mut ? `${this.k("mutRows")}(m,${this.placeOf(f.over, at)},${pos})` : `${this.k("rows")}(m,${this.x(f.over, at)},${pos})`;
      return `{d:${this.k("loop")},s:${str(f.site)},r:(m,b,o)=>${r},v:${this.parts(f.body, { ...at, locals })}}`;
    }
    if ("if" in n) {
      const d = at.locals.length;
      const lists: string[] = [];
      const list = (nodes: readonly ir.Node[], inner: At) => {
        lists.push(this.parts(nodes, inner));
        return `this.v[${lists.length - 1}]`;
      };
      let f = "";
      for (const br of n.if.branches) {
        const inner = { ...at };
        const when = this.when(br.when, inner);
        f += `if(${when})return[${list(br.then, inner)},b.slice(${d},${inner.locals.length})];`;
      }
      f += n.if.else?.length ? `return[${list(n.if.else, at)},[]];` : "return[[],[]];";
      return `{d:${this.k("branch")},s:${str(n.if.site)},f(m,b,o){${f.includes("b[") || f.includes("(b,") || f.includes("(m,b,") ? `b=b.slice(0,${d});` : ""}${f}},v:[${lists.join(",")}]}`;
    }
    if ("match" in n) {
      const d = at.locals.length;
      const lists: string[] = [];
      const list = (nodes: readonly ir.Node[], inner: At) => {
        lists.push(this.parts(nodes, inner));
        return `this.v[${lists.length - 1}]`;
      };
      let f = `const v=${this.x(n.match.value, at)};`;
      for (const c of n.match.cases) {
        const inner = { ...at };
        const when = this.patterns("v", c.when, inner, true);
        f += `if(${when})return[${list(c.then, inner)},b.slice(${d},${inner.locals.length})];`;
      }
      f += n.match.else?.length ? `return[${list(n.match.else, at)},[]];` : "return[[],[]];";
      return `{d:${this.k("branch")},s:${str(n.match.site)},f(m,b,o){b=b.slice(0,${d});${f}},v:[${lists.join(",")}]}`;
    }
    return `{d:${this.k("placed")},s:${str(n.place.site)},x:${this.fnAt(n.place.view, at)}${n.place.sentence ? ",sen:true" : ""}}`;
  }

  /** An expression in a view, as a fn. */
  private fnAt(e: ir.Expr, at: At): string {
    const code = this.x(e, at);
    // one that reads nothing is written once
    const f = fnCode(code);
    return f.startsWith("(m,b,o)") ? f : this.constantLater(f, "x");
  }

  private tpl(name: string): string {
    const key = `template ${name}`;
    let n = this.named.get(key);
    if (n) return n;
    this.named.set(key, (n = this.name("t")));
    const t = this.templates.get(name)!;
    const at: At = { locals: [], t };
    this.shells.unshift(`const ${n}={n:${str(name)}};`);
    const i = (t.inputs ?? []).map((f) => `[${str(f.name)},${f.default ? `(m,b,o)=>${this.x(f.default, at)}` : "undefined"}]`);
    const c = (t.cells ?? []).map((f) => `[${str(f.name)},${f.default ? `(m,b,o)=>${this.x(f.default, at)}` : "undefined"},${this.type(f.type)}]`);
    const q = (t.requires ?? []).map((r) => this.rule(r, { t }));
    const sl = (t.slots ?? []).map((s) => `${str(s.name)}:${str(slotKind(s.type))}`);
    this.later.push(`Object.assign(${n},{${opt("i", i, "[", "]")}${opt("c", c, "[", "]")}${opt("q", q, "[", "]")}${opt("sl", sl)}v:${this.parts(t.view, at)}});`);
    return n;
  }

  private widget(name: string): string {
    const key = `widget ${name}`;
    let n = this.named.get(key);
    if (n) return n;
    this.named.set(key, (n = this.name("w")));
    const w = this.widgets.get(name) ?? ({ name, supplier: "core" } as ir.ExternWidget);
    const supplied = this.choice.supplies(name);
    if (supplied) this.drawnBy.add(name);
    if (name === "core/plane") this.planes = true;
    const at: At = { locals: [], t: { inputs: w.inputs, slots: w.slots } as ir.Template };
    this.shells.unshift(`const ${n}={n:${str(name)}};`);
    const i = (w.inputs ?? []).flatMap((f) => (f.default ? [`[${str(f.name)},${this.thunk(f.default)}]`] : []));
    const mu = (w.inputs ?? []).flatMap((f) => (f.mut ? [`[${str(f.name)},${this.type(f.type)},${this.contract(f.type)}]`] : []));
    const ev = (w.events ?? []).flatMap((e) => (e.params?.length ? [`${str(e.name)}:[${e.params.map((p) => this.type(p.type)).join(",")}]`] : []));
    const q = (w.requires ?? []).map((r) => this.rule(r, { t: at.t }));
    const st = (w.inputs ?? []).filter((f) => w.piece && SENTENCES[w.piece]?.includes(f.name)).map((f) => str(f.name));
    const md = supplied ? undefined : this.parts(w.markdown ?? [], at);
    const sl = md || q.length ? (w.slots ?? []).map((s) => `${str(s.name)}:${str(slotKind(s.type))}`) : [];
    this.later.push(`Object.assign(${n},{${opt("i", i, "[", "]")}${opt("mu", mu, "[", "]")}${w.generics?.length && mu.length ? "g:true," : ""}${opt("q", q, "[", "]")}${opt("ev", ev)}${opt("st", st, "[", "]")}${opt("sl", sl)}${md ? `md:${md}` : ""}});`);
    return n;
  }
}

/** An expression's code as a fn of what it reads. */
function fnCode(code: string): string {
  return /\b[bo]\b/.test(code.replace(/"(?:[^"\\]|\\.)*"/g, "")) ? `(m,b,o)=>${code}` : `m=>${code}`;
}

function sorted<T>(o: { [key: string]: T } | undefined): [string, T][] {
  return Object.entries(o ?? {}).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
}

function slotKind(t: ir.Type): string {
  const of = t.name === "core/opt" ? t.args?.[0] : t;
  const kind = of?.name === "core/list" ? "list" : !of || of.name === "core/view" || of.name === "core/rich_text" ? "view" : "one";
  return t.name === "core/opt" ? `opt ${kind}` : kind;
}

/** A constant as a widget is given it, when nothing has to be worked out. */
function literal(e: ir.Expr): Input | undefined {
  if ("num" in e) return { num: Num.literal(e.num)!.show() };
  if ("text" in e || "bool" in e || "none" in e || "token" in e) return e as Input;
  if ("duration" in e) return { duration: Num.literal(e.duration)!.show() } as Input;
  if ("case" in e && !e.case.values) return e as Input;
  if ("some" in e) {
    const x = literal(e.some);
    return x && { some: x };
  }
  return undefined;
}
