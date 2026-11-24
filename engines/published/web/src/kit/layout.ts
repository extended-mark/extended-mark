// A page's planes laid out with sizes read from its elements.

import { Num } from "@extended-mark/runtime/num";
import { layoutPlanes } from "@extended-mark/widgets";
import type { Built } from "./mount.js";

export const layout: NonNullable<Built["layout"]> = (page, element) =>
  layoutPlanes(page, {
    spot(s) {
      const el = element(s.id);
      return { width: Num.int(Math.max(0, el?.offsetWidth ?? 0)), height: Num.int(Math.max(0, el?.offsetHeight ?? 0)) };
    },
    planeWidth(p) {
      const el = element(p.id);
      const view = el?.querySelector<HTMLElement>("[data-part='plane.view']");
      return Num.int(Math.max(0, Math.round((view ?? el)?.clientWidth ?? 0)));
    },
  });
