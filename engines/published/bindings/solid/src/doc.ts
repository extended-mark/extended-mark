// <XtmdDoc>: an Extended Mark page drawn by Solid itself. @extended-mark/view runs the document and gives the page as
// plain elements after every change; this follows them with Solid's own fine-grained rendering:
// each element is made once, kept by key among its siblings while its tag stays, and only what
// changed is set on it, so focus and what is being typed survive a change. On a server the same
// components render the page's HTML. Written without JSX, so it needs no Solid compiler.

import { createComponent, createEffect, createMemo, createSignal, mapArray, mergeProps, onCleanup, onMount, untrack, type Accessor, type Component, type JSX } from "solid-js";
import { Dynamic, isServer } from "solid-js/web";
import { Doc, type Child, type DocProps, type Restored, type Snapshot, type VNode, type Input, type WidgetProps, type ir } from "@extended-mark/view";

/** A child's key among its siblings: its own, or its place, with its tag (a new tag is a new element). */
function keyOf(c: Child, i: number): string {
  return typeof c === "string" ? `#t${i}` : `${c.key ?? `#${i}`}:${c.tag}`;
}

/** An element's props in Solid's spelling: attributes, style, live DOM properties (`prop:`), and
 * native listeners (`on:`, the DOM's own events with their currentTarget). */
function solidProps(v: VNode): Record<string, unknown> {
  const out: Record<string, unknown> = { ...v.attrs };
  if (v.style) out.style = { ...v.style };
  for (const [k, x] of Object.entries(v.props ?? {})) {
    if (isServer) {
      // a server writes what the property starts as
      if (k === "value" && v.tag !== "textarea") out.value = String(x);
      else if (x === true) out[k] = "";
    } else out[`prop:${k}`] = x;
  }
  if (!isServer) for (const [k, f] of Object.entries(v.on ?? {})) out[`on:${k}`] = f;
  return out;
}

/** The props of a custom element, whose own Solid would set as properties: attributes, by name. */
function attributes(v: VNode): Record<string, unknown> {
  const p = solidProps(v);
  return isServer ? p : Object.fromEntries(Object.entries(p).map(([k, x]) => [k === "style" ? k : `attr:${k}`, x]));
}

/** Children followed by key: each kept while its key is, its element told what changed. */
function children(list: Accessor<Child[]>): JSX.Element {
  const byKey = createMemo(() => new Map(list().map((c, i) => [keyOf(c, i), c] as const)));
  const keys = createMemo(() => [...byKey().keys()], undefined, { equals: (a, b) => a.length === b.length && a.every((k, i) => k === b[i]) });
  return mapArray(keys, (key) => {
    const item = createMemo(() => byKey().get(key)!);
    const first = untrack(item);
    if (typeof first === "string") return (() => item() as string) as unknown as JSX.Element;
    return createComponent(Node, { v: item as Accessor<VNode> });
  }) as unknown as JSX.Element;
}

/** One element: made once with its tag, then followed. Its hook is the latest render's, as one
 * may come later (a page's root has none while its IR is on its way). */
function Node(props: { v: Accessor<VNode> }): JSX.Element {
  const first = untrack(props.v);
  let el: Element | undefined;
  if (!isServer) {
    // the last render this element had: its key's gone by the time it is cleaned up
    let last = first;
    onMount(() => {
      if (el) last.hook?.mount?.(el);
    });
    let started = false;
    createEffect(() => {
      const v = props.v();
      if (!v) return;
      last = v;
      if (!started) {
        started = true;
        return;
      }
      if (el) v.hook?.update?.(el);
    });
    onCleanup(() => {
      if (el) last.hook?.unmount?.(el);
    });
  }
  if (first.tag === "style") {
    return createComponent(Dynamic, { component: "style", ...first.attrs, get innerHTML() {
      return props.v().children.join("");
    } } as never);
  }
  if (first.component) {
    // made once, its node followed
    const part = () => props.v().component ?? first.component!;
    const given = {
      get node() {
        return part().props.node;
      },
      event: (name: string, args?: Input[]) => part().props.event(name, args),
      write: (input: string, value: Input) => part().props.write(input, value),
      call: (input: string, args: Input[]) => part().props.call(input, args),
      slot: (input: string) => children(() => part().children(input)),
    };
    return createComponent(Dynamic, mergeProps({ component: first.tag }, () => attributes(props.v()), {
      get children() {
        return createComponent(first.component!.of as Component<SolidWidgetProps>, given);
      },
    }) as never);
  }
  const textarea = first.tag === "textarea";
  return createComponent(
    Dynamic,
    mergeProps(
      { component: first.tag, ref: (e: Element) => (el = e) },
      () => solidProps(props.v()),
      textarea ? {} : { get children() {
        return children(() => props.v().children);
      } },
    ) as never,
  );
}

/** One of @extended-mark/view's elements, followed by Solid: give it an accessor to the latest render. */
export function XtmdNode(props: { v: Accessor<VNode> }): JSX.Element {
  return createComponent(Node, props);
}

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

/** What a Solid component that draws a widget is given. */
export interface SolidWidgetProps extends WidgetProps {
  /** An input's widgets: a slot's, or a rich_text's. */
  slot(input: string): JSX.Element;
}

export interface XtmdDocProps extends Omit<DocProps, "components"> {
  /** Solid components as widgets, by full name: `{ "acme/chart": Chart }`. */
  components?: Record<string, Component<SolidWidgetProps>>;
  ref?: (handle: XtmdDocHandle) => void;
  id?: string;
  class?: string;
}

function current(p: XtmdDocProps): DocProps {
  return {
    ir: p.ir,
    state: p.state,
    defaultState: p.defaultState,
    defaultSnapshot: p.defaultSnapshot,
    page: p.page,
    externs: p.externs,
    fns: p.fns,
    widgets: p.widgets,
    components: p.components,
    measure: p.measure,
    onStateChange: p.onStateChange,
    onError: p.onError,
    onCalls: p.onCalls,
  };
}

/**
 * An Extended Mark page. `ir` is its IR. `state` with
 * `onStateChange` controls `$state`, or `defaultState` starts it; every write is checked by xtmd,
 * and what it refuses reaches `onError`. `ref` is given the handle: `snapshot`, `restore`, `set`,
 * `state`, `markdown` and `expose`.
 */
export function XtmdDoc(props: XtmdDocProps): JSX.Element {
  const doc = untrack(() => new Doc(current(props)));
  const [version, setVersion] = createSignal(0);
  onCleanup(doc.subscribe(() => setVersion((v) => v + 1)));
  // reads every prop, so it runs again when any changes
  createEffect(() => doc.update(current(props)));
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
  const root = createMemo(() => {
    version();
    const r = doc.render();
    if (props.id) r.attrs.id = props.id;
    if (props.class) r.attrs.class = `${r.attrs.class} ${props.class}`;
    return r;
  });
  return createComponent(Node, { v: root });
}
