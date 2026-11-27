// <XtmdDoc>: an Extended Mark page run from its IR and drawn by Vue itself (elements.ts). Listeners for the host are
// callback props: `@state-change="…"` is `onStateChange`.

import { defineComponent, type PropType } from "vue";
import type { Input } from "@extended-mark/runtime";
import type { Restored, Snapshot } from "@extended-mark/widgets";
import { Doc, type DocProps } from "@extended-mark/widgets/doc";
import { heldNow, heldProps, useHeld } from "./elements.js";

export { toVue } from "./elements.js";

/** <XtmdDoc>'s props, for a component that wraps it (@extended-mark/vue-live). */
export const docProps = {
  /** The page's IR, in hand. Kept as given: Vue doesn't make it reactive. */
  ir: Object as PropType<DocProps["ir"]>,
  ...heldProps,
  widgets: Object as PropType<DocProps["widgets"]>,
  /** How the plane's spots are measured: left out, from their elements. */
  measure: Function as PropType<DocProps["measure"]>,
};

/** What a template ref to <XtmdDoc> holds. */
export interface XtmdDocRef {
  doc: Doc;
  set(changes: Record<string, Input>): void;
  state(): Record<string, Input>;
  markdown(): string;
  expose(channel: string, name: string, ...args: Input[]): void;
  snapshot(): Snapshot;
  restore(snap: Snapshot): Restored;
}

/**
 * An Extended Mark page. `ir` is its IR. `state` with
 * `@state-change` controls `$state`, or `default-state` starts it; every write is checked by xtmd,
 * and what it refuses reaches `@error`. A template ref holds `snapshot`, `restore`, `set`,
 * `state`, `markdown` and `expose`.
 */
export const XtmdDoc = defineComponent({
  name: "XtmdDoc",
  props: docProps,
  setup(p, { expose }) {
    const { doc, render } = useHeld(
      (props: DocProps) => new Doc(props),
      (): DocProps => ({ ...heldNow(p as DocProps), ir: p.ir, widgets: p.widgets, measure: p.measure }),
    );
    const ref: XtmdDocRef = {
      doc,
      set: (c) => doc.set(c),
      state: () => doc.state(),
      markdown: () => doc.markdown(),
      expose: (c, n, ...a) => doc.expose(c, n, ...a),
      snapshot: () => doc.snapshot(),
      restore: (s) => doc.restore(s),
    };
    expose(ref);
    return render;
  },
});
