<!--
  A page the engine built, drawn by Svelte itself, on a server as on the client: what xtmd-svelte's components are. `make`
  is what holds the page; the rest are its props. bind:this gives snapshot(), restore(), set(), getState() and expose().
-->
<script lang="ts">
  import { onDestroy } from "svelte";
  import type { Input } from "@extended-mark/runtime";
  import type { Held, HeldProps, Restored, Snapshot } from "@extended-mark/widgets";
  import XtmdNode from "./XtmdNode.svelte";

  let { make, class: className = undefined, id = undefined, ...props }: HeldProps & { make: (props: HeldProps) => Held; class?: string; id?: string } = $props();

  // Svelte's own id, which is the same on a server as in the browser
  const uid = $props.id();
  // made once, from the props it starts with; the $effect below follows them after
  // svelte-ignore state_referenced_locally
  const doc = make({ ...props, uid: `xtmd${uid}-` });
  let version = $state(0);
  onDestroy(doc.subscribe(() => version++));
  // the page follows its props
  $effect(() => {
    doc.update({ ...props });
  });
  const root = $derived.by(() => {
    void version;
    const r = doc.render();
    if (id) r.attrs.id = id;
    if (className) r.attrs.class = `${r.attrs.class} ${className}`;
    return r;
  });

  export function set(changes: Record<string, Input>): void {
    doc.set(changes);
  }
  /** `$state` now (`state` is a rune's name in Svelte). */
  export function getState(): Record<string, Input> {
    return doc.state();
  }
  export function expose(channel: string, name: string, ...args: Input[]): void {
    doc.expose(channel, name, ...args);
  }
  export function snapshot(): Snapshot {
    return doc.snapshot();
  }
  export function restore(snap: Snapshot): Restored {
    return doc.restore(snap);
  }
</script>

<XtmdNode v={root} />
