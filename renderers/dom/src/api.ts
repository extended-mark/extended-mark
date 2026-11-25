// @extended-mark/dom's API: a page drawn into an element, with a widget set.

import type { Input, Node as Drawn, Page, PageOptions, ir } from "@extended-mark/runtime";
import type { Action, Measure, ViewWidget, WidgetProps } from "@extended-mark/widgets";

/** A host's own element for a widget, where a fn of plain elements won't do: what follows the widget, and takes
 * it away. */
export interface Follows {
  update(props: WidgetProps): void;
  destroy?(): void;
}

/** Draws a widget into an element of its own. `children` draws an input's widgets into a container. */
export type OwnWidget = (el: HTMLElement, props: WidgetProps, children: (container: Element, input: string) => void) => Follows;

export interface MountOptions {
  /** The host's widgets, by full name (`core/press`, `acme/chart`): they replace ours. */
  widgets?: Record<string, ViewWidget>;
  /** The host's own elements, by the widget each draws. */
  own?: Record<string, OwnWidget>;
  /** What is drawn where a widget failed, in place of our notice: `d.failed` says why. */
  failed?: ViewWidget;
  /** How spots are measured for the plane's layout. Left out, from the DOM: a ResizeObserver
   * and each child's box in page lengths (CSS pixels). The checks pass fixed sizes. */
  measure?: Measure;
}

/** A page drawn into an element. */
export interface Mounted {
  readonly page: Page;
  readonly root: HTMLElement;
  /** A node's element, by its id, if it is on the page now and isn't words. */
  element(id: string): HTMLElement | undefined;
  /** The reader acting on a widget: through its DOM when it can, else the page. */
  act(id: string, action: Action): void;
  /** The plane's layout, run now (it also runs after every change on its own). */
  layout(): void;
  unmount(): void;
}

export type { Action, Drawn, Input, Measure, Page, PageOptions, ir };
