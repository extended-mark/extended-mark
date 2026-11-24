// Where the boxes on a plane go (page 5, plane): sizes and points in, places out, in the plane's units.
// It knows no page and no widget, so whoever draws planes asks it with what they measured.
//
// At zoom 1 one unit of the plane is one of the page's own lengths, so a box is its size on the page,
// placed at its point: panning and zooming never change it.

import { Num } from "./eval/num.js";

/** A size on the page, in its own lengths (a CSS pixel, an Android dp). */
export interface Size {
  width: Num;
  height: Num;
}

/** A point or a region, in a plane's units. */
export interface Pt {
  x: Num;
  y: Num;
}
export interface Region {
  from: Pt;
  to: Pt;
}

/** A box on a plane, as it was measured. */
export interface Box {
  at: Pt;
  /** Which part of it is on its point: `CENTER`, `N`, `SE`… */
  anchor: string;
  nudge: Pt;
  size: Size;
}

/** A plane and what is on it. */
export interface Plane {
  boxes: readonly Box[];
  /** Left out, the region around every box. */
  bounds?: Region;
  /** Its width on the page, and that over its height. */
  width: Num;
  aspect: Num;
  center: Pt;
  zoom: Num;
  /** `fit` sets where the zoom starts, so only a plane laid out for the first time starts there. */
  fit: boolean;
  first: boolean;
  zoomRange?: { low?: Num; high?: Num };
}

/** Where everything on a plane is. */
export interface Places {
  boxes: Region[];
  center: Pt;
  zoom: Num;
  /** The region it draws. */
  drawn: Region;
}

const HALF = Num.literal("0.5")!;

/** Where a spot's child lands: the anchor's part of it on the point, moved by the nudge. */
export function place(at: Pt, anchor: string, nudge: Pt, size: Size): Region {
  // how much of the width is left of the point, and of the height below it
  const left = anchor.endsWith("W") ? Num.zero() : anchor.endsWith("E") ? size.width : size.width.mul(HALF);
  const below = anchor.startsWith("S") ? Num.zero() : anchor.startsWith("N") ? size.height : size.height.mul(HALF);
  const x = at.x.add(nudge.x).sub(left);
  const y = at.y.add(nudge.y).sub(below);
  return { from: { x, y }, to: { x: x.add(size.width), y: y.add(size.height) } };
}

/** The region around every box. */
export function around(boxes: Region[]): Region | undefined {
  if (!boxes.length) return undefined;
  const min = (a: Num, b: Num) => (a.lt(b) ? a : b);
  const max = (a: Num, b: Num) => (a.gt(b) ? a : b);
  return boxes.reduce((r, b) => ({
    from: { x: min(r.from.x, b.from.x), y: min(r.from.y, b.from.y) },
    to: { x: max(r.to.x, b.to.x), y: max(r.to.y, b.to.y) },
  }));
}

/** A centre kept where the drawn area stays inside the bounds, on each axis; their middle where
 * the bounds are smaller than it. */
export function keepInside(center: Pt, half: Pt, bounds: Region): Pt {
  const axis = (c: Num, h: Num, lo: Num, hi: Num): Num => {
    if (hi.sub(lo).lt(h.add(h))) return lo.add(hi).mul(HALF);
    if (c.sub(h).lt(lo)) return lo.add(h);
    if (c.add(h).gt(hi)) return hi.sub(h);
    return c;
  };
  return { x: axis(center.x, half.x, bounds.from.x, bounds.to.x), y: axis(center.y, half.y, bounds.from.y, bounds.to.y) };
}

const LEAST = Num.literal("0.1")!;
const MOST = Num.int(10);

/** A plane laid out: its boxes, its zoom inside what it allows, its centre kept where the drawn area stays
 * inside the bounds, and the region it draws. */
export function lay(p: Plane): Places {
  const boxes = p.boxes.map((b) => place(b.at, b.anchor, b.nudge, b.size));
  const bounds = p.bounds ?? around(boxes);
  // `fit` starts the zoom where the whole of the bounds fits, and the zoom's range counts from there
  let base = Num.int(1);
  if (p.fit && bounds) {
    const bw = bounds.to.x.sub(bounds.from.x);
    const bh = bounds.to.y.sub(bounds.from.y);
    if (!bw.isZero() && !bh.isZero()) {
      const across = p.width.div(bw);
      const up = p.width.div(p.aspect).div(bh);
      base = across.lt(up) ? across : up;
    }
  }
  let zoom = p.first && p.fit ? base : p.zoom;
  const lo = p.zoomRange?.low?.mul(base);
  const hi = p.zoomRange?.high?.mul(base);
  if (lo && zoom.lt(lo)) zoom = lo;
  if (hi && zoom.gt(hi)) zoom = hi;
  if (zoom.lt(LEAST)) zoom = LEAST;
  if (zoom.gt(MOST)) zoom = MOST;
  // the drawn area, in the plane's units: the widget's size on the page over the zoom
  const half: Pt = { x: p.width.div(zoom).mul(HALF), y: p.width.div(p.aspect).div(zoom).mul(HALF) };
  const center = bounds ? keepInside(p.center, half, bounds) : p.center;
  const drawn: Region = { from: { x: center.x.sub(half.x), y: center.y.sub(half.y) }, to: { x: center.x.add(half.x), y: center.y.add(half.y) } };
  return { boxes, center, zoom, drawn };
}
