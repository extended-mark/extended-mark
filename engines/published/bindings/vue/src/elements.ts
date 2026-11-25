// A page in Vue's own elements, whichever way it is run: part of the host's tree, rendered on a server as on the client
// and patched by key, so focus and what is being typed survive a change. Listeners are native ones on the elements; the
// hooks of widgets that draw on their element (a canvas, a plane) run as a directive.

import { h, markRaw, onBeforeUnmount, shallowRef, useId, watch, withDirectives, type Directive, type DirectiveArguments, type Component as VueComponent, type PropType, type VNode as VueNode } from "vue";
import { composing, slotsOf, type Child, type Held, type HeldProps, type VNode } from "@extended-mark/widgets";

/** A widget's hook, run as a directive on its element: the latest render's hook each time. */
const hooked: Directive<Element, VNode["hook"]> = {
  mounted: (el, b) => b.value?.mount?.(el),
  updated: (el, b) => b.value?.update?.(el),
  beforeUnmount: (el, b) => b.value?.unmount?.(el),
};

/** A field's value: what the page shows wins, except while a person is composing text in it. */
const setValue = (el: HTMLInputElement, b: { value: unknown }) => {
  if (!composing(el) && el.value !== String(b.value ?? "")) el.value = String(b.value ?? "");
};
const valued: Directive<HTMLInputElement, unknown> = {
  beforeMount: setValue,
  updated: setValue,
  getSSRProps: (b) => ({ value: b.value }),
};

/** One of the widgets' plain elements as Vue's. */
export function toVue(c: Child): VueNode | string {
  if (typeof c === "string") return c;
  const props: Record<string, unknown> = { ...c.attrs };
  if (c.key !== undefined) props.key = c.key;
  if (c.style) props.style = { ...c.style };
  // DOM properties, as Vue sets them on the live element
  for (const [k, x] of Object.entries(c.props ?? {})) if (k !== "value") props[k] = x;
  // native listeners: `onCompositionstart` is the DOM's `compositionstart`, with its currentTarget
  for (const [k, f] of Object.entries(c.on ?? {})) props[`on${k[0].toUpperCase()}${k.slice(1)}`] = f;
  if (c.tag === "style") return h("style", { ...props, innerHTML: c.children.join("") });
  if (c.component) {
    const { of, props: given, children } = c.component;
    // its slots by name: `<slot name="body" />` is the widgets of the input `body`
    const slots = Object.fromEntries(slotsOf(given.node).map((s) => [s, () => children(s).map(toVue)]));
    return h(c.tag, props, [h(of as VueComponent, { ...given }, slots)]);
  }
  const v = h(c.tag, props, c.tag === "textarea" ? undefined : c.children.map(toVue));
  const follow: DirectiveArguments = [];
  if (c.props && "value" in c.props) follow.push([valued, c.props.value]);
  if (c.hook) follow.push([hooked, c.hook]);
  return follow.length ? withDirectives(v, follow) : v;
}

/** The props of a page, whichever way it is run. */
export const heldProps = {
  /** `$state`, controlled. */
  state: Object as PropType<HeldProps["state"]>,
  defaultState: Object as PropType<HeldProps["defaultState"]>,
  defaultSnapshot: Object as PropType<HeldProps["defaultSnapshot"]>,
  page: Object as PropType<HeldProps["page"]>,
  externs: Object as PropType<HeldProps["externs"]>,
  fns: Object as PropType<HeldProps["fns"]>,
  /** Vue components as widgets, by full name: each is given `node`, `event`, `write` and `call`, and its slots by name. */
  components: Object as PropType<Record<string, VueComponent>>,
  onStateChange: Function as PropType<HeldProps["onStateChange"]>,
  onError: Function as PropType<HeldProps["onError"]>,
  onCalls: Function as PropType<HeldProps["onCalls"]>,
};

/** Those props now. */
export function heldNow(p: HeldProps): HeldProps {
  return {
    state: p.state,
    defaultState: p.defaultState,
    defaultSnapshot: p.defaultSnapshot,
    page: p.page,
    externs: p.externs,
    fns: p.fns,
    components: p.components,
    onStateChange: (s) => p.onStateChange?.(s),
    onError: (e) => p.onError?.(e),
    onCalls: (c) => p.onCalls?.(c),
  };
}

/** A held page, followed: what renders it, its props its host's. */
export function useHeld<P extends HeldProps, D extends Held<P>>(make: (props: P) => D, current: () => P): { doc: D; render: () => VueNode | string } {
  // Vue's own id, which is the same on a server as in the browser
  const doc = markRaw(make({ ...current(), uid: `xtmd${useId()}-` })) as D;
  const tick = shallowRef(0);
  const off = doc.subscribe(() => tick.value++);
  watch(current, (next) => doc.update(next), { flush: "post" });
  onBeforeUnmount(off);
  return {
    doc,
    render: () => {
      void tick.value;
      return toVue(doc.render());
    },
  };
}
