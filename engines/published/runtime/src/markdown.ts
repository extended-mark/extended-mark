// A page as Markdown blocks (page 1, Markdown out), with the values it has now: `$state`, the page's values
// and each widget's own variables as they are. A port of compiler/markdown's render.rs, on the live page.
// What writes blocks as text is write.ts.

import type * as ir from "./generated/ir.js";
import { Failure, type Binding } from "./eval/index.js";
import type { Value as V } from "./eval/value.js";
import { type Env, Core, checkSentence, setMarkdownWriter } from "./page.js";
import { writeMarkdown, type Block, type Inline, type Item } from "./write.js";

type Piece = { block: Block } | { item: Item } | { run: Inline };

function field(v: V | undefined, name: string): V | undefined {
  if (!v) return undefined;
  if (v.kind === "record") return v.fields.find(([k]) => k === name)?.[1];
  if (v.kind === "case") return v.values.find(([k]) => k === name)?.[1];
  return undefined;
}

function textOf(v: V | undefined): string {
  if (!v) return "";
  if (v.kind === "text") return v.s;
  if (v.kind === "num") return v.n.show();
  if (v.kind === "opt") return textOf(v.value ?? undefined);
  return "";
}

function itemsOf(v: V | undefined): V[] {
  return v && v.kind === "list" ? v.items.map((i) => i.value) : [];
}

/** A template's text: its pieces with each hole's shown text between them. */
function templateText(v: V | undefined): string {
  const pieces = itemsOf(field(v, "pieces"));
  const holes = itemsOf(field(v, "holes"));
  return pieces.map((p, k) => textOf(p) + (holes[k] ? textOf(field(holes[k], "value")) : "")).join("");
}

/** Runs that follow one another with the same marks are one run. */
function joinRuns(runs: Inline[]): Inline[] {
  const out: Inline[] = [];
  for (const r of runs) {
    const last = out[out.length - 1];
    if (last && "text" in last && "text" in r) {
      const a = last.text, b = r.text;
      if (!!a.bold === !!b.bold && !!a.italic === !!b.italic && !!a.strike === !!b.strike && a.link === b.link) {
        a.value += b.value;
        continue;
      }
    }
    if ("text" in r && r.text.value === "") continue;
    out.push(r);
  }
  return out;
}

/** List items that follow one another make one list, across a loop's turns too (page 3); a
 * sentence among blocks is a paragraph. */
function blocksOf(pieces: Piece[]): Block[] {
  const out: Block[] = [];
  let inSentence = false;
  for (const p of pieces) {
    const last = out[out.length - 1];
    if ("run" in p) {
      if (inSentence && last && "paragraph" in last) last.paragraph.push(p.run);
      else out.push({ paragraph: [p.run] });
      inSentence = true;
      continue;
    }
    inSentence = false;
    const b: Block = "block" in p ? p.block : { bullets: { ordered: false, items: [p.item] } };
    if (last && "bullets" in last && "bullets" in b && last.bullets.ordered === b.bullets.ordered) {
      last.bullets.items.push(...b.bullets.items);
      continue;
    }
    out.push(b);
  }
  return out;
}

/** What some calls write in a sentence: their runs, and the words of any blocks among them. */
function runsOf(pieces: Piece[]): Inline[] {
  const out: Inline[] = [];
  let among: Piece[] = [];
  const flush = () => {
    const bs = blocksOf(among);
    among = [];
    // a widget that is one paragraph writes its sentence
    if (bs.length === 1 && "paragraph" in bs[0]) out.push(...bs[0].paragraph);
    else if (bs.length) out.push({ text: { value: bs.map(plain).filter((s) => s).join(" ") } });
  };
  for (const p of pieces) {
    if ("run" in p) {
      flush();
      out.push(p.run);
    } else among.push(p);
  }
  flush();
  return out;
}

function join(path: string, part: string): string {
  return path ? `${path}/${part}` : part;
}

/** What failed, in its place: a quote (page 6). */
function notice(name: string, f: unknown): Piece {
  const msg = f instanceof Error ? f.message : String(f);
  return { block: { quote: [{ paragraph: [{ text: { value: `${name.split("/").pop()} failed: ${msg}` } }] }] } };
}

class Writer {
  depth = 0;
  private inSentence = 0;
  constructor(private readonly page: Core) {}

  nodes(nodes: ir.Node[], env0: Env): Piece[] {
    let env = env0;
    const out: Piece[] = [];
    for (const n of nodes) {
      const had = out.length;
      try {
        env = this.node(n, env, out);
      } catch (f) {
        // in the document's own code, a notice in its place, as a widget that fails is
        if (env.owner !== undefined || this.inSentence > 0) throw f;
        out.length = had;
        out.push(notice("extern" in n ? n.extern.name : "use" in n ? n.use.name : "doc", f));
        // what comes after may read it
        if ("declare" in n) break;
      }
    }
    return out;
  }

  /** One node's pieces, and the locals after it. */
  private node(n: ir.Node, env: Env, out: Piece[]): Env {
    if ("declare" in n) {
      const l = n.declare.let;
      return { ...env, bindings: [...env.bindings, [l.name, { value: this.page.evalIn(env, l.value) }]] };
    } else if ("for" in n) {
      // rows by position for paths, and by item for identity, as drawing walks them
      this.page.keyedRows(n.for, env).forEach((row, k) =>
        out.push(...this.nodes(n.for.body, { owner: env.owner, bindings: [...env.bindings, ...row.bindings], path: join(env.path, `${n.for.site}[${k}]`), key: join(env.key ?? env.path, `${n.for.site}[${row.key}]`) })),
      );
    } else if ("if" in n) {
      const b = this.page.pickBranch(n.if, env);
      if (b) out.push(...this.nodes(b.nodes, b.env));
    } else if ("match" in n) {
      const v = this.page.evalIn(env, n.match.value);
      let done = false;
      for (const c of n.match.cases) {
        const binds = this.page.matches(v, c.when);
        if (binds) {
          out.push(...this.nodes(c.then, { ...env, bindings: [...env.bindings, ...binds] }));
          done = true;
          break;
        }
      }
      if (!done) out.push(...this.nodes(n.match.else ?? [], env));
    } else if ("place" in n) {
      const v = this.page.evalIn(env, n.place.view);
      if (n.place.sentence) checkSentence(v);
      out.push(...this.view(v, join(env.path, n.place.site), join(env.key ?? env.path, n.place.site)));
    } else if ("extern" in n) {
      const props = new Map<string, V>();
      for (const [k, e] of Object.entries(n.extern.props ?? {})) props.set(k, this.page.evalIn(env, e));
      for (const [k, p] of Object.entries(n.extern.writes ?? {})) props.set(k, this.page.machine.evalWith(env.bindings, env.owner, { place: p } as ir.Expr));
      out.push(...this.widget(n.extern.name, join(env.path, n.extern.site), props, props, n.extern.slots ?? {}, env, join(env.key ?? env.path, n.extern.site)));
    } else if ("use" in n) {
      out.push(...this.widget(n.use.name, join(env.path, n.use.site), new Map(), n.use.args ?? {}, n.use.slots ?? {}, env, join(env.key ?? env.path, n.use.site), n.use.events ?? {}));
    }
    return env;
  }

  view(v: V, path: string, key: string): Piece[] {
    if (v.kind === "opt") return v.value ? this.view(v.value, path, key) : [];
    if (v.kind === "list") return v.items.flatMap((i, k) => this.view(i.value, `${path}[${k}]`, `${key}[${i.id}]`));
    if (v.kind === "content") {
      if ("parts" in v.of) return v.of.parts.flatMap((p, k) => this.view(p, join(path, `j${k}`), join(key, `j${k}`)));
      const c = v.of;
      return this.nodes(c.nodes as ir.Node[], { owner: c.owner, bindings: c.captured as [string, Binding][], path: c.at?.path ?? path, key: c.at?.key ?? key });
    }
    if (v.kind !== "view") return [];
    const who = v.at ?? key;
    const made: Env = { owner: v.owner, bindings: [...v.captured] as [string, Binding][], path, key: who };
    return this.widget(v.widget, path, new Map(v.args), v.args as Map<string, V>, v.slots as Record<string, ir.Node[]>, made, who, v.events as Record<string, string>);
  }

  /** A sentence's calls as runs. */
  sentence(v: V | undefined, path: string, key: string): Inline[] {
    if (!v) return [];
    checkSentence(v);
    this.inSentence++;
    try {
      return joinRuns(runsOf(this.view(v, path, key)));
    } finally {
      this.inSentence--;
    }
  }

  slot(slots: Record<string, ir.Node[]>, name: string, env: Env): Piece[] {
    return slots[name] ? this.nodes(slots[name], env) : [];
  }

  /** A widget: one that is a piece of Markdown is written as it; any other writes its markdown. */
  /** `key` and `events` are the call's, as drawing gives them: entering a widget to write its
   * Markdown must find the instance the page drew and leave it as it was. */
  widget(name: string, path: string, props: Map<string, V>, args: Record<string, ir.Expr> | Map<string, V>, slots: Record<string, ir.Node[]>, env: Env, key: string = path, events: Record<string, string> = {}): Piece[] {
    if (++this.depth > 64) {
      this.depth--;
      throw new Failure("Limit", `${name} places itself more than 64 deep`);
    }
    try {
      return this.widgetInner(name, path, props, args, slots, env, key, events);
    } catch (f) {
      if (this.page.externWidget(name)?.piece) throw f;
      // a widget that fails is a notice in its place, a quote in Markdown (page 6)
      return [notice(name, f)];
    } finally {
      this.depth--;
    }
  }

  private widgetInner(name: string, path: string, props: Map<string, V>, args: Record<string, ir.Expr> | Map<string, V>, slots: Record<string, ir.Node[]>, env: Env, key: string, events: Record<string, string>): Piece[] {
    const p = (k: string) => props.get(k);
    switch (this.page.externWidget(name)?.piece) {
      case "paragraph": {
        const runs = this.sentence(p("value"), join(path, "value"), join(key, "value"));
        return runs.length ? [{ block: { paragraph: runs } }] : [];
      }
      case "heading": {
        const l = p("level");
        const level = l && l.kind === "num" ? Math.min(6, Math.max(1, Number(l.n.show()))) : 1;
        return [{ block: { heading: { level, inline: this.sentence(p("value"), join(path, "value"), join(key, "value")) } } }];
      }
      case "bullets": {
        const o = p("ordered");
        const ordered = !!o && o.kind === "bool" && o.b;
        const s = p("start");
        const start = s && s.kind === "num" ? Number(s.n.show()) : 1;
        const items: Item[] = this.slot(slots, "items", env).map((x) => ("item" in x ? x.item : { blocks: blocksOf([x]) }));
        if (!items.length) return [];
        return [{ block: { bullets: ordered ? { ordered, start, items } : { ordered, items } } }];
      }
      case "item": {
        const d = p("done");
        const done = d && d.kind === "opt" && d.value && d.value.kind === "bool" ? d.value.b : d && d.kind === "bool" ? d.b : undefined;
        const item: Item = { blocks: blocksOf(this.slot(slots, "body", env)) };
        if (done !== undefined) item.done = done;
        return [{ item }];
      }
      case "quote":
        return [{ block: { quote: blocksOf(this.slot(slots, "body", env)) } }];
      case "code":
        return [{ block: { code: { lang: textOf(p("lang")), src: textOf(p("src")) } } }];
      case "math":
        return [{ block: { math: templateText(field(p("tex"), "tex")) } }];
      case "divider":
        return [{ block: { divider: {} } }];
      case "text":
        return [{ run: { text: { value: textOf(p("value")) } } }];
      case "bold":
      case "italic":
      case "strike":
      case "link": {
        const mark = this.page.externWidget(name)!.piece as "bold" | "italic" | "strike" | "link";
        const to = textOf(p("to"));
        return runsOf(this.slot(slots, "body", env)).map((run): Piece => ("text" in run ? { run: { text: { ...run.text, ...(mark === "link" ? { link: to } : { [mark]: true }) } } } : { run }));
      }
      case "code_span":
        return [{ run: { code: textOf(p("src")) } }];
      case "inline_math":
        return [{ run: { math: templateText(field(p("tex"), "tex")) } }];
      case "inline_image":
        return [{ run: { image: { src: textOf(p("src")), alt: textOf(p("alt")) } } }];
      case "note_mark":
        return [{ run: { note: textOf(p("mark")) } }];
      case "line_break":
        return [{ run: { break: {} } }];
      case "footnote":
        return [{ block: { footnote: { mark: textOf(p("mark")), blocks: blocksOf(this.slot(slots, "body", env)) } } }];
      case "table": {
        const cell = (at: string) => (c: V, k: number) => this.sentence(c, `${join(path, at)}[${k}]`, `${join(key, at)}[${k}]`);
        const head = itemsOf(p("head")).map(cell("head"));
        const rows = itemsOf(p("rows")).map((r, k) => itemsOf(r).map(cell(`rows[${k}]`)));
        const align = itemsOf(p("align")).map((a) => (a.kind === "case" && (a.name === "CENTER" || a.name === "END") ? a.name : "START")) as ("START" | "CENTER" | "END")[];
        return [{ block: { table: { head, rows, align } } }];
      }
    }
    const t = this.page.templateOf(name);
    if (t) {
      // a widget written in Extended Mark: its markdown, in its instance
      const own = this.page.enterUse(t, path, args, slots, events, env, key);
      return this.nodes(t.markdown ?? t.view, own);
    }
    const w = this.page.externWidget(name);
    if (w) {
      const own = this.page.enterExtern(w, path, props, slots, env);
      try {
        return this.nodes(w.markdown ?? [], own);
      } finally {
        this.page.leave(own);
      }
    }
    throw new Failure("Ir", `there is no widget ${name} to write`);
  }
}

function plain(b: Block): string {
  const runs = (r: Inline[]) => r.map((i) => ("text" in i ? i.text.value : "code" in i ? i.code : "math" in i ? i.math : "image" in i ? i.image.alt : "")).join("");
  if ("paragraph" in b) return runs(b.paragraph);
  if ("heading" in b) return runs(b.heading.inline);
  if ("quote" in b) return b.quote.map(plain).join(" ");
  return "";
}

/** The page's blocks, with the values it has now. */
export function pageBlocks(page: Core): Block[] {
  const doc = page.templateOf("doc");
  if (!doc) return [];
  return blocksOf(new Writer(page).nodes(doc.markdown ?? doc.view, { owner: undefined, bindings: [], path: "" }));
}

// ---- the one spelling (page 1, Markdown out)

setMarkdownWriter((page) => writeMarkdown(pageBlocks(page)));
