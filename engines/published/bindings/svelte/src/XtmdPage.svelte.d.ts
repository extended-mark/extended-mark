import type { Component } from "svelte";
import type { Input } from "@extended-mark/runtime";
import type { Held, HeldProps, Restored, Snapshot } from "@extended-mark/widgets";

declare const XtmdPage: Component<
  HeldProps & { make: (props: HeldProps) => Held; class?: string; id?: string },
  {
    set(changes: Record<string, Input>): void;
    getState(): Record<string, Input>;
    expose(channel: string, name: string, ...args: Input[]): void;
    snapshot(): Snapshot;
    restore(snap: Snapshot): Restored;
  }
>;
export default XtmdPage;
