import type { Component } from "svelte";
import type { LiveProps } from "@extended-mark/live";
import type { Input, ir } from "@extended-mark/runtime";
import type { Restored, Snapshot } from "@extended-mark/widgets";
import type { Doc, DocProps } from "@extended-mark/widgets/doc";

declare const XtmdLiveDoc: Component<
  Omit<DocProps, "ir"> & LiveProps & { class?: string; id?: string },
  {
    document(): Doc;
    set(changes: Record<string, Input>): void;
    getState(): Record<string, Input>;
    markdown(): string;
    expose(channel: string, name: string, ...args: Input[]): void;
    snapshot(): Snapshot;
    restore(snap: Snapshot): Restored;
  }
>;
export default XtmdLiveDoc;
