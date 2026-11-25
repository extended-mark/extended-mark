// A page the engine built, as a Solid component: what xtmd-solid writes its components with.

import { untrack, type Component, type JSX } from "solid-js";
import type { Held, HeldProps, Input, Restored, Snapshot } from "@extended-mark/view";
import { heldNow, useHeld, type SolidWidgetProps } from "./elements.js";

/** What `ref` on a built page is given. */
export interface BuiltHandle {
  set(changes: Record<string, Input>): void;
  state(): Record<string, Input>;
  expose(channel: string, name: string, ...args: Input[]): void;
  snapshot(): Snapshot;
  restore(snap: Snapshot): Restored;
}

export interface BuiltProps extends Omit<HeldProps, "components"> {
  components?: Record<string, Component<SolidWidgetProps>>;
  ref?: (handle: BuiltHandle) => void;
  id?: string;
  class?: string;
}

/** A page's component: `make` is what holds it. */
export function component(_name: string, make: (props: HeldProps) => Held): Component<BuiltProps> {
  return (props): JSX.Element => {
    const { doc, element } = useHeld(make, () => heldNow(props as HeldProps), props);
    untrack(() =>
      props.ref?.({
        set: (c) => doc.set(c),
        state: () => doc.state(),
        expose: (c, n, ...a) => doc.expose(c, n, ...a),
        snapshot: () => doc.snapshot(),
        restore: (s) => doc.restore(s),
      }),
    );
    return element();
  };
}
