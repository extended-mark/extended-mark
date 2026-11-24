// The planes of a page laid out: what is on each measured, placed by the runtime's layout, and written back,
// pass by pass until nothing changes.

import { Num } from "@extended-mark/runtime/num";
import { lay, type Pt, type Region, type Size } from "@extended-mark/runtime/layout";
import type { Input, Node, Page } from "@extended-mark/runtime";
import { exact, field, flag, inner, kase, nodes, pointInput, range, regionExact } from "./values.js";

/** What whoever draws a page tells its layout: the size of each spot's child, and the width a plane is given
 * when it says none. */
export interface Measure {
  spot(spot: Node): Size;
  planeWidth(plane: Node): Num;
}

const ZERO: Pt = { x: Num.zero(), y: Num.zero() };
const pt = (v: Input | undefined): Pt => {
  const [x, y] = [exact(field(v, "x")), exact(field(v, "y"))];
  return x && y ? { x, y } : ZERO;
};
const regionInput = (r: Region): Input => ({ some: { record: { type: "core/region", fields: { from: pointInput(r.from), to: pointInput(r.to) } } } });

/** The planes laid out already, by page: `fit` only sets where one starts. */
const started = new WeakMap<Page, Set<string>>();

function writesOf(page: Page, plane: Node, measure: Measure): { id: string; input: string; value: Input }[] {
  let seen = started.get(page);
  if (!seen) started.set(page, (seen = new Set()));
  const spots = nodes(plane.inputs.items).filter((n) => n.widget === "core/spot");
  const given = inner(plane.inputs.width);
  const width = exact(given && (page.token(given) ?? given));
  const zoom = exact(plane.inputs.zoom) ?? Num.int(1);
  const center = pt(plane.inputs.center);
  const zooms = range(plane.inputs.zoom_range);
  const at = lay({
    boxes: spots.map((s) => ({ at: pt(s.inputs.at), anchor: kase(s.inputs.anchor) ?? "CENTER", nudge: pt(s.inputs.nudge), size: measure.spot(s) })),
    bounds: regionExact(plane.inputs.bounds),
    width: width ?? measure.planeWidth(plane),
    aspect: exact(plane.inputs.aspect) ?? Num.literal("1.5")!,
    center,
    zoom,
    fit: flag(plane.inputs.fit),
    first: !seen.has(plane.id),
    zoomRange: zooms && { low: exact(zooms.low), high: exact(zooms.high) },
  });
  seen.add(plane.id);
  const out: { id: string; input: string; value: Input }[] = spots.map((s, k) => ({ id: s.id, input: "box", value: regionInput(at.boxes[k]) }));
  if (!at.zoom.equals(zoom)) out.push({ id: plane.id, input: "zoom", value: { num: at.zoom.show() } });
  if (!at.center.x.equals(center.x) || !at.center.y.equals(center.y)) out.push({ id: plane.id, input: "center", value: pointInput(at.center) });
  out.push({ id: plane.id, input: "drawn", value: regionInput(at.drawn) });
  return out;
}

/** Every plane, outermost first. One that failed has nothing to lay out. */
function planesIn(of: readonly Node[], out: Node[] = []): Node[] {
  const walk = (x: Input): void => {
    if ("nodes" in x) planesIn(x.nodes, out);
    else if ("list" in x) x.list.forEach(walk);
    else if ("some" in x) walk(x.some);
    else if ("record" in x) Object.values(x.record.fields ?? {}).forEach(walk);
    else if ("case" in x) Object.values(x.case.values ?? {}).forEach(walk);
  };
  for (const n of of) {
    if (n.widget === "core/plane" && n.failed === undefined) out.push(n);
    Object.values(n.inputs).forEach(walk);
  }
  return out;
}

/** Lays the page's planes out, outermost first. */
export function layoutPlanes(page: Page, measure: Measure): void {
  page.settle(() => {
    const planes = planesIn(page.tree().nodes);
    return { writes: planes.flatMap((p) => writesOf(page, p, measure)), of: planes.map((p) => p.id) };
  });
}
