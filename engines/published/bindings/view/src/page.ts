// Making a page from the host's props, with no DOM: shared by the bindings and by html.ts, which
// a server runs.

import { Page, type Outcome, type PageOptions, type Input, type ir } from "@extended-mark/runtime";

/** A page's IR: the file `xtmd build` writes, as `import page from "./x.xtmd"` gives it. */
export type Ir = ir.File | ir.Module;

/** A value the host gives, in the IR's form: `{ "num": "3" }`. */
export type Given = Input;

/** `$state`'s fields in a module, by name. */
export function stateFields(module: ir.Module): Map<string, ir.Type> {
  const out = new Map<string, ir.Type>();
  for (const d of module.declarations) {
    const decl = d.declare as Record<string, unknown>;
    if ("$state" in decl) for (const f of (decl["$state"] as ir.State).fields) out.set(f.name, f.type);
  }
  return out;
}

/** A channel's full name: `app` is the document's own `doc/app`, as `extern app` and `expose app`
 * declare it; a name with a module (`doc/app`, `core/page`) is kept as it is. */
export function channelName(module: ir.Module, name: string): string {
  if (name.includes("/")) return name;
  const full = `${module.module}/${name}`;
  for (const d of module.declarations) {
    const decl = d.declare as Record<string, unknown>;
    if ("expose" in decl && (decl.expose as ir.Expose).channel === full) return full;
    if ("extern" in decl) {
      const x = decl.extern as Record<string, unknown>;
      if ("channel" in x && (x.channel as ir.ExternChannel).name === full) return full;
    }
  }
  return name;
}

/** Each channel's values with its full name. */
export function channelsOf(module: ir.Module, given: Record<string, Record<string, Input>> | undefined): Record<string, Record<string, Input>> {
  const out: Record<string, Record<string, Input>> = {};
  for (const [c, v] of Object.entries(given ?? {})) {
    const full = channelName(module, c);
    out[full] = { ...out[full], ...v };
  }
  return out;
}

const texts = new WeakMap<object, string>();

/** An IR's content: what it is told apart by, whichever object holds it. */
export function irText(irFile: Ir): string {
  let t = texts.get(irFile);
  if (t === undefined) texts.set(irFile, (t = JSON.stringify(moduleOf(irFile))));
  return t;
}

/** Whether two IRs are the same page: a page made from one goes on when it is given the other. */
export function sameIr(a: Ir | undefined, b: Ir | undefined): boolean {
  return a === b || (!!a && !!b && irText(a) === irText(b));
}

export function moduleOf(irFile: Ir): ir.Module {
  return "ir" in irFile ? irFile.ir : irFile;
}

/**
 * A page made from its IR with `$state` given by the host, checked as a host's write is (page 6,
 * "Keeping things"): a field the page doesn't have, or a value that doesn't fit, keeps its
 * starting value, and so does a field whose value breaks a `require` that the others keep. What
 * was refused is in the outcome, with what failed as it was drawn. The page is drawn once, with the values that passed, so every widget
 * starts from them.
 */
export function makePage(irFile: Ir, options: Omit<PageOptions, "state">, given?: Record<string, Input>, report = true): { page: Page; outcome: Outcome } {
  const module = moduleOf(irFile);
  const opts = { ...options, externs: channelsOf(module, options.externs) };
  const fields = stateFields(module);
  // a field the page doesn't have: told, or, when a page takes over from another, dropped without a word
  const state = report ? given : given && Object.fromEntries(Object.entries(given).filter(([k]) => fields.has(k)));
  const page = new Page(irFile, { ...opts, state });
  return { page, outcome: page.takeOutcome() };
}
