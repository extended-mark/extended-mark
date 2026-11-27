"use client";
// <XtmdDoc>: an Extended Mark page run from its IR and drawn by React itself (elements.ts).

import { useState, type ComponentType, type ReactElement, type Ref } from "react";
import type { Input } from "@extended-mark/runtime";
import type { Restored, Snapshot } from "@extended-mark/widgets";
import { Doc, type DocProps } from "@extended-mark/widgets/doc";
import { useHeld, useUid, type Around, type ReactWidgetProps } from "./elements.js";

export { toReact, type ReactWidgetProps } from "./elements.js";

/** What the host holds to reach the page: `ref` on <XtmdDoc>. */
export interface XtmdDocHandle {
  /** The running document. */
  readonly doc: Doc;
  /** Writes `$state` fields as one change, checked: refused whole (and told to `onError`), or done. */
  set(changes: Record<string, Input>): void;
  /** `$state` now. */
  state(): Record<string, Input>;
  /** The page as Markdown, with the values it has now. */
  markdown(): string;
  /** Calls a fn the document exposes: `expose("tracker", "clear_done")`. */
  expose(channel: string, name: string, ...args: Input[]): void;
  /** Everything the page holds now: `$state` and every widget's own variables. */
  snapshot(): Snapshot;
  /** Puts a snapshot back: all of it with the same IR, `$state` only with another. */
  restore(snap: Snapshot): Restored;
}

export interface XtmdDocProps extends Omit<DocProps, "components">, Around {
  /** React components as widgets, by full name: `{ "acme/chart": Chart }`. */
  components?: Record<string, ComponentType<ReactWidgetProps>>;
  ref?: Ref<XtmdDocHandle | null>;
}

/**
 * An Extended Mark page. `ir` is its IR. `state` with
 * `onStateChange` controls `$state`, or `defaultState` starts it; every write is checked by xtmd,
 * and what it refuses reaches `onError`. Its `ref` is the handle: `snapshot`, `restore`, `set`,
 * `state`, `markdown` and `expose`.
 */
export function XtmdDoc({ ref, id, className, style, ...props }: XtmdDocProps): ReactElement {
  const uid = useUid();
  const [doc] = useState(() => new Doc({ ...props, uid }));
  return useHeld<DocProps, Doc, XtmdDocHandle>(
    doc,
    props,
    ref,
    (d) => ({
      doc: d,
      set: (c) => d.set(c),
      state: () => d.state(),
      markdown: () => d.markdown(),
      expose: (c, n, ...a) => d.expose(c, n, ...a),
      snapshot: () => d.snapshot(),
      restore: (s) => d.restore(s),
    }),
    { id, className, style },
  );
}
