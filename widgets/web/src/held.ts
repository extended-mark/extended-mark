// A page held for a framework: its `$state` the host's to control or to start from, every write checked, what was
// refused told to the host, and a snapshot that takes everything the page holds out and puts it back. No framework
// here, and no page: one that is run from its IR and one an engine built are held the same. A framework's renderer
// renders `render()` with its own elements and calls `update` with its props.

import type { Call, Input, Node, Outcome, PageOptions, PageValues } from "@extended-mark/runtime";
import { children, type Drawing, type ViewWidget } from "./draw.js";
import { RELEASE, type DocError, type Restored, type Snapshot } from "./snapshot.js";
import type { Tokens } from "./tokens.js";
import { h, type Child, type VNode } from "./vnode.js";

/** What a host gives a page, whichever way it is run: the same in every framework. */
export interface HeldProps {
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
  /** The host's own components as widgets, by full name (`acme/chart`), each given a widget's node, what
   * it does (`event`, `write`, `call`) and its slots, as its framework gives them. */
  components?: Record<string, unknown>;
  /** What the page's elements' ids start with, the same on a server as in the browser: a framework's renderer gives its
   * own, and left out it is a count, which only one page of a document started by a script can rely on. */
  uid?: string;
  /** What is drawn where a widget failed, in place of our notice: `d.failed` says why. */
  failed?: ViewWidget;
  /** `$state` after each change to it. */
  onStateChange?: (state: Record<string, Input>) => void;
  /** A write xtmd refused, a widget that failed. */
  onError?: (error: DocError) => void;
  /** The host's extern fns the page called, after each change that called any. */
  onCalls?: (calls: Call[]) => void;
}

/** A page, however it is run: the host's side of it (docs/specs/06). */
export interface HeldPage {
  tree(): { nodes: Node[] };
  node(id: string): Node | undefined;
  event(id: string, name: string, args?: Input[]): unknown;
  write(id: string, input: string, value: Input): unknown;
  call(id: string, input: string, args: Input[]): Input;
  set(fields: Record<string, Input>): void;
  setExtern(channel: string, values: Record<string, Input>): void;
  setPage(page: PageValues): void;
  expose(channel: string, name: string, args: Input[]): void;
  state(): Record<string, Input>;
  data(): { state: Record<string, Input>; own?: Record<string, Record<string, Input>> };
  restore(data: { state?: Record<string, Input>; own?: Record<string, Record<string, Input>> }): string[] | undefined;
  takeCalls(): Call[];
  takeOutcome(): Outcome;
  onChange(f: () => void): () => void;
  /** Moves at every change. */
  readonly changes: number;
  readonly errors: unknown[];
}

let docs = 0;

/** A page a framework's renderer draws. */
export abstract class Held<P extends HeldProps = HeldProps> {
  protected props: P;
  private unlisten: (() => void) | undefined;
  private drawing: { page: HeldPage; of: unknown[]; d: Drawing } | undefined;
  private root: Element | undefined;
  private shown: string | undefined;
  private queued = false;
  private gone = false;
  private wantLayout = false;
  private rendering = false;
  /** The page, once there is one. */
  protected held: HeldPage | undefined;
  protected tokens: Tokens | undefined;
  /** Bumped at every change the page draws: a renderer's store snapshot. */
  version = 0;
  readonly uid: string;
  private readonly listeners = new Set<() => void>();

  constructor(props: P) {
    this.props = props;
    this.uid = props.uid ?? `xtmdv${++docs}-`;
  }

  /** Whether it can't be used any more. */
  protected get stopped(): boolean {
    return this.gone;
  }

  // ---- what each way of running a page says of it

  /** Its widgets, by full name. */
  protected abstract widgets(): Record<string, ViewWidget>;
  /** Its stylesheet, with its tokens. */
  protected abstract css(): string;
  /** What its places mean the same widgets in: a snapshot of another has only its `$state`. */
  protected abstract identity(): string;
  /** A channel's full name. */
  protected abstract channel(name: string): string;
  /** Its planes laid out, measured from the elements. */
  protected abstract layOut(element: (id: string) => HTMLElement | null): void;

  // ---- what a renderer calls

  /** Called after every change the page draws: the renderer renders again. */
  subscribe(f: () => void): () => void {
    this.listeners.add(f);
    return () => this.listeners.delete(f);
  }

  /** The page as elements: a placeholder while there is no page. */
  render(): VNode {
    const page = this.held;
    if (!page || !this.tokens) return h("div", undefined, { class: "xtmd-root", "data-xtmd-root": "", "data-xtmd-loading": "" });
    this.rendering = true;
    let root: VNode;
    try {
      root = this.rootOf(page, this.tokens);
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
  private rootOf(page: HeldPage, tokens: Tokens): VNode {
    const { components, failed } = this.props;
    const of = [this.widgets(), components, failed];
    if (this.drawing?.page !== page || !of.every((x, k) => x === this.drawing!.of[k])) this.drawing = { page, of, d: this.drawingOf(page, tokens) };
    return h("div", undefined, { class: `xtmd-root ${tokens.sheet().name}`, "data-xtmd-root": "", "data-xtmd-theme": tokens.theme().toLowerCase() }, [
      { tag: "style", attrs: { "data-xtmd-dom": "" }, children: [this.css()] },
      ...children(this.drawing.d, page.tree().nodes),
    ]);
  }

  private drawingOf(page: HeldPage, tokens: Tokens): Drawing {
    const { components, failed } = this.props;
    const asked = new Map<string, number>();
    // a reader's event or write, done or refused, reaches the host
    const told = (): void => this.report();
    return {
      registry: this.widgets(),
      components,
      failed,
      tokens,
      uid: this.uid,
      kept: new WeakMap<Node, Child>(),
      node: (id) => page.node(id),
      event: (id, name, args) => {
        page.event(id, name, args);
        told();
      },
      write: (id, input, value) => {
        page.write(id, input, value);
        told();
      },
      call: (id, input, args) => page.call(id, input, args),
      // after every change but the layout's own: a spot in a list made anew at each redraw, whose own box
      // never stays, would lay the page out for ever
      layout: (id, again) => {
        if (!again && asked.get(id) === page.changes) return;
        asked.set(id, page.changes);
        this.layout();
      },
      fail: (f) => void page.errors.push(f),
    };
  }

  /** The host's props changed: do what each change means. */
  update(next: P): void {
    const prev = this.props;
    this.props = next;
    const page = this.held;
    if (this.gone || !page) return;
    if (next.page && JSON.stringify(next.page) !== JSON.stringify(prev.page)) page.setPage(next.page);
    if (next.externs !== prev.externs) {
      for (const [c, values] of Object.entries(next.externs ?? {})) {
        if (JSON.stringify(values) === JSON.stringify(prev.externs?.[c])) continue;
        page.setExtern(this.channel(c), values);
      }
    }
    // controlled: a new `$state` from the host wins, checked as any write of the host's is; one xtmd
    // refuses is told to onError once, and the page keeps the last that fit
    if (next.state && JSON.stringify(next.state) !== JSON.stringify(prev.state) && JSON.stringify(next.state) !== JSON.stringify(page.state())) {
      page.set(next.state);
      this.report();
    }
  }

  /** Stops the page: it can't be used again. */
  destroy(): void {
    this.gone = true;
    this.unlisten?.();
    this.listeners.clear();
  }

  // ---- what a host does through a renderer's handle

  /** Writes `$state` fields as one change, checked: refused whole, or done. */
  set(changes: Record<string, Input>): void {
    this.held?.set(changes);
    this.report();
  }

  /** `$state` now. */
  state(): Record<string, Input> {
    return this.held?.state() ?? {};
  }

  /** Calls a fn the document exposes. */
  expose(channel: string, name: string, ...args: Input[]): void {
    this.held?.expose(this.channel(channel), name, args);
  }

  /** Everything the page holds now: `$state`, and every widget's own variables by place. */
  snapshot(): Snapshot {
    return { xtmd: RELEASE, ir: this.identity(), state: this.state(), own: this.held?.data().own ?? {} };
  }

  /** Puts a snapshot back. Of the same page, all of it; of another, `$state` only, as widgets'
   * places may now be other widgets. `$state` goes back as one checked write. */
  restore(snap: Snapshot): Restored {
    const page = this.held;
    if (!page) return { restored: "none", dropped: [], why: "the page isn't loaded yet" };
    if (!snap || typeof snap !== "object" || snap.xtmd !== RELEASE || typeof snap.state !== "object") {
      return { restored: "none", dropped: [], why: `a snapshot this xtmd (${RELEASE}) can't read` };
    }
    const same = snap.ir === this.identity();
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

  /** A page to hold, with what was told as it was made. */
  protected hold(page: HeldPage, tokens: Tokens, outcome: Outcome): void {
    this.unlisten?.();
    this.held = page;
    this.tokens = tokens;
    this.shown = JSON.stringify(page.state());
    for (const r of outcome.refused) this.props.onError?.({ kind: "refused", ...r });
    for (const f of outcome.failures) this.props.onError?.({ kind: "failed", ...f });
    this.unlisten = page.onChange(() => this.changed());
    this.notify();
  }

  private notify(): void {
    this.version++;
    for (const l of this.listeners) l();
  }

  /** After a change the page drew: the renderer renders, then the host hears what changed. */
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
    const page = this.held;
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
    const o = page.takeOutcome();
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
    const page = this.held;
    const root = this.root;
    if (!page || !root || !this.wantLayout) return;
    this.wantLayout = false;
    try {
      this.layOut((id) => root.querySelector<HTMLElement>(`[data-xtmd-site="${CSS.escape(id)}"]`));
    } catch (f) {
      page.errors.push(f);
    }
  }
}
