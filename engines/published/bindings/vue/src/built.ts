// A page the engine built, as a Vue component: what xtmd-vue writes its components with.

import { defineComponent } from "vue";
import type { Held, HeldProps } from "@extended-mark/view";
import { heldNow, heldProps, useHeld } from "./elements.js";

/** A page's component: `make` is what holds it. */
export function component(name: string, make: (props: HeldProps) => Held) {
  return defineComponent({
    name,
    props: heldProps,
    setup(p, { expose }) {
      const { doc, render } = useHeld(make, () => heldNow(p as HeldProps));
      expose({
        set: doc.set.bind(doc),
        state: () => doc.state(),
        expose: doc.expose.bind(doc),
        snapshot: () => doc.snapshot(),
        restore: doc.restore.bind(doc),
      });
      return render;
    },
  });
}
