// A page's planes laid out with sizes read from its elements.

import { layoutPlanes, measured } from "@extended-mark/widgets";
import type { Built } from "./mount.js";

export const layout: NonNullable<Built["layout"]> = (page, element) => layoutPlanes(page, measured(element));
