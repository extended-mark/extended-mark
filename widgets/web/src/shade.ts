// A shade's pixels, worked out by the page's shader, for everything that paints them on a canvas.

import { Num } from "@extended-mark/runtime/num";
import type { Input, Node as Drawn } from "@extended-mark/runtime";
import { exact, field, inner, num } from "./values.js";

/** The most pixels a shade works out; the canvas is scaled to its size on the page. */
export const MAX_SHADE_PIXELS = 65536;
/** The longest a shade works, in milliseconds: one whose shader is slower is sampled less often. */
const BUDGET = 2000;

/** What a painter needs of the page: the shader called, and a failure told. */
export interface Shader {
  call(args: Input[]): Input;
  fail(f: unknown): void;
}

/**
 * Paints a shade: a coarse pass now, so something shows at once, then every sampled pixel in
 * slices of a few milliseconds, so the page stays responsive.
 */
export function paint(cv: HTMLCanvasElement, el: Element, d: Drawn, wPx: number, hPx: number, shader: Shader, id: number, current: () => number): void {
  const fn = inner(d.inputs.shader);
  const args = d.inputs.args ?? { none: {} };
  const area = d.inputs.area;
  const from = { x: exact(field(field(area, "from"), "x")), y: exact(field(field(area, "from"), "y")) };
  const to = { x: exact(field(field(area, "to"), "x")), y: exact(field(field(area, "to"), "y")) };
  const g = cv.getContext("2d");
  el.setAttribute("data-state", "painting");
  if (!fn || !("fn" in fn) || !g || !from.x || !from.y || !to.x || !to.y || wPx <= 0 || hPx <= 0) {
    el.setAttribute("data-state", fn && "fn" in fn ? "empty" : "no-shader");
    return;
  }
  const [lx, hx] = from.x.lt(to.x) ? [from.x, to.x] : [to.x, from.x];
  const [ly, hy] = from.y.lt(to.y) ? [from.y, to.y] : [to.y, from.y];
  /** One pass of cols × rows samples: each one's colour, from the top-left, row by row. */
  const pass = (cols: number, rows: number) => {
    const px = hx.sub(lx).div(Num.int(cols));
    const py = hy.sub(ly).div(Num.int(rows));
    const half = Num.parse("0.5")!;
    const xs = Array.from({ length: cols }, (_, i): Input => ({ num: lx.add(px.mul(Num.int(i).add(half))).show() }));
    const ys = Array.from({ length: rows }, (_, j): Input => ({ num: hy.sub(py.mul(Num.int(j).add(half))).show() }));
    const size: Input = { num: px.show() };
    const data = new ImageData(cols, rows);
    let j = 0;
    const row = () => {
      for (let i = 0; i < cols; i++) {
        const c = shader.call([{ record: { type: "core/point", fields: { x: xs[i], y: ys[j] } } }, size, args]);
        const k = (j * cols + i) * 4;
        const ch = (n: string, or: number) => Math.round(255 * Math.min(1, Math.max(0, num(field(c, n)) ?? or)));
        data.data[k] = ch("r", 0);
        data.data[k + 1] = ch("g", 0);
        data.data[k + 2] = ch("b", 0);
        data.data[k + 3] = ch("a", 1);
      }
      j++;
    };
    return { data, row, done: () => j >= rows };
  };
  const show = (data: ImageData) => {
    cv.width = data.width;
    cv.height = data.height;
    g.putImageData(data, 0, 0);
  };
  const fail = (f: unknown) => {
    shader.fail(f);
    el.setAttribute("data-state", "failed");
  };
  // the coarse pass, and how long one sample takes
  const dpr = typeof devicePixelRatio === "number" ? Math.min(2, devicePixelRatio) : 1;
  const scale = Math.min(dpr, Math.sqrt(MAX_SHADE_PIXELS / (wPx * hPx)));
  let cols = Math.max(1, Math.round(wPx * scale));
  let rows = Math.max(1, Math.round(hPx * scale));
  const c0 = Math.min(cols, 16);
  const r0 = Math.min(rows, 16);
  const since = performance.now();
  try {
    const coarse = pass(c0, r0);
    while (!coarse.done()) coarse.row();
    show(coarse.data);
  } catch (f) {
    fail(f);
    return;
  }
  const budget = Math.max(c0 * r0, (BUDGET / Math.max(0.1, performance.now() - since)) * c0 * r0);
  if (cols * rows > budget) {
    const k = Math.sqrt(budget / (cols * rows));
    cols = Math.max(1, Math.floor(cols * k));
    rows = Math.max(1, Math.floor(rows * k));
  }
  el.setAttribute("data-samples", `${cols}x${rows}`);
  if (cols <= c0 && rows <= r0) {
    el.setAttribute("data-state", "done");
    return;
  }
  const full = pass(cols, rows);
  const slice = () => {
    if (current() !== id) return;
    const until = performance.now() + 12;
    try {
      while (!full.done() && performance.now() < until) full.row();
    } catch (f) {
      fail(f);
      return;
    }
    if (full.done()) {
      show(full.data);
      el.setAttribute("data-state", "done");
    } else setTimeout(slice, 0);
  };
  setTimeout(slice, 0);
}
