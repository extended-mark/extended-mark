// A built page in a document: the HTML it was written as is taken over, and followed from then on. What never
// changes is HTML alone, and is left as it is.

import { Patcher, Tokens, children, domHost, domReads, focusOf, type Child, type Drawing, type ViewWidget } from "@extended-mark/widgets";
import type { Input, Node as Drawn, Page } from "@extended-mark/runtime";
import { Kit, type Given, type Program } from "./kit.js";
import { Nodes } from "./nodes.js";

/** What the page is made of, as its engine wrote it. */
export interface Built {
  program: Program;
  widgets: Record<string, ViewWidget>;
  /** What each token stands for, and the class its stylesheet is under. */
  tokens: { types: { name: string; values: Record<string, Input> }[]; sheet: string };
  /** The children of the page's element as it was written, in runs: `true`, one that stays as it is for good; a
   * number, how many the program's next part drew. */
  html: (true | number)[];
  /** Value fns the engine gives. */
  fns?: () => NonNullable<Given["fns"]>;
  /** What the page's planes are laid out by. */
  layout?: (page: Page, element: (id: string) => HTMLElement | undefined) => void;
  /** A page that reads a slot as calls. */
  calls?: (m: Kit) => void;
}

/** One change from outside (schemas/actions.schema.json). */
export type Action =
  | { event: { id: string; name: string; args?: Input[] } }
  | { write: { id: string; input: string; value: Input } }
  | { set: { fields: Record<string, Input> } }
  | { expose: { channel: string; name: string; args?: Input[] } }
  | { extern: { channel: string; values: Record<string, Input> } }
  | { page: { values: Record<string, Input> } }
  | { restore: { data: { state?: Record<string, Input>; own?: Record<string, Record<string, Input>> } } };

export interface MountedPage {
  /** One change from outside. */
  act(a: Action): void;
  /** `$state` and widgets' own variables (schemas/state.schema.json): what a host keeps, and gives back. */
  data(): { state: Record<string, Input>; own: Record<string, Record<string, Input>> };
  /** What was refused, failed or called since it was last taken (schemas/told.schema.json). */
  told(): ReturnType<Kit["told"]>;
  /** What of the data it started from had no place. */
  readonly left: string[] | undefined;
  destroy(): void;
}

export interface MountOptions extends Given {
  /** Called after every change. */
  onChange?: (page: MountedPage) => void;
}

/** What a value of a token is, and a page's theme: as the widgets ask. */
function tokensOf(m: Kit, built: Built): Tokens {
  const all = Object.fromEntries(built.tokens.types.flatMap((t) => Object.entries(t.values).map(([n, v]) => [`${t.name}.${n}`, v])));
  return new Tokens({
    ir: { declarations: built.tokens.types.map((t) => ({ declare: { token: t } })) } as any,
    given: (): Record<string, Input> => {
      const v = m.externNow("core/page.theme");
      return v ? { "core/page.theme": m.plain(v) } : {};
    },
    token: (of) => ("token" in of ? all[of.token] : "tokval" in of ? of.tokval.value : undefined),
  });
}

/** What draws a page's nodes, on a server as in a document. */
export function drawingOf(m: Kit, nodes: Nodes, built: Built, uid: string, more: Partial<Drawing> = {}): Drawing {
  return {
    registry: built.widgets,
    tokens: tokensOf(m, built),
    uid,
    kept: new WeakMap<Drawn, Child>(),
    node: (id) => nodes.of(id),
    event: (id, name, args) => m.event(id, name, args),
    write: (id, input, value) => m.write(id, input, value),
    call: (id, input, args) => nodes.call(id, input, args),
    layout: () => {},
    fail: (f) => void m.errors.push(f),
    ...more,
  };
}

export function kitOf(built: Built, given: Given): Kit {
  const g = { ...given, fns: { ...built.fns?.(), ...given.fns } };
  if (!built.calls) return new Kit(built.program, g);
  const { calls } = built;
  return new (class extends Kit {
    protected override ready(): void {
      calls(this);
    }
  })(built.program, g);
}

let mounts = 0;

export function mount(root: HTMLElement, built: Built, opts: MountOptions = {}): MountedPage {
  const m = kitOf(built, opts);
  const nodes = new Nodes(m);
  let live = true;
  let rendering = false;
  let again = false;
  let settling = false;
  const element = (id: string) => root.querySelector<HTMLElement>(`[data-xtmd-site="${CSS.escape(id)}"]`) ?? undefined;
  const drawing = drawingOf(m, nodes, built, `xtmd${++mounts}-`, {
    layout: (_id, over) => {
      if (over) settle();
    },
  });
  const patcher = new Patcher<globalThis.Node>(domHost, root, true);
  const still = built.html.map((run, k): Child | undefined => (run === true ? { tag: "", key: `still${k}`, attrs: {}, children: [] } : undefined));
  /** What the program draws, part by part, with what stays as it is where it was written. */
  const runs = (): Child[][] => {
    const drawn = nodes.all();
    let k = 0;
    return still.map((s) => {
      if (s !== undefined) return [s];
      const from = k ? m.ends[k - 1] : 0;
      return children(drawing, drawn.slice(from, m.ends[k++]));
    });
  };
  const theme = () => root.setAttribute("data-xtmd-theme", drawing.tokens.theme().toLowerCase());
  theme();
  const there = [...root.childNodes];
  patcher.adoptAll(runs().map((c, k) => ({ children: c, live: there.splice(0, Number(built.html[k])) })), domReads);
  for (const n of there) n.remove();

  function render(): void {
    if (!live) return;
    if (rendering) {
      again = true;
      return;
    }
    rendering = true;
    const refocus = focusOf(root);
    try {
      theme();
      for (let k = 0; k < 4; k++) {
        again = false;
        patcher.renderAll(runs().flat());
        if (!again) break;
      }
    } finally {
      rendering = false;
    }
    refocus?.();
  }

  const page = {
    tree: () => ({ nodes: nodes.all() }),
    settle: (pass: Parameters<Kit["settle"]>[0]) => m.settle(pass),
    token: (of: Input) => drawing.tokens.value(of),
  } as unknown as Page;

  function settle(): void {
    if (!live || settling || !built.layout) return;
    settling = true;
    try {
      built.layout(page, element);
    } catch (f) {
      m.errors.push(f);
    } finally {
      settling = false;
    }
  }

  m.listener = () => {
    render();
    settle();
    opts.onChange?.(handle);
  };
  const name = (x: Input | undefined) => (x && "case" in x ? x.case.name : undefined);
  const handle: MountedPage = {
    act(a) {
      if ("event" in a) m.event(a.event.id, a.event.name, a.event.args);
      else if ("write" in a) m.write(a.write.id, a.write.input, a.write.value);
      else if ("set" in a) m.set(a.set.fields);
      else if ("expose" in a) m.expose(a.expose.channel, a.expose.name, a.expose.args);
      else if ("extern" in a) m.extern(a.extern.channel, a.extern.values);
      else if ("page" in a) m.page({ width: name(a.page.values.width), theme: name(a.page.values.theme) });
      else m.restoreData(a.restore.data);
    },
    data: () => ({ state: m.stateNow(), own: m.ownNow() }),
    told: () => m.told(),
    left: m.left,
    destroy() {
      live = false;
      m.listener = () => {};
      patcher.destroy();
    },
  };
  settle();
  return handle;
}
