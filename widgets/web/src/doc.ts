// A document run from its IR, as a renderer holds it: a runtime's page and this widget set, put together. It is an entry
// of its own, as nothing else here needs a runtime.

import { channelName, irText, makePage, sameIr, type Ir, type Page, type PageOptions } from "@extended-mark/runtime";
import { webFns } from "./fns.js";
import { Held, type HeldProps } from "./held.js";
import { layoutPlanes, measured, type Measure } from "./planes.js";
import { dataOf, hash, type Snapshot } from "./snapshot.js";
import { css as styleText } from "./style.js";
import { Tokens } from "./tokens.js";
import type { ViewWidget } from "./draw.js";
import { widgets as core } from "./widgets/index.js";

/** What a host gives the document: the same in every framework. */
export interface DocProps extends HeldProps {
  /** The page's IR, in hand: the file `xtmd compile` writes. */
  ir?: Ir;
  /** A pack's own extern widgets, drawn the way core's are, by full name. */
  widgets?: Record<string, ViewWidget>;
  /** How the plane's spots are measured: left out, from their elements. The checks give fixed sizes. */
  measure?: (page: Page) => Measure;
  /** A page was made: from the IR the document was given, and from each other one after it. */
  onPage?: (page: Page) => void;
}

/** A document a framework's renderer renders. */
export class Doc extends Held<DocProps> {
  /** The IR the page runs. */
  private irFile: Ir | undefined;
  private irHash = "";
  private registry: { given: DocProps["widgets"]; all: Record<string, ViewWidget> } | undefined;
  /** The page reads its value fns through this, so it always calls the latest props' fns. */
  private readonly fns: NonNullable<PageOptions["fns"]>;

  constructor(props: DocProps) {
    super(props);
    const web = webFns();
    this.fns = new Proxy({}, { get: (_t, name) => (typeof name === "string" ? (this.props.fns?.[name] ?? web[name]) : undefined), has: (_t, name) => typeof name === "string" && (name in web || !!this.props.fns?.[name]) });
    if (props.ir) this.start(props.ir, props.state ?? props.defaultState, props.defaultSnapshot);
  }

  /** The running page, once it has its IR. */
  get running(): Page | undefined {
    return this.held as Page | undefined;
  }

  override update(next: DocProps): void {
    if (!this.stopped && next.ir && !sameIr(next.ir, this.irFile)) {
      this.props = next;
      // a new page: `$state` carried over where its fields still exist
      this.start(next.ir, next.state ?? this.running?.state() ?? next.defaultState, undefined);
      return;
    }
    super.update(next);
  }

  /** The page as Markdown, with the values it has now. */
  markdown(): string {
    return this.running?.markdown() ?? "";
  }

  protected widgets(): Record<string, ViewWidget> {
    const given = this.props.widgets;
    if (!this.registry || this.registry.given !== given) this.registry = { given, all: { ...core, ...given } };
    return this.registry.all;
  }

  protected css(): string {
    return styleText + this.tokens!.sheet().css;
  }

  protected identity(): string {
    return this.irHash;
  }

  protected channel(name: string): string {
    return channelName(this.running!.ir, name);
  }

  protected layOut(element: (id: string) => HTMLElement | null): void {
    const page = this.running!;
    layoutPlanes(page, this.props.measure?.(page) ?? measured(element));
  }

  private start(irFile: Ir, given: DocProps["state"], snap: Snapshot | undefined): void {
    this.irFile = irFile;
    this.irHash = hash(irText(irFile));
    const p = this.props;
    // `$state` as the host gives it, checked as a host's write is: what doesn't fit is refused
    const supplied = { ...this.widgets(), ...p.components };
    const { page, outcome } = makePage(irFile, { page: p.page, externs: p.externs, fns: this.fns, supplies: (w) => Object.hasOwn(supplied, w), data: dataOf(snap, this.irHash) }, given);
    this.hold(page, new Tokens(page), outcome);
    p.onPage?.(page);
  }
}
