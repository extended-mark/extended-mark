// A page running on @extended-mark/runtime: mounted, changed only through checked transactions, and
// written back as Markdown (page 6, plan tab 11). Each page is built by the xtmd binary.

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Page, Num, childrenOf, lay, place, keepInside, around } from "../dist/internal.js";
import { xtmdBinary, workdir, repo as repoRoot } from "./xtmd.mjs";

const dir = workdir();
let n = 0;

/** A document's IR, built by the xtmd binary. */
function page(source, opts = {}) {
  const file = join(dir, `page${n++}.xtmd`);
  writeFileSync(file, source);
  const out = file.replace(/\.xtmd$/, ".ir.json");
  execFileSync(xtmdBinary(), ["compile", file, "--out", out], { stdio: ["ignore", "pipe", "pipe"] });
  return new Page(JSON.parse(readFileSync(out, "utf8")), opts);
}

const num = (s) => ({ num: String(s) });

test("a page mounts: its tree, its $state and its Markdown", () => {
  const p = page('$state {\n    n: int 0..10 = 3;\n}\n@@@\n# Count\n\n@slider(value = $state.n, label = "N");\n');
  const tree = p.observeTree();
  assert.equal(tree.s1.widget, "core/slider");
  assert.deepEqual(tree.s1.props.value, num(3));
  assert.deepEqual(p.observeState(), { n: num(3) });
  assert.equal(p.markdown(), "# Count\n\nN: 3\n");
});

test("a widget's write goes to the caller's variable, checked against its contract", () => {
  const p = page('$state {\n    n: int 0..10 = 3;\n}\n@@@\n@slider(value = $state.n, label = "N");\n');
  p.write("s0", "value", p.value(num(7)));
  assert.deepEqual(p.observeState(), { n: num(7) });
  p.write("s0", "value", p.value(num(12)));
  assert.deepEqual(p.observeState(), { n: num(7) });
  assert.deepEqual(p.takeOutcome().refused, [{ code: "contracts.outside", site: "s0" }]);
  p.write("s0", "value", p.value(num("1.5")));
  assert.deepEqual(p.observeState(), { n: num(7) });
});

test("a host's write to $state is one change, checked once against $state's rules", () => {
  const p = page('$state {\n    low: number = 1;\n    high: number = 5;\n\n    require "low stays at or below high" {\n        return low <= high;\n    }\n}\n@@@\n\\{$state.low} to \\{$state.high}\n');
  p.set({ low: p.value(num(9)) });
  assert.deepEqual(p.takeOutcome().refused, [{ rule: "low stays at or below high" }]);
  assert.deepEqual(p.observeState().low, num(1));
  // both at once is one change, so the rule holds after it
  p.set({ low: p.value(num(9)), high: p.value(num(10)) });
  assert.deepEqual(p.takeOutcome().refused, []);
  assert.equal(p.markdown(), "9 to 10\n");
  // a field that doesn't exist, or a value its type refuses, changes nothing
  p.set({ nope: p.value(num(1)) });
  assert.deepEqual(p.takeOutcome().refused, [{ code: "contracts.outside", site: "$state.nope" }]);
});

test("a handler that fails is undone whole and reported; the page carries on", () => {
  const p = page(
    '$state {\n    tries: int = 0;\n    total: number = 10;\n}\n\nwidget go() {\n    view {\n        @press(label = "Go", click = fn() { $state.tries = $state.tries + 1; $state.total = $state.total / 0; });\n    }\n}\n@@@\n@go;\n',
  );
  p.event("s0/s0", "click");
  assert.deepEqual(p.observeState(), { tries: num(0), total: num(10) });
  assert.deepEqual(p.takeOutcome().failures, [{ code: "failure.number", site: "s0/s0" }]);
});

test("extern channels: the host's fns are reported as calls, its values redraw, and exposed fns run as one change", () => {
  const p = page(
    'extern scores {\n    unit: text = "K";\n    fn saved(total: int);\n}\n\n$state {\n    points: int 0... = 0;\n}\n\nexpose scores {\n    fn clear() {\n        $state.points = 0;\n    }\n}\n\nwidget tally(mut points: int) {\n    view {\n        @press(label = "Add", click = fn() { points = points + 1; scores.saved(points); });\n    }\n    markdown {\n        \\{points} \\{scores.unit}\n    }\n}\n@@@\n@tally(points = $state.points);\n',
  );
  p.event("s0/s0", "click");
  assert.deepEqual(p.takeCalls(), [{ call: { name: "doc/scores.saved", args: [num(1)] } }]);
  assert.equal(p.markdown(), "1 K\n");
  p.setExtern("doc/scores", { unit: p.value({ text: "points" }) });
  assert.equal(p.markdown(), "1 points\n");
  p.expose("doc/scores", "clear");
  assert.deepEqual(p.observeState(), { points: num(0) });
});

test("a loop's rows keep their widgets' variables by item, not by position", () => {
  const p = page(
    'record task {\n    title: text;\n}\n\n$state {\n    tasks: list<task> = [task { title = "a" }, task { title = "b" }];\n}\n\nwidget row(t: task) {\n    mut votes: int = 0;\n    view {\n        @press(label = "\\{t.title}: \\{votes}", click = fn() { votes = votes + 1; });\n    }\n}\n\nwidget board() {\n    view {\n        mut for (t in $state.tasks) {\n            if (mut x: task = t) {\n                @row(t = x);\n                @press(label = "Drop", click = fn() { $state.tasks.remove(x); });\n            }\n        }\n    }\n}\n@@@\n@board;\n',
  );
  p.event("s0/s0[1]/s2/s0", "click");
  assert.deepEqual(p.observeTree()["s0/s0[1]/s2/s0"].props.label, { text: "b: 1" });
  p.event("s0/s0[0]/s3", "click");
  // "b" is now the first row, and keeps its vote
  assert.deepEqual(p.observeTree()["s0/s0[0]/s2/s0"].props.label, { text: "b: 1" });
});

test("a widget that fails while drawing says why in its place, and is tried again when what it reads changes", () => {
  const p = page('$state {\n    parts: int 0... = 0;\n}\n\nwidget share() {\n    view {\n        @label(value = "Each: \\{100 / $state.parts}");\n    }\n}\n@@@\n@share;\n\nAfter.\n');
  assert.deepEqual(p.takeOutcome().failures, [{ code: "failure.number", site: "s0" }]);
  assert.equal(p.observeTree().s0.failed, "a division by zero");
  p.set({ parts: p.value(num(4)) });
  assert.deepEqual(p.observeTree()["s0/s0"].props.value, { text: "Each: 25" });
});

test("the page's values reach the document as core's page channel", () => {
  const p = page('widget where() {\n    view {\n        @label(value = page.width == width.NARROW ? "Narrow" : "Wide");\n    }\n}\n@@@\n@where;\n', { page: { width: "NARROW" } });
  assert.equal(p.markdown(), "Narrow\n");
  p.setPage({ width: "WIDE" });
  assert.equal(p.markdown(), "Wide\n");
});

test("onChange is told after every redraw", () => {
  const p = page('$state {\n    n: int 0..10 = 0;\n}\n@@@\n@slider(value = $state.n, label = "N");\n');
  let told = 0;
  const off = p.onChange(() => told++);
  p.write("s0", "value", p.value(num(1)));
  off();
  p.write("s0", "value", p.value(num(2)));
  assert.equal(told, 1);
});

// ---- the plane's layout

const N = (s) => Num.literal(String(s));
const pt = (x, y) => ({ x: N(x), y: N(y) });
const show = (r) => [r.from.x, r.from.y, r.to.x, r.to.y].map((x) => x.show());

test("a spot's box: its anchor's part of it on the point, moved by its nudge", () => {
  const size = { width: N(40), height: N(20) };
  assert.deepEqual(show(place(pt(0, 0), "CENTER", pt(0, 0), size)), ["-20", "-10", "20", "10"]);
  assert.deepEqual(show(place(pt(0, 0), "SW", pt(0, 0), size)), ["0", "0", "40", "20"]);
  assert.deepEqual(show(place(pt(0, 0), "NE", pt(0, 0), size)), ["-40", "-20", "0", "0"]);
  assert.deepEqual(show(place(pt(0, 0), "N", pt(5, 5), size)), ["-15", "-15", "25", "5"]);
  assert.deepEqual(show(place(pt(10, 10), "E", pt(0, 0), size)), ["-30", "0", "10", "20"]);
});

test("bounds left out are the region around every box; the centre stays inside them", () => {
  const r = around([
    { from: pt(-4, -10), to: pt(4, 10) },
    { from: pt(100, 50), to: pt(124, 70) },
  ]);
  assert.deepEqual(show(r), ["-4", "-10", "124", "70"]);
  const b = { from: pt(-400, -300), to: pt(400, 300) };
  // inside: kept; past an edge: moved back; bounds smaller than the view: their middle
  assert.deepEqual([keepInside(pt(10, 10), pt(100, 50), b)].map((p) => [p.x.show(), p.y.show()]), [["10", "10"]]);
  assert.deepEqual([keepInside(pt(390, -290), pt(100, 50), b)].map((p) => [p.x.show(), p.y.show()]), [["300", "-250"]]);
  assert.deepEqual([keepInside(pt(390, 0), pt(500, 50), b)].map((p) => [p.x.show(), p.y.show()]), [["0", "0"]]);
});

test("lay: boxes by their points, the zoom inside its range, the centre inside the bounds", () => {
  const r = (a, b, c, d) => ({ from: pt(a, b), to: pt(c, d) });
  const of = (p) => lay({ boxes: [], width: N(100), aspect: N(1), center: pt(0, 0), zoom: N(1), fit: false, first: true, ...p });
  const at = (q) => [q.x.show(), q.y.show()].join(",");
  const box = { at: pt(10, 0), anchor: "W", nudge: pt(0, 0), size: { width: N(20), height: N(10) } };
  // bounds left out are the boxes'; smaller than what is drawn, the centre is their middle
  const a = of({ boxes: [box] });
  assert.deepEqual([at(a.boxes[0].from), at(a.boxes[0].to), at(a.center), at(a.drawn.from), at(a.drawn.to)], ["10,-5", "30,5", "20,0", "-30,-50", "70,50"]);
  // fit: the whole of the bounds, the first time only, and the range counts from it
  const bounds = r(0, 0, 400, 200);
  assert.deepEqual([of({ bounds, fit: true }).zoom, of({ bounds, fit: true, first: false }).zoom, of({ bounds, fit: true, first: false, zoomRange: { high: N(2) } }).zoom].map((z) => z.show()), ["0.25", "1", "0.5"]);
  assert.deepEqual([of({ zoom: N(50) }).zoom, of({ zoom: Num.parse("0.01") }).zoom].map((z) => z.show()), ["10", "0.1"]);
});

test("settle: passes run until one changes nothing, each one change; one that never does is a failure, once", () => {
  const p = page("$state {\n    n: int = 0;\n}\n@@@\n@slider(value = $state.n, label = \"N\", range = 0..100);\n");
  const at = p.tree()[0].path;
  const now = () => Number(p.observeState().n.num);
  let passes = 0;
  const pass = (next) => () => (passes++, { writes: [{ path: at, input: "value", value: p.value(num(next(now()))) }], of: [at] });
  p.settle(pass((n) => Math.min(3, n + 1)));
  assert.deepEqual([now(), passes, p.takeOutcome()], [3, 4, { refused: [], failures: [] }]);
  passes = 0;
  p.settle(pass((n) => (n + 1) % 50));
  assert.deepEqual(p.takeOutcome().failures, [{ code: "failure.layout", site: at }]);
  // held until something else changes the page
  const held = passes;
  p.settle(pass((n) => n + 1));
  assert.equal(passes, held);
  p.set({ n: p.value(num(0)) });
  p.settle(pass((n) => Math.min(2, n + 1)));
  assert.equal(now(), 2);
  // nothing to lay out
  p.settle(() => ({ writes: [], of: [] }));
});

test("writing the Markdown leaves the page as it was: a row's events still run afterwards", () => {
  const p = page(
    'record task {\n    title: text;\n}\n\n$state {\n    tasks: list<task> = [task { title = "a" }, task { title = "b" }];\n}\n\nwidget row(t: task) {\n    event drop();\n    view {\n        @press(label = "Drop \\{t.title}", click = fn() { invoke drop(); });\n    }\n    markdown {\n        - \\{t.title}\n    }\n}\n\nwidget board() {\n    view {\n        mut for (t in $state.tasks) {\n            if (mut x: task = t) {\n                @row(t = x, drop = fn() { $state.tasks.remove(x); });\n            }\n        }\n    }\n    markdown {\n        @for (t in $state.tasks) {\n        - \\{t.title}\n        }\n    }\n}\n@@@\n@board;\n',
  );
  assert.equal(p.markdown(), "- a\n- b\n");
  p.event("s0/s0[0]/s2/s0", "click");
  assert.deepEqual(p.takeOutcome().failures, []);
  assert.equal(p.markdown(), "- b\n");
  p.event("s0/s0[0]/s2/s0", "click");
  assert.equal(p.markdown(), "");
});

test("writing the Markdown leaves a widget's events in place: one that passes its event on still does", () => {
  const p = page('$state {\n    n: int = 0;\n}\n\nwidget go() {\n    event pressed();\n    view {\n        @press(label = "Go", click = fn() { invoke pressed(); });\n    }\n    markdown {\n        Go\n    }\n}\n@@@\n@go(pressed = fn() { $state.n = $state.n + 1; });\n\nPressed \\{$state.n} times.\n');
  assert.equal(p.markdown(), "Go\n\nPressed 0 times.\n");
  p.event("s0/s0", "click");
  assert.deepEqual(p.observeState(), { n: num(1) });
});

test("a host's write of the wrong type is refused, whatever its contract", () => {
  const p = page('record task {\n    title: text;\n}\n\n$state {\n    tasks: list<task> = [task { title = "a" }];\n    n: int = 1;\n    note: opt<text> = opt.NONE;\n}\n@@@\n\\{$state.n}\n');
  for (const [k, x] of [["tasks", num(3)], ["n", { text: "3" }], ["tasks", { list: [{ record: { type: "doc/task", fields: { title: num(1) } } }] }], ["note", { some: num(2) }]]) {
    p.set({ [k]: p.value(x) });
    assert.deepEqual(p.takeOutcome().refused, [{ code: "contracts.outside", site: `$state.${k}` }], k);
  }
  p.set({ tasks: p.value({ list: [{ record: { type: "doc/task", fields: { title: { text: "b" } } } }] }), note: p.value({ some: { text: "hi" } }) });
  assert.deepEqual(p.takeOutcome().refused, []);
  assert.equal(p.observeState().tasks.list[0].record.fields.title.text, "b");
});

test("an extern widget the renderer doesn't supply shows its Markdown, and is reported", () => {
  const pack = join(repoRoot, "checks/packs/badge");
  const file = join(dir, "pack-page.xtmd");
  writeFileSync(file, 'import badge;\n@@@\n# Votes\n\n@badge(label = "Votes", count = 2);\n\n@seal(text = "Checked by nobody");\n');
  const out = file.replace(/\.xtmd$/, ".ir.json");
  execFileSync(xtmdBinary(), ["compile", file, "--pack", pack, "--out", out], { stdio: ["ignore", "pipe", "pipe"] });
  const ir = JSON.parse(readFileSync(out, "utf8"));
  assert.deepEqual(ir.ir.needs.packs, [{ externs: ["badge/badge", "badge/seal"], name: "badge", version: 1 }]);
  const p = new Page(ir, { supplies: (w) => w.startsWith("core/") || w === "badge/badge" });
  const tree = p.observeTree();
  assert.equal(tree.s1.widget, "badge/badge");
  // the seal is drawn as its Markdown: a quote holding a paragraph, under its own path
  assert.equal(tree.s2, undefined);
  assert.deepEqual(Object.values(tree).map((n) => n.widget).filter((w) => w !== "core/heading" && w !== "core/words"), ["badge/badge", "core/quote", "core/paragraph"]);
  assert.deepEqual(p.missing(), ["badge/seal"]);
  // its calls show their Markdown, supplied or not
  assert.equal(p.markdown(), "# Votes\n\n**Votes**: 2\n\n> Checked by nobody\n");
  p.setSupplies(() => true);
  assert.deepEqual(p.missing(), []);
  assert.equal(p.observeTree().s2.widget, "badge/seal");
});

test("a spot outside its plane's bounds is a warning for the host when the page runs, once as it moves out", () => {
  const p = page(
    '$state {\n    x: number = 5;\n}\n\nwidget map() {\n    view {\n        @plane(label = "Map", bounds = region { from = point { x = 0, y = 0 }, to = point { x = 10, y = 10 } }) {\n            items {\n                @spot(at = point { x = $state.x, y = 5 }) {\n                    @label(value = "here");\n                }\n            }\n        }\n    }\n}\n@@@\n@map;\n',
  );
  const outside = [{ rule: "a spot is outside the plane's bounds: it is clipped, and the reader can't pan to it", site: "s0/s0" }];
  assert.deepEqual(p.takeWarnings(), []);
  p.set({ x: p.value(num(20)) });
  assert.deepEqual(p.takeWarnings(), outside);
  // still outside: not told again; back inside and out again: told again
  p.set({ x: p.value(num(30)) });
  assert.deepEqual(p.takeWarnings(), []);
  p.set({ x: p.value(num(3)) });
  p.set({ x: p.value(num(-1)) });
  assert.deepEqual(p.takeWarnings(), outside);
  assert.deepEqual(p.takeOutcome().failures, []);
});

test("warn tells the host and refuses nothing: in a widget, in $state and in a record", () => {
  const p = page(`record span {
    low: number;
    high: number;

    warn "the span is wide" {
        return high - low < 10;
    }
}

widget gauge(high: number) {
    warn "the gauge is high" {
        return high < 50;
    }
    view {
        @label(value = "\\{high}");
    }
    markdown {
        \\{high}
    }
}

$state {
    n: number = 1;

    warn "n is big" {
        return $state.n < 100;
    }
}

fn wide(n: number): span {
    return span { low = 0, high = n };
}
@@@
@gauge(high = $state.n);

Wide: \\{wide($state.n).high}
`);
  assert.deepEqual(p.takeWarnings(), []);
  p.set({ n: p.value(num(60)) });
  assert.deepEqual(p.takeWarnings().map((w) => [w.rule, w.site]).sort(), [["the gauge is high", "s0"], ["the span is wide", undefined]]);
  p.set({ n: p.value(num(200)) });
  assert.deepEqual(p.takeWarnings(), [{ rule: "n is big", site: "$state" }]);
  assert.deepEqual(p.observeState(), { n: num(200) });
  assert.deepEqual(p.takeOutcome(), { refused: [], failures: [] });
  assert.equal(p.markdown(), "200\n\nWide: 200\n");
});

test("an extern's require has to hold, and one a pack says is a piece of Markdown is written as it", () => {
  writeFileSync(join(dir, "ext.xtmdpack"), `package ext;

pack ext@1 {
    uses core@2;
    provides said, bars;
}

//@about A quotation of a pack's own.
extern widget said(by: text) {
    slot body: view;

    markdown #quote;
}

//@about Bars.
extern widget bars(points: list<number>) {
    tokens color.C1;

    require "bars have points" {
        return points.len() > 0;
    }

    markdown {
        \\{points.len()} bars
    }
}
`);
  const file = join(dir, "ext.xtmd");
  writeFileSync(file, `import ext;

$state {
    points: list<number> = [1];
}

widget chart() {
    view {
        @bars(points = $state.points);
    }
    markdown {
        @bars(points = $state.points);
    }
}
@@@
@said(by = "Ada") {
    Hello
}

@chart;
`);
  execFileSync(xtmdBinary(), ["compile", file, "--pack", join(dir, "ext.xtmdpack"), "--out", join(dir, "ext.ir.json")], { stdio: ["ignore", "pipe", "pipe"] });
  const p = new Page(JSON.parse(readFileSync(join(dir, "ext.ir.json"), "utf8")), { supplies: () => true });
  assert.equal(p.markdown(), "> Hello\n\n1 bars\n");
  p.set({ points: p.value({ list: [] }) });
  assert.deepEqual(p.takeOutcome().failures, [{ rule: "bars have points", site: "s2" }]);
});

test("a widget of another kind at a key keeps its own variables", () => {
  const p = page(
    '$state {\n    which: bool = true;\n}\n\nwidget one() {\n    mut n: int = 0;\n    view {\n        @press(label = "one \\{n}", click = fn() { n = n + 1; });\n    }\n}\n\nwidget two() {\n    mut n: int = 0;\n    view {\n        @press(label = "two \\{n}", click = fn() { n = n + 1; });\n    }\n}\n\nwidget pile() {\n    view {\n        @press(label = "swap", click = fn() { $state.which = !$state.which; });\n        @stack {\n            if ($state.which) {\n                @one;\n            } else {\n                @two;\n            }\n        }\n    }\n}\n@@@\n@pile;\n',
  );
  const at = "s0/s1/s1[0]/s3/s0";
  p.event(at, "click");
  assert.deepEqual(p.observeTree()[at].props.label, { text: "one 1" });
  p.event("s0/s0", "click");
  assert.deepEqual(p.observeTree()[at].props.label, { text: "two 0" });
  // `one` left the key, and `two` holds it: its count moves, redraw after redraw
  for (const k of [1, 2, 3]) {
    p.event(at, "click");
    assert.deepEqual(p.observeTree()[at].props.label, { text: `two ${k}` });
  }
});

test("a widget in a list slot writes its Markdown with its inputs", () => {
  const p = page(
    'widget tally(label: text) {\n    mut votes: int = 0;\n    view {\n        @press(label = "\\{label}: \\{votes}", click = fn() { votes = votes + 1; });\n    }\n    markdown {\n        \\{label} has \\{votes}\n    }\n}\n\nwidget pile() {\n    view {\n        @stack {\n            @tally(label = "a");\n            @tally(label = "b");\n        }\n    }\n}\n@@@\n@pile;\n',
  );
  // as the compiler writes it, with the values it has now
  assert.equal(p.markdown(), "a has 0\n\nb has 0\n");
  const [press] = Object.entries(p.observeTree()).find(([, n]) => n.props.label?.text === "a: 0");
  // a call of a slot is who its caller wrote, wherever it is placed: in the view, and in the Markdown
  p.event(press, "click");
  assert.equal(p.markdown(), "a has 1\n\nb has 0\n");
  p.event(press, "click");
  assert.deepEqual(p.observeTree()[press].props.label, { text: "a: 2" });
});

// ---- reusing what didn't change (plan tab 2: only the bindings that read what changed)

const MEMO_PAGE =
  'record task {\n    title: text;\n}\n\n$state {\n    n: int = 0;\n    tasks: list<task> = [task { title = "a" }, task { title = "b" }];\n    note: text = "hi";\n}\n\nwidget row(t: task) {\n    mut votes: int = 0;\n    view {\n        @press(label = "\\{t.title}: \\{votes}", click = fn() { votes = votes + 1; });\n    }\n}\n\nwidget board() {\n    view {\n        @label(value = "n is \\{$state.n}");\n        @slider(value = $state.n, label = "N", range = 0..100);\n        mut for (t in $state.tasks) {\n            if (mut x: task = t) {\n                @row(t = x);\n                @press(label = "Drop", click = fn() { $state.tasks.remove(x); });\n            }\n        }\n        @label(value = $state.note);\n    }\n}\n@@@\n# Board\n\n@board;\n';

test("a widget whose reads didn't change is the very same node after a change", () => {
  const p = page(MEMO_PAGE);
  const before = new Map(p.tree().flatMap(function all(d) { return [[d.path, d], ...childrenOf(d).flat().flatMap(all)]; }));
  p.write("s1/s1", "value", p.value(num(5)));
  const after = new Map(p.tree().flatMap(function all(d) { return [[d.path, d], ...childrenOf(d).flat().flatMap(all)]; }));
  // what reads $state.n is drawn anew; the rows and the note are reused as they were
  assert.notEqual(after.get("s1/s0"), before.get("s1/s0"));
  assert.deepEqual(p.observeTree()["s1/s0"].props.value, { text: "n is 5" });
  for (const reused of ["s1/s6", "s1/s2[0]/s4/s0", "s1/s2[1]/s5", "m0"]) {
    assert.ok(before.get(reused), `${reused} is drawn`);
    assert.equal(after.get(reused), before.get(reused), `${reused} is reused`);
  }
});

test("with and without reuse, every step observes the same tree, $state and Markdown", () => {
  const on = page(MEMO_PAGE);
  const off = page(MEMO_PAGE, { memo: false });
  const same = () => {
    assert.deepEqual(on.observeTree(), off.observeTree());
    assert.deepEqual(on.observeState(), off.observeState());
    assert.equal(on.markdown(), off.markdown());
  };
  same();
  const steps = [
    (p) => p.write("s1/s1", "value", p.value(num(7))),
    (p) => p.event("s1/s2[1]/s4/s0", "click"),
    (p) => p.event("s1/s2[1]/s4/s0", "click"),
    (p) => p.event("s1/s2[0]/s5", "click"),
    (p) => p.set({ note: p.value({ text: "there" }) }),
    (p) => p.event("s1/s2[0]/s4/s0", "click"),
    (p) => p.set({ n: p.value(num(-1)) }),
    (p) => p.setPage({ width: "NARROW", theme: "DARK" }),
    (p) => p.write("s1/s1", "value", p.value(num(3))),
  ];
  for (const step of steps) {
    step(on);
    step(off);
    same();
  }
});

test("an undone change leaves no reused node behind", () => {
  const p = page('$state {\n    n: int = 0;\n    total: number = 10;\n}\n\nwidget go() {\n    view {\n        @label(value = "\\{$state.n} of \\{$state.total}");\n        @press(label = "Go", click = fn() { $state.n = $state.n + 1; $state.total = $state.total / 0; });\n    }\n}\n@@@\n@go;\n');
  p.event("s0/s1", "click");
  assert.deepEqual(p.observeTree()["s0/s0"].props.value, { text: "0 of 10" });
  assert.equal(p.takeOutcome().failures.length, 1);
});

const TAKES = "extern scores {\n    best: int;\n    worst: int = 0;\n}\n@@@\n\\{scores.best} \\{scores.worst}\n";

test("a top-level let that holds a widget is placed by @name, each time another", () => {
  const p = page('let limit: int = 3;\nlet abc: label = @label(value = "up to \\{limit}");\nlet tap: view = @press(label = "Add", click = fn() { $state.n = $state.n + 1; });\n\n$state {\n    n: int = 0;\n}\n\nwidget held(shown: view) {\n    view {\n        @shown;\n        @abc;\n    }\n}\n@@@\n@abc;\n\n@held(shown = abc);\n\n@tap;\n\n\\{$state.n}\n');
  assert.equal(p.markdown(), "up to 3\n\nup to 3\n\nup to 3\n\nAdd\n\n0\n");
  const tap = Object.entries(p.observeTree()).find(([, d]) => d.widget === "core/press")[0];
  p.event(tap, "click");
  assert.deepEqual(p.observeState(), { n: { num: "1" } });
  assert.deepEqual(p.takeOutcome(), { refused: [], failures: [] });
});

test("an IR of another version is refused, and told what to do", () => {
  const ir = page("Hi.\n").module;
  assert.throws(() => new Page({ ir: { ...ir, version: 3 } }), /this page is IR 3, this runtime reads IR 4: run xtmd migrate/);
  assert.throws(() => new Page({ ir: { ...ir, version: 5 } }), /this page is IR 5, this runtime reads IR 4: use a later runtime/);
});

test("a page's needs are checked when it loads: core's fns the renderer has, and what the host gives", () => {
  const ir = page("Hi.\n").module;
  assert.throws(() => new Page({ ir: { ...ir, needs: { ...ir.needs, core: [...ir.needs.core, "core/text.backwards"] } } }), /this page needs `core\/text.backwards`, which this runtime doesn't have/);
  assert.deepEqual(page("Hi.\n").lacking(), []);
  assert.deepEqual(page(TAKES, { externs: { "doc/scores": { best: num(3) } } }).lacking(), []);
  // a value the host gives later is lacked until then
  const p = page("extern scores {\n    best: int;\n    worst: int = 0;\n}\n@@@\n@if (scores.worst > 0) {\n    \\{scores.best}\n}\n");
  assert.deepEqual(p.lacking(), ["doc/scores.best"]);
});
