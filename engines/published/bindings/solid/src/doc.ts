// <XtmdDoc>: an Extended Mark page run from its IR and drawn by Solid itself (elements.ts).

import { untrack, type Component, type JSX } from "solid-js";
import { Doc, type DocProps, type Restored, type Snapshot, type Input } from "@extended-mark/view";
import { heldNow, useHeld, type SolidWidgetProps } from "./elements.js";

export { XtmdNode, type SolidWidgetProps } from "./elements.js";

/** What `ref` on <XtmdDoc> is given. */
export interface XtmdDocHandle {
  readonly doc: Doc;
  set(changes: Record<string, Input>): void;
  state(): Record<string, Input>;
  markdown(): string;
  expose(channel: string, name: string, ...args: Input[]): void;
  snapshot(): Snapshot;
  restore(snap: Snapshot): Restored;
}

export interface XtmdDocProps extends Omit<DocProps, "components"> {
  /** Solid components as widgets, by full name: `{ "acme/chart": Chart }`. */
  components?: Record<string, Component<SolidWidgetProps>>;
  ref?: (handle: XtmdDocHandle) => void;
  id?: string;
  class?: string;
}

/**
 * An Extended Mark page. `ir` is its IR. `state` with
 * `onStateChange` controls `$state`, or `defaultState` starts it; every write is checked by xtmd,
 * and what it refuses reaches `onError`. `ref` is given the handle: `snapshot`, `restore`, `set`,
 * `state`, `markdown` and `expose`.
 */
export function XtmdDoc(props: XtmdDocProps): JSX.Element {
  const { doc, element } = useHeld(
    (p: DocProps) => new Doc(p),
    (): DocProps => ({ ...heldNow(props as DocProps), ir: props.ir, widgets: props.widgets, measure: props.measure }),
    props,
  );
  untrack(() =>
    props.ref?.({
      doc,
      set: (c) => doc.set(c),
      state: () => doc.state(),
      markdown: () => doc.markdown(),
      expose: (c, n, ...a) => doc.expose(c, n, ...a),
      snapshot: () => doc.snapshot(),
      restore: (s) => doc.restore(s),
    }),
  );
  return element();
}
