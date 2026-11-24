// A host's own widgets: core's press replaced, and a pack's badge supplied.
import { h } from "@extended-mark/widgets";

export const widgets = {
  "core/press": (d, ctx) => ({ ...h("button", "mine", { type: "button" }, [`> ${d.inputs.label.text}`]), on: { click: () => ctx.event("click") } }),
  "badge/badge": (d) => h("mark", "badge", {}, [`${d.inputs.label.text}: ${d.inputs.count.num}`]),
};
