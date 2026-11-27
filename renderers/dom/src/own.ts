// Elements made and changed on a page, with the host's own for the widgets it draws itself.

import { Patcher, domHost, h, type Child, type Component, type Host } from "@extended-mark/widgets";
import type { OwnWidget } from "./api.js";

/** A host's own element for a widget, its slots' widgets followed into where it puts them. */
function own(el: Node, c: Component): { update(c: Component): void; destroy(): void } {
  let now = c;
  const slots = new Map<Element, { input: string; patcher: Patcher<Node> }>();
  const follow = () => {
    for (const { input, patcher } of slots.values()) patcher.render(holder(now.children(input)));
  };
  const holder = (kids: Child[]) => ({ ...h("xtmd-slot", undefined, {}, kids), style: { display: "contents" } });
  const made = (c.of as OwnWidget)(el as HTMLElement, c.props, (container, input) => {
    if (!slots.has(container)) slots.set(container, { input, patcher: new Patcher<Node>(dom, container) });
    follow();
  });
  return {
    update(next) {
      now = next;
      made.update(next.props);
      follow();
    },
    destroy() {
      for (const s of slots.values()) s.patcher.destroy();
      made.destroy?.();
    },
  };
}
export const dom: Host<Node> = { ...domHost, component: own };
