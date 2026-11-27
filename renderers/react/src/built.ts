"use client";
// A page the engine built, as a React component: what xtmd-react writes its components with.

import { useState, type ComponentType, type ReactElement, type Ref } from "react";
import type { Input } from "@extended-mark/runtime";
import type { Held, HeldProps, Restored, Snapshot } from "@extended-mark/widgets";
import { useHeld, useUid, type Around, type ReactWidgetProps } from "./elements.js";

/** What the host holds to reach a built page: its `ref`. */
export interface BuiltHandle {
  set(changes: Record<string, Input>): void;
  state(): Record<string, Input>;
  expose(channel: string, name: string, ...args: Input[]): void;
  snapshot(): Snapshot;
  restore(snap: Snapshot): Restored;
}

export interface BuiltProps extends Omit<HeldProps, "components">, Around {
  components?: Record<string, ComponentType<ReactWidgetProps>>;
  ref?: Ref<BuiltHandle | null>;
}

/** A page's component: `make` is what holds it. */
export function component(name: string, make: (props: HeldProps) => Held): (props: BuiltProps) => ReactElement {
  const Page = ({ ref, id, className, style, ...props }: BuiltProps): ReactElement => {
    const uid = useUid();
    const [doc] = useState(() => make({ ...props, uid }));
    return useHeld<HeldProps, Held, BuiltHandle>(
      doc,
      props,
      ref,
      (d) => ({
        set: (c) => d.set(c),
        state: () => d.state(),
        expose: (c, n, ...a) => d.expose(c, n, ...a),
        snapshot: () => d.snapshot(),
        restore: (s) => d.restore(s),
      }),
      { id, className, style },
    );
  };
  Page.displayName = name;
  return Page;
}
