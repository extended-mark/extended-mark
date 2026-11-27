// core.gfx in SVG, as elements: a canvas or a plane gives its shapes a space (its units, y up, onto
// SVG's coordinates), layers are groups, and shapes are SVG elements in it, as @extended-mark/dom draws them
// (the same parts and classes, so the same stylesheet). `place` puts ordinary widgets at a point;
// `handle` is dragged; `shade` paints a 2D canvas from a shader, per sampled pixel, in its hook.
// A surface draws its slots inside its space, so the shapes read it as they are drawn; where the
// space needs the surface on the screen (a pointer in units) it is measured in the listener.

import type { Input, Node as Drawn } from "@extended-mark/runtime";
import { exact, field, flag, kase, list, nodes, num, placesFor, point, pointInput, range, region, sig, str, toNum, type XY } from "../values.js";
import { paint } from "../shade.js";
import type { ViewContext, ViewWidget } from "../draw.js";
import type { Act } from "../act.js";
import { attr, cls, css, h, on, s, type VNode } from "../vnode.js";
import { claim, idOf, isClaimed } from "./controls.js";

/** A drawing surface's coordinates: its units onto SVG user units, and the screen back to units.
 * `from` is an element drawn in this space (the event's): the space finds its own surface from it
 * on the screen, since a render has no elements to measure. */
export interface Space {
  x(u: number): number;
  y(u: number): number;
  /** A length along x (a circle's radius). */
  len(u: number): number;
  /** Where a pointer is, in units. */
  unit(clientX: number, clientY: number, from: Element): XY | undefined;
  /** How many units one CSS pixel on the screen is now, for rounding a write. */
  unitsPerPx(from: Element): number;
  /** Keeps a point inside what the surface shows, for a handle. */
  keep(p: XY): XY;
}

/** A space for shapes drawn outside any canvas: units are pixels, y down. */
const loose: Space = { x: (u) => u, y: (u) => -u, len: (u) => u, unit: () => undefined, unitsPerPx: () => 1, keep: (p) => p };

const spaces: Space[] = [];

/** Draws a surface's slots inside its space: the shapes in them read it as they are drawn. */
export function inSpace<T>(space: Space, draw: () => T): T {
  spaces.push(space);
  try {
    return draw();
  } finally {
    spaces.pop();
  }
}

function currentSpace(): Space {
  return spaces[spaces.length - 1] ?? loose;
}

/** Whether a shape has the pointer or the keyboard's focus now, as it last told the page. */
const inside = new WeakMap<Element, boolean>();

/** A shape's events: clicks, and the pointer or the focus entering and leaving. Only events that
 * do something make a shape interactive. */
function shapeEvents(el: VNode, d: Drawn, ctx: ViewContext, name: string): VNode {
  const has = (e: string) => !!d.events?.includes(e);
  const click = has("click");
  const hover = has("enter") || has("leave");
  const live = click || hover;
  const enter = (me: Element, now: boolean) => {
    if (!hover || now === (inside.get(me) ?? false)) return;
    inside.set(me, now);
    ctx.event(now ? "enter" : "leave");
  };
  on(el, "click", (e) => {
    if (!click) return;
    claim(e);
    ctx.event("click");
  });
  on(el, "keydown", (e) => {
    const key = (e as KeyboardEvent).key;
    if (!click || (key !== "Enter" && key !== " ")) return;
    e.preventDefault();
    ctx.event("click");
  });
  // inside while the pointer is over it, or while the keyboard's focus is on it
  on(el, "pointerenter", (e) => enter(e.currentTarget as Element, true));
  on(el, "pointerleave", (e) => {
    const me = e.currentTarget as Element;
    enter(me, me.matches(":focus-visible"));
  });
  on(el, "focus", (e) => {
    const me = e.currentTarget as Element;
    if (me.matches(":focus-visible")) enter(me, true);
  });
  on(el, "blur", (e) => {
    const me = e.currentTarget as Element;
    enter(me, me.matches(":hover"));
  });
  attr(el, "role", click ? "button" : name ? "img" : undefined);
  attr(el, "tabindex", live ? "0" : undefined);
  attr(el, "aria-label", name || undefined);
  attr(el, "aria-hidden", name || live ? undefined : "true");
  return cls(el, "xtmd-live", live);
}

/** A stroke's width in pixels: a stroke token, or stroke.PX's number. */
function strokeWidth(v: Input | undefined, ctx: ViewContext): number {
  return num(ctx.tokens.value(v)) ?? 1;
}

/** An opacity as its attribute: left off at 1. */
function opacity(el: VNode, d: Drawn): VNode {
  const o = num(d.inputs.opacity) ?? 1;
  return attr(el, "opacity", o === 1 ? undefined : String(o));
}

// ---- canvas

const SURFACE = "[data-part='canvas.surface']";

export const canvas: ViewWidget = (d, ctx) => {
  const ends = (v: Input | undefined) => {
    const r = range(v);
    return { lo: num(r?.low) ?? -1, hi: num(r?.high) ?? 1 };
  };
  const xr = ends(d.inputs.x);
  const yr = ends(d.inputs.y);
  const w = ctx.tokens.number(d.inputs.width) ?? 320;
  const aspect = num(d.inputs.aspect) ?? (xr.hi - xr.lo) / (yr.hi - yr.lo || 1);
  const ht = w / (aspect > 0 ? aspect : 1);
  /** The canvas's SVG on the screen, from anything drawn in it. */
  const surface = (from: Element): Element => from.closest(SURFACE) ?? from;
  const space: Space = {
    x: (u) => ((u - xr.lo) / (xr.hi - xr.lo || 1)) * w,
    y: (u) => ((yr.hi - u) / (yr.hi - yr.lo || 1)) * ht,
    len: (u) => (u / (xr.hi - xr.lo || 1)) * w,
    unit(cx, cy, from) {
      const r = surface(from).getBoundingClientRect();
      if (!r.width || !r.height) return undefined;
      return { x: xr.lo + ((cx - r.left) / r.width) * (xr.hi - xr.lo), y: yr.hi - ((cy - r.top) / r.height) * (yr.hi - yr.lo) };
    },
    unitsPerPx(from) {
      const r = surface(from).getBoundingClientRect();
      return (xr.hi - xr.lo) / (r.width || w);
    },
    keep: (p) => ({ x: Math.min(xr.hi, Math.max(xr.lo, p.x)), y: Math.min(yr.hi, Math.max(yr.lo, p.y)) }),
  };
  const listens = !!d.events?.includes("click");
  const layers = inSpace(space, () => ctx.children(nodes(d.inputs.layers)));
  const svg = s("svg", "canvas.surface", { preserveAspectRatio: "none", viewBox: `0 0 ${w} ${ht}` }, layers);
  on(svg, "click", (e) => {
    if (!listens || isClaimed(e)) return;
    const me = e as MouseEvent;
    const from = e.currentTarget as Element;
    const at = space.unit(me.clientX, me.clientY, from);
    if (!at) return;
    const p = placesFor(space.unitsPerPx(from));
    ctx.event("click", [pointInput({ x: toNum(at.x, p), y: toNum(at.y, p) })]);
  });
  const el = h("figure", "canvas", { role: "group", "aria-label": str(d.inputs.label) || undefined }, [svg]);
  css(el, "width", `${w}px`);
  css(el, "aspect-ratio", `${w} / ${ht}`);
  return cls(el, "xtmd-clickable", listens);
};

// ---- layer

export const layer: ViewWidget = (d, ctx) => {
  const space = currentSpace();
  const id = idOf(ctx.uid, "clip", d);
  const rect = s("rect");
  const r = region(d.inputs.clip);
  if (r) {
    const [x0, x1] = [space.x(r.from.x), space.x(r.to.x)].sort((a, b) => a - b);
    const [y0, y1] = [space.y(r.from.y), space.y(r.to.y)].sort((a, b) => a - b);
    for (const [k, v] of [["x", x0], ["y", y0], ["width", x1 - x0], ["height", y1 - y0]] as const) attr(rect, k, String(v));
  }
  const shapes = s("g", "layer.shapes", { "clip-path": r ? `url(#${id})` : undefined }, ctx.children(nodes(d.inputs.shapes)));
  return opacity(s("g", "layer", {}, [s("clipPath", undefined, { id }, [rect]), shapes]), d);
};

// ---- circle and path

export const circle: ViewWidget = (d, ctx) => {
  const space = currentSpace();
  const t = ctx.tokens;
  const at = point(d.inputs.at) ?? { x: 0, y: 0 };
  const el = s("circle", "circle", {
    cx: String(space.x(at.x)),
    cy: String(space.y(at.y)),
    r: String(Math.abs(space.len(num(d.inputs.r) ?? 0))),
  });
  css(el, "fill", t.color(d.inputs.fill) ?? "none");
  css(el, "stroke", t.color(d.inputs.stroke));
  css(el, "stroke-width", `${strokeWidth(d.inputs.width, ctx)}px`);
  opacity(el, d);
  return shapeEvents(el, d, ctx, str(d.inputs.label));
};

export const path: ViewWidget = (d, ctx) => {
  const space = currentSpace();
  const t = ctx.tokens;
  const pts = list(d.inputs.points)
    .map(point)
    .filter((p): p is XY => !!p)
    .map((p) => ({ x: space.x(p.x), y: space.y(p.y) }));
  const closed = flag(d.inputs.closed);
  const line = s("path", "path.line", { d: pts.map((p, k) => `${k ? "L" : "M"}${p.x} ${p.y}`).join("") + (closed ? "Z" : "") });
  const color = t.color(d.inputs.stroke);
  const w = strokeWidth(d.inputs.width, ctx);
  css(line, "fill", closed ? (t.color(d.inputs.fill) ?? "none") : "none");
  css(line, "stroke", color ?? "none");
  css(line, "stroke-width", `${w}px`);
  css(line, "stroke-dasharray", flag(d.inputs.dash) ? `${w * 4} ${w * 3}` : undefined);
  // the head at the last point, along the last segment, in the stroke's colour
  const kind = kase(d.inputs.head) ?? "NONE";
  const [a, b] = [pts[pts.length - 2], pts[pts.length - 1]];
  let hd = "";
  if (a && b && kind !== "NONE") {
    const len = Math.hypot(b.x - a.x, b.y - a.y) || 1;
    const [ux, uy] = [(b.x - a.x) / len, (b.y - a.y) / len];
    const size = 4 + w * 3;
    if (kind === "ARROW") {
      const back = { x: b.x - ux * size, y: b.y - uy * size };
      hd = `M${b.x} ${b.y}L${back.x - uy * size * 0.5} ${back.y + ux * size * 0.5}L${back.x + uy * size * 0.5} ${back.y - ux * size * 0.5}Z`;
    } else {
      const r = size * 0.4;
      hd = `M${b.x - r} ${b.y}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0Z`;
    }
  }
  const head = s("path", "path.head", { d: hd || undefined });
  css(head, "fill", hd ? (color ?? "none") : undefined);
  const el = opacity(s("g", "path", {}, [line, head]), d);
  return shapeEvents(el, d, ctx, str(d.inputs.label));
};

/** A shape acted on by the checks: a click is a real click; entering and leaving are the pointer's. */
const shapeAct: Act = (el, a) => {
  if (!("event" in a)) return false;
  if (a.event === "click") el.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  else if (a.event === "enter" || a.event === "leave") el.dispatchEvent(new PointerEvent(a.event === "enter" ? "pointerenter" : "pointerleave"));
  else return false;
  return true;
};

// ---- place: ordinary widgets at a point

/** How far an anchor moves a box from its point, as CSS translate percentages. */
export function anchorShift(anchor: string): string {
  const x = anchor.endsWith("W") ? "0%" : anchor.endsWith("E") ? "-100%" : "-50%";
  const y = anchor.startsWith("N") ? "0%" : anchor.startsWith("S") ? "-100%" : "-50%";
  return `translate(${x}, ${y})`;
}

export const place: ViewWidget = (d, ctx) => {
  const space = currentSpace();
  const at = point(d.inputs.at) ?? { x: 0, y: 0 };
  const nudge = point(d.inputs.nudge) ?? { x: 0, y: 0 };
  const name = str(d.inputs.label);
  // the foreignObject sits on the point; its box hangs from it by the anchor, nudged in pixels, y up
  const box = h("div", "place.box", { role: name ? "group" : undefined, "aria-label": name || undefined }, ctx.children(nodes(d.inputs.child)));
  css(box, "transform", anchorShift(kase(d.inputs.anchor) ?? "CENTER"));
  const el = s("foreignObject", "place", { width: "1", height: "1", x: String(space.x(at.x) + nudge.x), y: String(space.y(at.y) - nudge.y) }, [box]);
  return opacity(el, d);
};

// ---- handle

/** The pointer dragging a handle now, by the handle's element. */
const drags = new WeakMap<Element, number>();

export const handle: ViewWidget = (d, ctx) => {
  const space = currentSpace();
  const at = point(d.inputs.at) ?? { x: 0, y: 0 };
  const write = (p: XY, from: Element) => {
    const k = space.keep(p);
    const places = placesFor(space.unitsPerPx(from));
    ctx.write("at", pointInput({ x: toNum(k.x, places), y: toNum(k.y, places) }));
  };
  const dot = s("circle", "handle.dot", { r: "7", cx: String(space.x(at.x)), cy: String(space.y(at.y)) });
  css(dot, "fill", ctx.tokens.color(d.inputs.tone) ?? "var(--xtmd-color-accent)");
  const shown = `(${exact(field(d.inputs.at, "x"))?.show() ?? 0}, ${exact(field(d.inputs.at, "y"))?.show() ?? 0})`;
  const el = s("g", "handle", { tabindex: "0", role: "slider", "aria-roledescription": "handle" }, [
    s("circle", "handle.hit", { r: "14", cx: String(space.x(at.x)), cy: String(space.y(at.y)) }),
    dot,
  ]);
  opacity(el, d);
  attr(el, "aria-label", str(d.inputs.label) || undefined);
  attr(el, "aria-valuetext", shown);
  on(el, "pointerdown", (e) => {
    const pe = e as PointerEvent;
    if (pe.button !== 0) return;
    // the handle's drag, not the plane's pan around it
    e.stopPropagation();
    e.preventDefault();
    const me = e.currentTarget as SVGGElement;
    drags.set(me, pe.pointerId);
    me.setPointerCapture?.(pe.pointerId);
    me.focus({ preventScroll: true });
  });
  on(el, "pointermove", (e) => {
    const pe = e as PointerEvent;
    const me = e.currentTarget as Element;
    if (drags.get(me) !== pe.pointerId) return;
    const p = space.unit(pe.clientX, pe.clientY, me);
    if (p) write(p, me);
  });
  const stop = (e: Event) => {
    const me = e.currentTarget as Element;
    if (drags.get(me) === (e as PointerEvent).pointerId) drags.delete(me);
  };
  on(el, "pointerup", stop);
  on(el, "pointercancel", stop);
  on(el, "click", claim);
  on(el, "keydown", (e) => {
    const ke = e as KeyboardEvent;
    const me = e.currentTarget as Element;
    const step = space.unitsPerPx(me) * (ke.shiftKey ? 40 : 8);
    const move: Record<string, XY> = { ArrowLeft: { x: -step, y: 0 }, ArrowRight: { x: step, y: 0 }, ArrowUp: { x: 0, y: step }, ArrowDown: { x: 0, y: -step } };
    const m = move[ke.key];
    if (!m) return;
    e.preventDefault();
    write({ x: at.x + m.x, y: at.y + m.y }, me);
  });
  return el;
};

// ---- shade: pixels from a shader

/** A shade's paint, by its element: what it last painted, and which paint is the current one. */
const paints = new WeakMap<Element, { was: string; job: number }>();

export const shade: ViewWidget = (d, ctx) => {
  const space = currentSpace();
  const el = s("foreignObject", "shade", { role: "img", "aria-label": str(d.inputs.label) || undefined }, [h("canvas", "shade.pixels")]);
  opacity(el, d);
  const area = d.inputs.area;
  const r = region(area);
  let run: ((root: Element) => void) | undefined;
  if (r) {
    const [x0, x1] = [space.x(r.from.x), space.x(r.to.x)].sort((a, b) => a - b);
    const [y0, y1] = [space.y(r.from.y), space.y(r.to.y)].sort((a, b) => a - b);
    attr(el, "x", String(x0));
    attr(el, "y", String(y0));
    attr(el, "width", String(Math.max(0, x1 - x0)));
    attr(el, "height", String(Math.max(0, y1 - y0)));
    const now = [sig(area), sig(d.inputs.shader), sig(d.inputs.args), x1 - x0, y1 - y0].join("|");
    run = (root) => {
      let st = paints.get(root);
      if (!st) paints.set(root, (st = { was: "", job: 0 }));
      if (now === st.was) return;
      const cv = root.querySelector<HTMLCanvasElement>(":scope > [data-part='shade.pixels']");
      if (!cv) return;
      st.was = now;
      const mine = st;
      paint(cv, root, d, x1 - x0, y1 - y0, { call: (a) => ctx.call("shader", a), fail: (f) => ctx.fail(f) }, ++mine.job, () => mine.job);
    };
  }
  el.hook = {
    mount: (root) => run?.(root),
    update: (root) => run?.(root),
    unmount(root) {
      const st = paints.get(root);
      if (!st) return;
      // the paint in flight stops; the same element mounted again paints afresh
      st.job++;
      st.was = "";
    },
  };
  return el;
};

// ---- acting through the element, for the checks

/** Each shape acting as a reader would: clicking, and the pointer entering and leaving. */
export const acts: Record<string, Act> = {
  "core/circle": shapeAct,
  "core/path": shapeAct,
  // a drag lands on whole pixels, so a check's exact point goes through the page
  "core/handle": () => false,
};
