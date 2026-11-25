// What comes to a page from outside is one of its type, or is refused and nothing changes: a host's writes
// and calls, its extern values, what a widget writes and the arguments of its events (page 6). The changes
// are checks/pages/outside.json's, which the Rust runtime is given too.

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Page, everyNode } from "../dist/index.js";
import { fits } from "./fits.mjs";
import { xtmdBinary, workdir, repo } from "./xtmd.mjs";

const pages = join(repo, "checks/pages");
const out = join(workdir(), "outside.ir.json");
execFileSync(xtmdBinary(), ["build", join(pages, "outside.xtmd"), "--pack", join(pages, "pick"), "--out", out], { stdio: ["ignore", "pipe", "pipe"] });
const ir = JSON.parse(readFileSync(out, "utf8"));
const { changes } = JSON.parse(readFileSync(join(pages, "outside.json"), "utf8"));

const holds = (p) => ({ ...p.state(), ...Object.fromEntries(Object.entries(p.given()).map(([k, v]) => [k.replace("doc/", ""), v])), "s3.open": p.node("s3").inputs.open });

function change(p, d) {
  if (d.set) p.set(d.set);
  if (d.extern) p.setExtern("doc/host", d.extern);
  if (d.expose) p.expose("doc/host", d.expose.name, d.expose.args);
  if (d.write) p.write(d.write.id, d.write.input, d.write.value);
  if (d.event) p.event(d.event.id, d.event.name, d.event.args);
}

/** The same changes, as actions. */
const actions = (d) => [
  ...(d.set ? [{ set: { fields: d.set } }] : []),
  ...(d.extern ? [{ extern: { channel: "doc/host", values: d.extern } }] : []),
  ...(d.expose ? [{ expose: { channel: "doc/host", name: d.expose.name, args: d.expose.args } }] : []),
  ...(d.write ? [{ write: d.write }] : []),
  ...(d.event ? [{ event: d.event }] : []),
];

test("each change is the same as an action, and what it led to is told once", () => {
  for (const c of changes) {
    const p = new Page(ir);
    const before = holds(p);
    for (const a of actions(c.do)) {
      // what comes wrong is an action all the same: the page says no
      assert.equal(fits(a, "actions", "action"), undefined, c.name);
      p.act(a);
    }
    const told = p.told();
    assert.equal(fits(told, "told", "told"), undefined, c.name);
    assert.deepEqual(told, { ...(c.refused ? { refused: c.refused } : {}), ...(c.failures ? { failures: c.failures } : {}) }, c.name);
    assert.deepEqual(p.told(), {}, c.name);
    assert.deepEqual(holds(p), { ...before, ...c.state }, c.name);
  }
});

test("the page's values and its data are actions too", () => {
  const p = new Page(ir);
  const data = p.data();
  p.act({ set: { fields: { n: { num: "7" } } } });
  p.act({ page: { values: { theme: { case: { enum: "core/theme", name: "DARK" } } } } });
  assert.equal(p.given()["core/page.theme"].case.name, "DARK");
  p.act({ restore: { data } });
  assert.deepEqual(p.data(), data);
  assert.throws(() => p.act({ erase: {} }), /is no action/);
});

for (const c of changes) {
  test(`from outside: ${c.name}`, () => {
    const p = new Page(ir);
    const before = holds(p);
    change(p, c.do);
    assert.deepEqual(p.takeOutcome(), { refused: c.refused ?? [], failures: c.failures ?? [] });
    assert.deepEqual(holds(p), { ...before, ...c.state });
  });
}

test("what a page starts from is checked as what comes later is", () => {
  const p = new Page(ir, { externs: { "doc/host": { n: { text: "x" }, s: { text: "b" } } }, state: { n: { text: "x" }, t: { text: "b" } } });
  assert.deepEqual(p.takeOutcome().refused.map((b) => b.site).sort(), ["$state.n", "doc/host.n"]);
  assert.deepEqual([p.given()["doc/host.n"], p.given()["doc/host.s"], p.state().n, p.state().t], [{ num: "3" }, { text: "b" }, { num: "3" }, { text: "b" }]);
});

test("a fn the engine supplies gives a value of its type", () => {
  const given = (v) => new Page(ir, { fns: { "pick/dial": () => v } });
  assert.deepEqual(given({ num: "4" }).takeOutcome().failures, []);
  for (const v of [{ num: "99" }, { text: "x" }]) assert.deepEqual(given(v).takeOutcome().failures.map((b) => b.code), ["failure.rule"], JSON.stringify(v));
});

function page(text) {
  const file = join(workdir(), "outside-own.xtmd");
  writeFileSync(file, text);
  execFileSync(xtmdBinary(), ["build", file, "--out", `${file}.json`], { stdio: ["ignore", "pipe", "pipe"] });
  return new Page(JSON.parse(readFileSync(`${file}.json`, "utf8")));
}

test("a failure in the document's own code stays where it is", () => {
  const p = page("$state {\n    n: int = 1;\n}\n@@@\nOne \\{10 / $state.n} two.\n\nThe rest.\n\n@slider(value = $state.n, label = \"N\");\n");
  p.set({ n: { num: "0" } });
  assert.deepEqual(p.state(), { n: { num: "0" } });
  assert.deepEqual(p.takeOutcome(), { refused: [], failures: [{ code: "failure.number", site: "m0" }] });
  assert.equal(p.node("m0").failed, "a division by zero");
  assert.equal(p.markdown(), "> paragraph failed: a division by zero\n\nThe rest.\n\nN: 0\n");
  p.write("s2", "value", { num: "2" });
  assert.equal(p.markdown(), "One 5 two.\n\nThe rest.\n\nN: 2\n");
});

test("a branch, a loop or a local of the document's that fails is a notice, and the rest is drawn", () => {
  const p = page("$state {\n    n: int = 1;\n}\n@@@\n@if (10 / $state.n > 1) {\n    Big.\n}\n\nThe rest.\n");
  p.set({ n: { num: "0" } });
  assert.deepEqual(p.tree().nodes.map((x) => x.failed), ["a division by zero", undefined]);
  assert.equal(p.markdown(), "> doc failed: a division by zero\n\nThe rest.\n");
});

test("a rule of $state's that can't be worked out refuses the write that made it so", () => {
  const p = page("$state {\n    n: number = 1;\n\n    require \"a tenth at most\" {\n        return 1 / n <= 10;\n    }\n}\n@@@\n\\{$state.n}\n");
  p.set({ n: { num: "0" } });
  assert.deepEqual(p.takeOutcome().refused, [{ code: "failure.number", site: "$state" }]);
  assert.deepEqual(p.state(), { n: { num: "1" } });
});

const TASKS = "record task {\n    mut title: text;\n}\n\n$state {\n    xs: list<task> = [task { title = \"a\" }, task { title = \"b\" }];\n}\n\nwidget row(x: task) {\n    mut votes: int = 0;\n    view {\n        @press(label = \"\\{x.title}: \\{votes}\", click = fn() { votes = votes + 1; });\n    }\n    markdown { }\n}\n\nwidget board() {\n    view {\n        @press(label = \"Add\", click = fn() { $state.xs.add(task { title = \"new\" }); });\n        mut for (t in $state.xs) {\n            if (mut x: task = t) {\n                @row(x = x);\n                @press(label = \"Mark \\{x.title}\", click = fn() { x.title = \"\\{x.title}!\"; });\n            }\n        }\n    }\n    markdown { }\n}\n@@@\n@board;\n";
/** The same with no widget of its own in a row, so nothing else takes a number. */
const PLAIN = TASKS.replace("                @row(x = x);\n", "");
const labels = (p) => everyNode(p.tree().nodes).map((x) => x.inputs.label.text);
const press = (p, label) => p.event(everyNode(p.tree().nodes).find((x) => x.inputs.label?.text === label).id, "click");
const titles = (p) => p.state().xs.list.map((x) => x.record.fields.title.text);
const list = (...of) => ({ list: of.map((x) => ({ record: { type: "doc/task", fields: { title: { text: x } } } })) });

test("an item a handler adds is another than every item there, whoever gave those (#14616)", () => {
  for (const start of [(p) => p, (p) => (p.set({ xs: list("a", "b") }), p)]) {
    const p = start(page(PLAIN));
    press(p, "Add");
    assert.deepEqual(labels(p), ["Add", "Mark a", "Mark b", "Mark new"]);
    press(p, "Mark new");
    assert.deepEqual(titles(p), ["a", "b", "new!"]);
  }
});

test("a list the host replaces keeps the rows of the items it left alone (page 3)", () => {
  const said = (p) => labels(p).filter((x) => x.includes(":"));
  const at = (p, label) => everyNode(p.tree().nodes).find((x) => x.inputs.label.text === label).id;
  const p = page(TASKS);
  press(p, "a: 0");
  press(p, "b: 0");
  const [a, b] = [at(p, "a: 1"), at(p, "b: 1")];
  // one added in front, and another order
  p.set({ xs: list("new", "b", "a") });
  assert.deepEqual(said(p), ["new: 0", "b: 1", "a: 1"]);
  assert.deepEqual([at(p, "a: 1"), at(p, "b: 1")], [a, b]);
  // the whole of it again changes nothing
  p.set({ xs: p.state().xs });
  assert.deepEqual(said(p), ["new: 0", "b: 1", "a: 1"]);
  // one changed and one gone
  p.act({ set: { fields: { xs: list("new", "a!") } } });
  assert.deepEqual(said(p), ["new: 0", "a!: 0"]);
  // a value that is there twice, before or after, is no one item
  p.set({ xs: list("x", "y") });
  press(p, "x: 0");
  press(p, "y: 0");
  p.set({ xs: list("x", "x", "y") });
  assert.deepEqual(said(p), ["x: 0", "x: 0", "y: 1"]);
  press(p, "x: 0");
  p.set({ xs: list("x", "y") });
  assert.deepEqual(said(p), ["x: 0", "y: 1"]);
  // unless the list is as it was
  p.set({ xs: list("x", "x") });
  press(p, "x: 0");
  p.set({ xs: list("x", "x") });
  assert.deepEqual(said(p), ["x: 1", "x: 0"]);
  // and an item a handler adds then is another than all of them
  press(p, "Add");
  press(p, "Mark new");
  assert.deepEqual(titles(p), ["x", "x", "new!"]);
});

test("a list in a record, and a channel's, keep their rows the same way", () => {
  const row = 'widget row(x: text) {\n    mut votes: int = 0;\n    view {\n        @press(label = "\\{x}: \\{votes}", click = fn() { votes = votes + 1; });\n    }\n    markdown { }\n}\n';
  const texts = (...of) => ({ list: of.map((text) => ({ text })) });
  const rows = (p) => labels(p);
  const inner = page(`record board {\n    name: text;\n    xs: list<text> = ["a", "b"];\n}\n\n$state {\n    b: board = board { name = "one" };\n}\n\n${row}@@@\n@for (x in $state.b.xs) {\n    @row(x = x);\n}\n`);
  press(inner, "a: 0");
  inner.set({ b: { record: { type: "doc/board", fields: { name: { text: "two" }, xs: texts("c", "a") } } } });
  assert.deepEqual(rows(inner), ["c: 0", "a: 1"]);
  const given = page(`extern host {\n    xs: list<text> = ["a", "b"];\n}\n\n${row}@@@\n@for (x in host.xs) {\n    @row(x = x);\n}\n`);
  given.setExtern("doc/host", { xs: texts("p", "q") });
  press(given, "q: 0");
  given.setExtern("doc/host", { xs: texts("q", "r") });
  assert.deepEqual(rows(given), ["q: 1", "r: 0"]);
});

test("a list the host replaces by other items has its rows made again: a row's memory never moves to another item (page 3)", () => {
  const p = page(TASKS);
  press(p, "a: 0");
  press(p, "b: 0");
  assert.deepEqual(labels(p).filter((x) => x.includes(":")), ["a: 1", "b: 1"]);
  p.set({ xs: list("c", "d") });
  assert.deepEqual(labels(p).filter((x) => x.includes(":")), ["c: 0", "d: 0"]);
  // and its data put back is, by place
  const [q, data] = [page(TASKS), p.data()];
  press(q, "a: 0");
  q.restore(data);
  assert.deepEqual(labels(q).filter((x) => x.includes(":")), ["c: 0", "d: 0"]);
});

test("a fn is the page's own: data put back leaves out a variable that holds one, and puts back the rest", () => {
  const doc = "$state {\n    n: number = 1;\n}\n\nwidget w() {\n    mut f: fn(): number = fn() { return 1; };\n    mut k: number = 0;\n\n    view {\n        @label(value = \"\\{f()} \\{k}\");\n        @press(label = \"Take\", click = fn() { f = fn() { return $state.n + 1; }; k = k + 1; });\n    }\n    markdown { }\n}\n@@@\n@w;\n";
  const label = (p) => everyNode(p.tree().nodes)[0].inputs.label?.text ?? everyNode(p.tree().nodes)[0].inputs.value?.text;
  const p = page(doc);
  press(p, "Take");
  assert.equal(label(p), "2 1");
  const data = JSON.parse(JSON.stringify(p.data()));
  const q = page(doc);
  assert.deepEqual(q.restore(data), ["s0.f"]);
  assert.deepEqual([label(q), q.takeOutcome().failures], ["1 1", []]);
});

test("the same values give the same ids, however a page came to start with them (page 6)", () => {
  const ids = (p) => everyNode(p.tree().nodes).map((x) => x.id);
  const plain = page(TASKS);
  const { ir } = plain;
  const list = plain.state().xs;
  assert.deepEqual(ids(new Page(ir, { data: plain.data() })), ids(plain));
  assert.deepEqual(ids(new Page(ir, { state: { xs: list } })), ids(plain));
  // refused, it starts with its own
  assert.deepEqual(ids(new Page(ir, { state: { xs: { list: [{ num: "1" }] } } })), ids(plain));
  // and an item added then is another than every one there
  const p = new Page(ir, { data: plain.data() });
  press(p, "Add");
  assert.equal(new Set(ids(p)).size, ids(p).length);
  assert.deepEqual(labels(p).filter((x) => x.includes(":")), ["a: 0", "b: 0", "new: 0"]);
});
