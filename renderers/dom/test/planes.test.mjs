// The planes laid out by the web's widgets, with fixed sizes and no browser: the runtime's cases that have
// planes, and what a layout writes.

import { test } from "node:test";
import assert from "node:assert/strict";
import { Num, Page, everyNode } from "@extended-mark/runtime";
import { layoutPlanes } from "@extended-mark/widgets";
import { cases, run } from "../../../runtime/typescript/test/run-case.mjs";
import { measure } from "../../../runtime/typescript/test/sizes.mjs";
import { ir } from "./browser.mjs";

for (const c of cases.filter((c) => c.planes)) test(`${c.id}: ${c.about}`, () => run(c, (host, core) => layoutPlanes(host, measure(core))));

const num = (n) => ({ num: String(n) });
const region = (x0, y0, x1, y1) => ({ record: { type: "core/region", fields: { from: { record: { type: "core/point", fields: { x: num(x0), y: num(y0) } } }, to: { record: { type: "core/point", fields: { x: num(x1), y: num(y1) } } } } } });
const sized = (w, h, across = 100) => ({ spot: () => ({ width: Num.int(w), height: Num.int(h) }), planeWidth: () => Num.int(across) });
const MAP = '$state {\n    home: opt<region> = opt.NONE;\n}\n\nwidget map() {\n    view {\n        @plane(label = "Map", bounds = region { from = point { x = -100, y = -100 }, to = point { x = 100, y = 100 } }) {\n            items {\n                @spot(at = point { x = 0, y = 0 }, box = $state.home) {\n                    @label(value = "Home");\n                }\n            }\n        }\n    }\n}\n@@@\n@map;\n';
const page = (text) => new Page(JSON.parse(ir(text)), { supplies: () => true });

test("a layout writes boxes once: a second pass with nothing changed writes nothing", () => {
  const p = page(MAP);
  let redraws = 0;
  p.onChange(() => redraws++);
  layoutPlanes(p, sized(32, 20));
  assert.equal(redraws, 1);
  assert.deepEqual(p.state().home, { some: region(-16, -10, 16, 10) });
  layoutPlanes(p, sized(32, 20));
  assert.equal(redraws, 1);
  assert.deepEqual(p.takeOutcome(), { refused: [], failures: [] });
});

test("the layout is measured by node, and its writes are changes", () => {
  const p = page(MAP);
  let last = [];
  p.onChange((c) => (last = c));
  const seen = [];
  layoutPlanes(p, { spot: (s) => (seen.push(s.widget), { width: Num.int(2), height: Num.int(2) }), planeWidth: () => Num.int(100) });
  assert.ok(seen.length && seen.every((w) => w === "core/spot"));
  const [plane] = everyNode(p.tree().nodes).filter((n) => n.widget === "core/plane");
  assert.ok(plane.inputs.drawn.some.record.fields.from && last.some((c) => c.id === plane.id));
});

test("a plane with a width of its own takes it, and one with none the width it is given", () => {
  const right = (text) => {
    const p = page(text);
    layoutPlanes(p, sized(2, 2, 10));
    const [plane] = everyNode(p.tree().nodes).filter((n) => n.widget === "core/plane");
    return Number(plane.inputs.drawn.some.record.fields.to.record.fields.x.num);
  };
  assert.equal(right(MAP), 5);
  assert.ok(right(MAP.replace('label = "Map"', 'label = "Map", width = size.S')) > 5);
});
