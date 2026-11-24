// Which of a document's own parts never change: nothing in them is read, written, or done anything with, through
// every widget they call. Those are HTML alone.

import type { ir } from "@extended-mark/runtime";

export function moving(m: ir.Module): boolean[] {
  const templates = new Map(m.templates.map((t) => [t.name, t]));
  const handlers = new Map((m.handlers ?? []).map((h) => [h.id, h]));
  const widgets = new Map<string, ir.ExternWidget>();
  const outside = new Set<string>();
  const fns = new Map<string, ir.FnDecl>();
  for (const d of m.declarations) {
    const x = d.declare;
    if ("fn" in x) fns.set(x.fn.name, x.fn);
    else if ("extern" in x) {
      if ("widget" in x.extern) widgets.set(x.extern.widget.name, x.extern.widget);
      // one nobody can have worked out ahead: the host's, or one of where the page is read
      else if ("fn" in x.extern && (x.extern.fn.supplier !== "core" || x.extern.fn.name === "core/moment.local")) outside.add(x.extern.fn.name);
      else if ("channel" in x.extern) for (const f of x.extern.channel.fns ?? []) outside.add(`${x.extern.channel.name}.${f.name}`);
    }
  }
  for (const t of m.templates) for (const f of t.fns ?? []) fns.set(f.name, f);
  const known = new Map<string, boolean>();

  /** Code anywhere in which something that changes is named, or a widget that does is called. */
  const code = (x: unknown): boolean => {
    if (Array.isArray(x)) return x.some(code);
    if (typeof x !== "object" || x === null) return false;
    const o = x as Record<string, any>;
    if ("get" in o && typeof o.get === "object") return "$state" in o.get || "cell" in o.get || "extern" in o.get;
    if ("root" in o && typeof o.root === "object") return !("local" in o.root) || code(o.path ?? []);
    if ("call" in o && typeof o.call === "object" && typeof o.call.name === "string") return called(o.call.name) || code(o.call.args ?? []);
    if ("extern" in o && typeof o.extern === "object" && typeof o.extern.site === "string") return extern(o.extern);
    if ("use" in o && typeof o.use === "object" && typeof o.use.site === "string") return use(o.use.name) || happens(o.use.events) || code([o.use.args ?? {}, o.use.slots ?? {}]);
    if ("view" in o && typeof o.view === "object" && typeof o.view.widget === "string") return (templates.has(o.view.widget) ? use(o.view.widget) : drawn(o.view.widget)) || happens(o.view.events) || code([o.view.args ?? {}, o.view.slots ?? {}]);
    return Object.entries(o).some(([k, v]) => k !== "reads" && k !== "type" && code(v));
  };
  const happens = (events: { [k: string]: string } | undefined): boolean => Object.values(events ?? {}).some((id) => {
    const h = handlers.get(id);
    return !(h && h.body.length === 1 && "invoke" in h.body[0]);
  });
  const once = (key: string, work: () => boolean): boolean => {
    const k = known.get(key);
    if (k !== undefined) return k;
    // one that calls itself moves if something else in it does
    known.set(key, false);
    const is = work();
    known.set(key, is);
    return is;
  };
  const called = (name: string): boolean => {
    if (outside.has(name)) return true;
    const own = fns.get(name);
    if (own) return once(`fn ${name}`, () => (own.writes?.length ?? 0) > 0 || (own.reads ?? []).some((r) => !("local" in r)) || code(own.body));
    // an interface's: any fn it may be sent to
    const ends = name.slice(name.lastIndexOf("."));
    return [...fns.keys()].some((f) => f.endsWith(ends) && once(`fn ${f}`, () => (fns.get(f)!.reads ?? []).some((r) => !("local" in r)) || code(fns.get(f)!.body)));
  };
  const drawn = (name: string): boolean => {
    const w = widgets.get(name);
    return !w || (w.inputs ?? []).some((f) => f.mut) || (w.requires ?? []).length > 0 || code(w.markdown ?? []) || (w.inputs ?? []).some((f) => f.default && code(f.default));
  };
  const extern = (x: ir.NodeExtern): boolean => drawn(x.name) || Object.keys(x.writes ?? {}).length > 0 || happens(x.events) || code([x.props ?? {}, x.slots ?? {}]);
  const use = (name: string): boolean =>
    once(`use ${name}`, () => {
      const t = templates.get(name);
      return !t || (t.cells ?? []).length > 0 || (t.requires ?? []).length > 0 || code([t.view, (t.inputs ?? []).map((f) => f.default ?? {})]);
    });

  const out: boolean[] = [];
  let after = false;
  for (const n of templates.get("doc")?.view ?? []) {
    const is = after || code(n);
    // what comes after a local that may fail isn't drawn when it does
    if ("declare" in n && is) after = true;
    out.push(is);
  }
  return out;
}
