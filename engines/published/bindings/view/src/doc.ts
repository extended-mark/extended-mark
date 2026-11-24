// A document running, as a framework engine holds it: made from its IR, its `$state` the host's to control or to start from, every write checked by xtmd, what was
// refused told to the host, and a snapshot that takes everything the page holds out and puts it
// back. No framework here: an engine renders `render()` with its own renderer and calls `update`
// with its props.

import { Page, type Call, type Node, type Outcome, type PageOptions, type PageValues, type Input, type ir } from "@extended-mark/runtime";
import { channelName, irText, makePage, sameIr, type Ir } from "./page.js";
import { RELEASE, dataOf, hash, type DocError, type Restored, type Snapshot } from "./snapshot.js";
import { Num } from "@extended-mark/runtime/num";
import { Tokens, children, h, layoutPlanes, styleText, webFns, widgets as core, type Child, type Drawing, type Measure, type VNode, type ViewWidget } from "@extended-mark/widgets";

/** What an engine gives the document: the same in every framework. */
export interface DocProps {
  /** The page's IR, in hand: the file `xtmd build` writes. */
  ir?: Ir;
  /** `$state`, controlled: the page shows this, and every change goes through `onStateChange`. */
  state?: Record<string, Input>;
  /** `$state` to start from, when the host doesn't control it. */
  defaultState?: Record<string, Input>;
  /** A snapshot to start from: `$state` and every widget's own variables. */
  defaultSnapshot?: Snapshot;
  /** The page's own values: its width and theme. */
  page?: PageValues;
  /** Each extern channel's values, by its name in the document (`tracker`) or in full. */
  externs?: Record<string, Record<string, Input>>;
  /** Input fns the host supplies, by full name (`acme/rate`). */
  fns?: PageOptions["fns"];
  /** A pack's own extern widgets, drawn the way core's are, by full name. */
  widgets?: Record<string, ViewWidget>;
  /** The host's own components as widgets, by full name (`acme/chart`), each given a widget's node, what
   * it does (`event`, `write`, `call`) and its slots, as its framework gives them. */
  components?: Record<string, unknown>;
  /** What is drawn where a widget failed, in place of our notice: `d.failed` says why. */
  failed?: ViewWidget;
  /** How the plane's spots are measured: left out, from their elements. The checks give fixed sizes. */
  measure?: (page: Page) => Measure;
  /** `$state` after each change to it. */
  onStateChange?: (state: Record<string, Input>) => void;
  /** A write xtmd refused, a widget that failed. */
  onError?: (error: DocError) => void;
  /** The host's extern fns the page called, after each change that called any. */
  onCalls?: (calls: Call[]) => void;
}

let docs = 0;

/** A document a framework engine renders. */
export class Doc {
  private props: DocProps;
  private page: Page | undefined;
  private tokens: Tokens | undefined;
  /** The IR the page runs. */
  private irFile: Ir | undefined;
  private irHash = "";
  private unlisten: (() => void) | undefined;
  private drawing: ReturnType<Doc["drawingOf"]> | undefined;
  private root: Element | undefined;
  private shown: string | undefined;
  private queued = false;
  private gone = false;
  private wantLayout = false;
  private rendering = false;
  /** Bumped at every change the page draws: an engine's store snapshot. */
  version = 0;
  readonly uid = `xtmdv${++docs}-`;
  private readonly listeners = new Set<() => void>();
  /** The page reads its value fns through this, so it always calls the latest props' fns. */
  private readonly fns: NonNullable<PageOptions["fns"]>;

  constructor(props: DocProps) {
    this.props = props;
    const web = webFns();
    this.fns = new Proxy({}, { get: (_t, name) => (typeof name === "string" ? (this.props.fns?.[name] ?? web[name]) : undefined), has: (_t, name) => typeof name === "string" && (name in web || !!this.props.fns?.[name]) });
    if (props.ir) this.start(props.ir, props.state ?? props.defaultState, props.defaultSnapshot);
  }

  /** Called after every change the page draws: the engine renders again. */
  subscribe(f: () => void): () => void {
    this.listeners.add(f);
    return () => this.listeners.delete(f);
  }

  /** The running page, once it has its IR. */
  get running(): Page | undefined {
    return this.page;
  }

  /** The page as elements: a placeholder while the host has no IR for it yet. */
  render(): VNode {
    if (!this.page || !this.tokens) {
      return h("div", undefined, { class: "xtmd-root", "data-xtmd-root": "", "data-xtmd-loading": "" });
    }
    this.rendering = true;
    let root: VNode;
    try {
      root = this.rootOf(this.page, this.tokens);
    } finally {
      this.rendering = false;
    }
    // the root's element, for the plane's measure and the snapshot's tests
    root.hook = {
      mount: (el) => {
        this.root = el;
        this.settle();
      },
      update: (el) => {
        this.root = el;
        this.settle();
      },
      unmount: () => (this.root = undefined),
    };
    return root;
  }

  /** The page as one root element: its stylesheet, its tokens for both themes, and every drawn widget. */
  private rootOf(page: Page, tokens: Tokens): VNode {
    const { widgets, components, failed } = this.props;
    const dr = this.drawing;
    if (!dr || dr.page !== page || dr.widgets !== widgets || dr.components !== components || dr.failed !== failed) this.drawing = this.drawingOf(page, tokens);
    const sheet = tokens.sheet();
    return h("div", undefined, { class: `xtmd-root ${sheet.name}`, "data-xtmd-root": "", "data-xtmd-theme": tokens.theme().toLowerCase() }, [
      { tag: "style", attrs: { "data-xtmd-dom": "" }, children: [styleText + sheet.css] },
      ...children(this.drawing!, page.tree().nodes),
    ]);
  }

  private drawingOf(page: Page, tokens: Tokens) {
    const { widgets, components, failed } = this.props;
    const asked = new Map<string, number>();
    // a reader's event or write, done or refused, reaches the host
    const told = <T>(x: T): T => (this.report(), x);
    return {
      page,
      widgets,
      registry: { ...core, ...(widgets ?? {}) },
      components,
      failed,
      tokens,
      uid: this.uid,
      kept: new WeakMap<Node, Child>(),
      node: (id) => page.node(id),
      event: (id, name, args) => told(page.event(id, name, args)),
      write: (id, input, value) => told(page.write(id, input, value)),
      call: (id, input, args) => page.call(id, input, args),
      // after every change but the layout's own: a spot in a list made anew at each redraw, whose own box
      // never stays, would lay the page out for ever
      layout: (id, again) => {
        if (!again && asked.get(id) === page.changes) return;
        asked.set(id, page.changes);
        this.layout();
      },
      fail: (f) => page.errors.push(f),
    } satisfies Drawing & { page: Page; widgets: unknown };
  }

  /** The engine's props changed: do what each change means. */
  update(next: DocProps): void {
    const prev = this.props;
    this.props = next;
    if (this.gone) return;
    if (next.ir && !sameIr(next.ir, this.irFile)) {
      // a new page: `$state` carried over where its fields still exist
      this.start(next.ir, next.state ?? this.page?.state() ?? next.defaultState, undefined);
      return;
    }
    const page = this.page;
    if (!page) return;
    if (next.page && JSON.stringify(next.page) !== JSON.stringify(prev.page)) page.setPage(next.page);
    if (next.externs !== prev.externs) {
      for (const [c, values] of Object.entries(next.externs ?? {})) {
        if (JSON.stringify(values) === JSON.stringify(prev.externs?.[c])) continue;
        page.setExtern(channelName(page.ir, c), (values));
      }
    }
    // controlled: a new `$state` from the host wins, checked as any write of the host's is; one xtmd
    // refuses is told to onError once, and the page keeps the last that fit
    if (next.state && JSON.stringify(next.state) !== JSON.stringify(prev.state) && JSON.stringify(next.state) !== JSON.stringify(page.state())) {
      page.set((next.state));
      this.report();
    }
  }

  /** Stops the page: the document can't be used again. */
  destroy(): void {
    this.gone = true;
    this.unlisten?.();
    this.listeners.clear();
  }

  // ---- what a host does through an engine's handle

  /** Writes `$state` fields as one change, checked: refused whole, or done. */
  set(changes: Record<string, Input>): void {
    this.page?.set((changes));
    this.report();
  }

  /** `$state` now. */
  state(): Record<string, Input> {
    return this.page?.state() ?? {};
  }

  /** The page as Markdown, with the values it has now. */
  markdown(): string {
    return this.page?.markdown() ?? "";
  }

  /** Calls a fn the document exposes. */
  expose(channel: string, name: string, ...args: Input[]): void {
    if (!this.page) return;
    this.page.expose(channelName(this.page.ir, channel), name, args);
  }

  /** Everything the page holds now: `$state`, and every widget's own variables by place. */
  snapshot(): Snapshot {
    return { xtmd: RELEASE, ir: this.irHash, state: this.state(), own: this.page?.data().own ?? {} };
  }

  /** Puts a snapshot back. With the same IR, all of it; with another, `$state` only, as widgets'
   * places may now be other widgets. `$state` goes back as one checked write. */
  restore(snap: Snapshot): Restored {
    const page = this.page;
    if (!page) return { restored: "none", dropped: [], why: "the page isn't loaded yet" };
    if (!snap || typeof snap !== "object" || snap.xtmd !== RELEASE || typeof snap.state !== "object") {
      return { restored: "none", dropped: [], why: `a snapshot this xtmd (${RELEASE}) can't read` };
    }
    const same = snap.ir === this.irHash;
    const left = page.restore(same ? { state: snap.state, own: snap.own ?? {} } : { state: snap.state });
    const o = page.takeOutcome();
    if (!left) {
      for (const r of o.refused) this.props.onError?.({ kind: "refused", ...r });
      return { restored: "none", dropped: o.refused.map((r) => r.site ?? r.rule ?? "$state"), why: o.refused[0].rule ?? o.refused[0].code };
    }
    this.changed();
    return same ? { restored: "all", dropped: left } : { restored: "state", dropped: ["own"] };
  }

  // ---- inside

  private start(irFile: Ir, given: Record<string, Input> | undefined, snap: Snapshot | undefined): void {
    this.unlisten?.();
    this.irFile = irFile;
    this.irHash = hash(irText(irFile));
    const p = this.props;
    // `$state` as the host gives it, checked as a host's write is: what doesn't fit is refused
    const registry = { ...core, ...(p.widgets ?? {}), ...(p.components ?? {}) };
    const { page, outcome } = makePage(irFile, { page: p.page, externs: p.externs, fns: this.fns, supplies: (w) => Object.hasOwn(registry, w), data: dataOf(snap, this.irHash) }, given);
    this.page = page;
    this.tokens = new Tokens(page);
    this.shown = JSON.stringify(page.state());
    for (const r of outcome.refused) p.onError?.({ kind: "refused", ...r });
    for (const f of outcome.failures) p.onError?.({ kind: "failed", ...f });
    this.unlisten = page.onChange(() => this.changed());
    this.notify();
  }

  private notify(): void {
    this.version++;
    for (const l of this.listeners) l();
  }

  /** After a change the page drew: the engine renders, then the host hears what changed. */
  changed(): void {
    this.notify();
    this.report();
  }

  /** The host hears what the page did, once what is happening now is done: `$state` if it changed,
   * the calls made, and what was refused. A write refused draws nothing new. */
  report(): void {
    if (this.queued) return;
    this.queued = true;
    queueMicrotask(() => {
      this.queued = false;
      if (!this.gone) this.deliver();
    });
  }

  private deliver(): void {
    const page = this.page;
    if (!page) return;
    const p = this.props;
    const state = page.state();
    const json = JSON.stringify(state);
    if (json !== this.shown) {
      this.shown = json;
      p.onStateChange?.(state);
    }
    const calls = page.takeCalls();
    if (calls.length) p.onCalls?.(calls);
    const o: Outcome = page.takeOutcome();
    for (const r of o.refused) p.onError?.({ kind: "refused", ...r });
    for (const f of o.failures) p.onError?.({ kind: "failed", ...f });
  }

  /** A widget asks for the plane's layout: after this render is in the document, or now when it
   * asks between renders (a spot's size changed, with nothing else). */
  private layout(): void {
    this.wantLayout = true;
    if (!this.rendering) this.settle();
  }

  /** The plane's layout, measured from the elements on the page, once a render is in. */
  private settle(): void {
    const page = this.page;
    const root = this.root;
    if (!page || !root || !this.wantLayout) return;
    this.wantLayout = false;
    const at = (id: string) => root.querySelector(`[data-xtmd-site="${CSS.escape(id)}"]`) as HTMLElement | null;
    const measure: Measure = this.props.measure?.(page) ?? {
      spot: (s) => {
        const el = at(s.id);
        return { width: Num.int(Math.max(0, el?.offsetWidth ?? 0)), height: Num.int(Math.max(0, el?.offsetHeight ?? 0)) };
      },
      planeWidth: (pl) => {
        const el = at(pl.id);
        const view = el?.querySelector?.("[data-part='plane.view']") as HTMLElement | null | undefined;
        return Num.int(Math.max(0, Math.round((view ?? el)?.clientWidth ?? 0)));
      },
    };
    try {
      layoutPlanes(page, measure);
    } catch (f) {
      page.errors.push(f);
    }
  }
}
