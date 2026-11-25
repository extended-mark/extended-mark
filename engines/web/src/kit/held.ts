// A built page as a framework holds it (@extended-mark/widgets' Held): the same props and handle as a page run from its
// IR, without its Markdown.

import { Held, dataOf, styleText, type HeldPage, type HeldProps, type Tokens, type ViewWidget } from "@extended-mark/widgets";
import type { Input, Page } from "@extended-mark/runtime";
import type { Kit } from "./kit.js";
import { drawingOf, kitOf, type Built } from "./mount.js";
import { Nodes } from "./nodes.js";

/** What a page is made of, for a framework. */
export interface BuiltPage extends Omit<Built, "html"> {
  /** The IR it was built from, by its hash: a snapshot's places mean the same widgets only in the same one. */
  ir: string;
  /** Its module, which a channel named without one is of. */
  module: string;
}

type Told = ReturnType<HeldPage["takeOutcome"]>;

/** The host's side of a page (docs/specs/06), on its kit. */
class KitPage implements HeldPage {
  readonly nodes: Nodes;
  private readonly calls: ReturnType<HeldPage["takeCalls"]> = [];
  private outcome: Told = { refused: [], failures: [] };

  constructor(readonly kit: Kit) {
    this.nodes = new Nodes(kit);
  }

  private hear(): void {
    const t = this.kit.told();
    this.calls.push(...((t.calls ?? []) as typeof this.calls));
    this.outcome.refused.push(...((t.refused ?? []) as Told["refused"]));
    this.outcome.failures.push(...((t.failures ?? []) as Told["failures"]));
  }

  tree = () => ({ nodes: this.nodes.all() });
  node = (id: string) => this.nodes.of(id);
  event = (id: string, name: string, args?: Input[]) => this.kit.event(id, name, args);
  write = (id: string, input: string, value: Input) => this.kit.write(id, input, value);
  call = (id: string, input: string, args: Input[]) => this.nodes.call(id, input, args);
  set = (fields: Record<string, Input>) => this.kit.set(fields);
  setExtern = (channel: string, values: Record<string, Input>) => this.kit.extern(channel, values);
  setPage = (page: { width?: string; theme?: string }) => this.kit.page(page);
  expose = (channel: string, name: string, args: Input[]) => this.kit.expose(channel, name, args);
  state = () => this.kit.stateNow();
  data = () => ({ state: this.kit.stateNow(), own: this.kit.ownNow() });
  restore = (data: Parameters<HeldPage["restore"]>[0]) => this.kit.restoreData(data);

  takeCalls() {
    this.hear();
    return this.calls.splice(0);
  }

  takeOutcome() {
    this.hear();
    const o = this.outcome;
    this.outcome = { refused: [], failures: [] };
    return o;
  }

  onChange(f: () => void) {
    this.kit.listener = f;
    return () => {
      this.kit.listener = () => {};
    };
  }

  get changes() {
    return this.kit.changes;
  }

  get errors() {
    return this.kit.errors;
  }
}

/** A built page a framework's renderer renders. */
export class BuiltDoc extends Held<HeldProps> {
  private readonly page: KitPage;
  private readonly drawn: Tokens;
  /** The page as its layout sees it, one for good: what is laid out already is known by it. */
  private readonly laid: Page;

  constructor(
    private readonly built: BuiltPage,
    props: HeldProps,
  ) {
    super(props);
    const own = built.fns?.() ?? {};
    // it reads its value fns through this, so it always calls the latest props' fns
    const fns = new Proxy({} as Record<string, (args: readonly Input[]) => Input | null>, {
      get: (_t, name) => (typeof name === "string" ? (this.props.fns?.[name] ?? own[name]) : undefined),
      has: (_t, name) => typeof name === "string" && (name in own || !!this.props.fns?.[name]),
    });
    const externs: Record<string, Record<string, Input>> = {};
    for (const [c, v] of Object.entries(props.externs ?? {})) externs[this.channel(c)] = { ...externs[this.channel(c)], ...v };
    const kit = kitOf({ ...built, fns: undefined, html: [] }, { page: props.page, state: props.state ?? props.defaultState, externs, fns, data: dataOf(props.defaultSnapshot, built.ir) });
    this.page = new KitPage(kit);
    this.drawn = drawingOf(kit, this.page.nodes, { ...built, html: [] }, this.uid).tokens;
    this.laid = { tree: this.page.tree, settle: (pass: Parameters<Kit["settle"]>[0]) => kit.settle(pass), token: (of: Input) => this.drawn.value(of) } as unknown as Page;
    this.hold(this.page, this.drawn, this.page.takeOutcome());
  }

  protected widgets(): Record<string, ViewWidget> {
    return this.built.widgets;
  }

  protected css(): string {
    return styleText + this.drawn.sheet().css;
  }

  protected identity(): string {
    return this.built.ir;
  }

  protected channel(name: string): string {
    if (name.includes("/")) return name;
    const full = `${this.built.module}/${name}`;
    const { ch, ex } = this.built.program;
    return Object.hasOwn(ch ?? {}, full) || Object.hasOwn(ex ?? {}, full) ? full : name;
  }

  protected layOut(element: (id: string) => HTMLElement | null): void {
    this.built.layout?.(this.laid, (id) => element(id) ?? undefined);
  }
}
