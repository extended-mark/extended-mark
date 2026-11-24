// @extended-mark/dom's API, fixed before its widgets so that the bindings, the web engine and the
// playground can all be written against it.
//
// A page is drawn from its tree (docs/specs/06, The host): each node becomes one WidgetView, kept across
// changes by its id (a loop row's widgets follow its item), and updated in place,
// so focus, selection and scroll survive a change.

import type { Input, Measure, Node as Drawn, Page, PageOptions, ir } from "@extended-mark/runtime";

/** What a widget can do on the page it is drawn on. */
export interface WidgetContext {
  readonly page: Page;
  /** The reader's event on this widget: runs the caller's handler, as one change. */
  event(name: string, args?: Input[]): void;
  /** This widget writing one of its `mut` inputs, as a slider does: checked, then one change. */
  write(input: string, value: Input): void;
  /** A fn input called, as a shade calls its shader. */
  call(input: string, args: Input[]): Input;
  /** Draws an input's widgets into a container, keeping the ones already there by id. */
  children(container: Element, nodes: readonly Drawn[]): void;
  /** What a token stands for: `space.M` is 16, `color.INK` a colour for each theme. */
  token(v: Input | undefined): Input | undefined;
}

/** One drawn widget: its element, and how it follows the page. */
export interface WidgetView {
  /** Its root: an element, on which @extended-mark/dom sets `data-xtmd-site` (its id) and
   * `data-xtmd-widget`, or text, for words of a sentence. */
  readonly el: Element | Text;
  /** The page changed: show this widget's new inputs. */
  update(d: Drawn): void;
  /** For the checks: the reader acting on it through its own DOM, as a person would.
   * Returns false when this widget can't do it that way, and the caller goes through the page. */
  act?(action: Action): boolean;
  destroy?(): void;
}

/** What a reader does to a widget, in the checks' terms. */
export type Action = { event: string; args?: Input[] } | { write: string; value: Input };

/** A widget's maker: our defaults for every `core/*` extern, or the host's own, by full name. */
export type Widget = (d: Drawn, ctx: WidgetContext) => WidgetView;

export interface MountOptions {
  /** The host's widgets, by full name (`core/press`, `acme/chart`): they replace ours. */
  widgets?: Record<string, Widget>;
  /** What is drawn where a widget failed, in place of our notice: `d.failed` says why. */
  failed?: Widget;
  /** How spots are measured for the plane's layout. Left out, from the DOM: a ResizeObserver
   * and each child's box in page lengths (CSS pixels). The checks pass fixed sizes. */
  measure?: Measure;
}

/** A page drawn into an element. */
export interface Mounted {
  readonly page: Page;
  readonly root: HTMLElement;
  /** The widget view of a node, by its id, if it is on the page now. */
  view(id: string): WidgetView | undefined;
  /** The reader acting on a widget: through its DOM when it can, else the page. */
  act(id: string, action: Action): void;
  /** The plane's layout, run now (it also runs after every change on its own). */
  layout(): void;
  unmount(): void;
}

export type { Drawn, Input, Measure, Page, PageOptions, ir };
