// The host's contract (page 6, The host): the tree every runtime gives fits its schema, its ids are one
// each and follow their items, what changed makes the next tree from the last, and a change works out
// only what reads it.

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { Page, apply } from "../dist/index.js";
import { HostPage, Page as Core, coreOf } from "../dist/internal.js";
import { xtmdBinary, workdir, repo } from "./xtmd.mjs";
import { fits } from "./fits.mjs";

const dir = workdir();
let n = 0;

function build(file, packs = []) {
  const out = join(dir, `host${n++}.ir.json`);
  try {
    execFileSync(xtmdBinary(), ["build", file, ...(packs.length ? ["--pack", packs.join(",")] : []), "--out", out], { stdio: ["ignore", "pipe", "pipe"] });
  } catch {
    return undefined; // a case about a compile error
  }
  return JSON.parse(readFileSync(out, "utf8"));
}

function ir(text) {
  const file = join(dir, `host${n++}.xtmd`);
  writeFileSync(file, text);
  const x = build(file);
  assert.ok(x, `doesn't compile:\n${text}`);
  return x;
}

const page = (text, opts) => new Page(ir(text), opts);

function every(nodes, out = []) {
  const walk = (x) => {
    if (x.nodes) every(x.nodes, out);
    else if (x.list) x.list.forEach(walk);
    else if (x.record) Object.values(x.record.fields).forEach(walk);
    else if (x.case) Object.values(x.case.values ?? {}).forEach(walk);
    else if (x.some) walk(x.some);
  };
  for (const n of nodes) {
    out.push(n);
    Object.values(n.inputs).forEach(walk);
  }
  return out;
}

const find = (p, widget, k = 0) => every(p.tree().nodes).filter((n) => n.widget === widget)[k];

function check(p, name) {
  const e = fits(p.tree(), "tree", "tree");
  assert.equal(e, undefined, `${name}: ${e}`);
  const ids = every(p.tree().nodes).map((n) => n.id);
  assert.equal(new Set(ids).size, ids.length, `${name}: an id twice: ${ids.filter((x, k) => ids.indexOf(x) !== k)}`);
  assert.ok(ids.every((x) => x && p.node(x).id === x));
}

/** The pages of the cases that compile. */
function cases() {
  const out = [];
  const walk = (d) => {
    for (const f of readdirSync(d)) {
      const p = join(d, f);
      if (statSync(p).isDirectory()) walk(p);
      else if (f.endsWith(".json")) {
        const c = JSON.parse(readFileSync(p, "utf8"));
        const src = join(dirname(p), c.source ?? "");
        if (!c.source?.endsWith(".xtmd") || !existsSync(src)) continue;
        const x = build(src, (c.packs ?? []).map((q) => join(dirname(p), q)));
        if (x) out.push([c.id, x]);
      }
    }
  };
  walk(join(repo, "checks/cases"));
  walk(join(repo, "runtime/typescript/test/cases"));
  return out;
}

function random(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Something a reader or a host could do now. */
function act(p, rnd) {
  const acts = [() => p.setPage({ theme: rnd() < 0.5 ? "DARK" : "LIGHT", width: rnd() < 0.5 ? "NARROW" : "WIDE" })];
  for (const n of every(p.tree().nodes)) {
    for (const e of n.events ?? []) acts.push(() => p.event(n.id, e, e === "change" ? [{ text: "x" }] : []));
    if (n.widget === "core/slider") acts.push(() => p.write(n.id, "value", { num: String(Math.floor(rnd() * 12)) }));
    if (n.widget === "core/field") acts.push(() => p.write(n.id, "value", { text: ["", "a", "lemon"][Math.floor(rnd() * 3)] }));
    if (n.widget === "core/overlay") acts.push(() => p.write(n.id, "open", { bool: rnd() < 0.5 }));
  }
  return acts[Math.floor(rnd() * acts.length)];
}

test("every case's tree, state, Markdown blocks and what it tells fit their schemas, has an id for each node, and changes make the next tree", () => {
  let pages = 0;
  let changed = 0;
  let said = 0;
  for (const [id, x] of cases()) {
    let p;
    try {
      p = new Page(x, { supplies: () => true });
    } catch {
      continue; // a page that can't be mounted without what its host gives
    }
    pages++;
    check(p, id);
    let mine = p.tree();
    p.onChange((changes) => {
      for (const c of changes) assert.equal(fits(c, "tree", "change"), undefined, id);
      mine = apply(mine, changes);
      changed += changes.length;
    });
    const rnd = random(id.length * 7919 + 3);
    for (let k = 0; k < 12; k++) {
      act(p, rnd)();
      check(p, id);
      assert.deepEqual(mine, p.tree(), `${id}: after change ${k + 1}`);
    }
    assert.equal(fits(p.data(), "state", "data"), undefined, id);
    const told = { calls: p.takeCalls(), ...p.takeOutcome(), warnings: p.takeWarnings() };
    assert.equal(fits(told, "told", "told"), undefined, id);
    said += told.calls.length + told.refused.length + told.failures.length;
    assert.equal(fits({ blocks: p.blocks() }, "blocks", "blocks"), undefined, id);
  }
  assert.ok(pages > 50 && changed > 50 && said > 0, `${pages} pages, ${changed} changes, ${said} told`);
});

const TASKS = `record task {
    title: text;
}

$state {
    tasks: list<task> = [task { title = "a" }, task { title = "b" }, task { title = "c" }];
}

widget row(t: task) {
    mut votes: int = 0;
    view {
        @press(label = "\\{t.title}: \\{votes}", click = fn() { votes = votes + 1; });
    }
}

widget board() {
    view {
        mut for (t in $state.tasks) {
            if (mut x: task = t) {
                @row(t = x);
                @press(label = "Drop", click = fn() { $state.tasks.remove(x); });
            }
        }
    }
}
@@@
@board;
`;

test("a page's tree is the one every runtime gives", () => {
  // the same files xtmd-runtime's tests read (runtime/rust/tests/host.rs)
  const given = { supplies: () => true, externs: { "doc/clock": { today: { text: "Tuesday" } } } };
  for (const [page, tree] of [["schemas/examples/counter.xtmd", "schemas/examples/counter.tree.json"], ["apps/example-host/page.xtmd", "apps/example-host/test/page.tree.json"]]) {
    assert.deepEqual(new Page(build(join(repo, page)), given).tree(), JSON.parse(readFileSync(join(repo, tree), "utf8")), page);
  }
});

test("an id follows its item, and what changed says what is kept", () => {
  const p = page(TASKS);
  const label = (n) => n.inputs.label.text;
  const b = find(p, "core/press", 2);
  assert.equal(label(b), "b: 0");
  p.event(b.id, "click");
  assert.equal(label(p.node(b.id)), "b: 1");
  let last;
  p.onChange((c) => (last = c));
  p.event(find(p, "core/press", 1).id, "click");
  assert.equal(label(p.node(b.id)), "b: 1");
  assert.equal(last.length, 1);
  assert.equal(last[0].id, "");
  assert.ok(last[0].children.nodes.every((c) => "keep" in c) && last[0].children.nodes.length === 4);
  // a label is one input of one node
  p.event(b.id, "click");
  assert.deepEqual(last, [{ id: b.id, inputs: { label: { text: "b: 2" } } }]);
  // what nothing changed in is the same object
  const c = find(p, "core/press", 2);
  p.event(b.id, "click");
  assert.equal(p.node(c.id), c);
});

test("a widget that fails is a node that says why, and is another when it draws again", () => {
  const p = page('$state {\n    parts: int 0... = 0;\n}\n\nwidget share() {\n    view {\n        @label(value = "Each: \\{100 / $state.parts}");\n    }\n}\n@@@\n@share;\n\nAfter.\n');
  const [failed, after] = p.tree().nodes;
  assert.deepEqual({ ...failed, id: "" }, { id: "", widget: "doc/share", inputs: {}, failed: "a division by zero" });
  assert.equal(after.widget, "core/paragraph");
  let last;
  p.onChange((c) => (last = c));
  p.set({ parts: { num: "4" } });
  assert.equal(last[0].children.nodes[0].node.inputs.value.text, "Each: 25");
  assert.deepEqual(last[0].children.nodes[1], { keep: after.id });
});

test("what a widget writes has to fit what its node says", () => {
  const p = page('$state {\n    n: int 0..10 = 3;\n}\n@@@\n@slider(value = $state.n, label = "N");\n');
  const s = find(p, "core/slider");
  assert.deepEqual(s.writes.value, { range: { low: { num: "0" }, high: { num: "10" } }, step: { num: "1" } });
  p.write(s.id, "value", { num: "7" });
  assert.deepEqual(p.state(), { n: { num: "7" } });
  p.write(s.id, "value", { num: "12" });
  assert.deepEqual(p.state(), { n: { num: "7" } });
  assert.deepEqual(p.takeOutcome().refused, [{ code: "contracts.outside", site: s.id }]);
});

test("a fn is called by its node's id and its input, and moves when what it reads does", () => {
  const p = page(`$state {
    red: number 0..1 = 1;
    other: int = 0;
}

fn tint(at: point, px: number, args: number): pixel {
    return pixel { r = $state.red, g = at.x.clamp(0, 1), b = 0, a = 1 };
}

widget picture() {
    view {
        @canvas(label = "Picture") {
            @layer {
                @shade(label = "Tint", area = region { from = point { x = 0, y = 0 }, to = point { x = 1, y = 1 } }, shader = tint, args = 0);
            }
        }
        @label(value = "\\{$state.other}");
    }
}
@@@
@picture;
`);
  const id = find(p, "core/shade").id;
  const point = { record: { type: "core/point", fields: { x: { num: "0.5" }, y: { num: "0" } } } };
  const pixel = p.call(id, "shader", [point, { num: "0.01" }, { num: "0" }]);
  assert.deepEqual([pixel.record.fields.r, pixel.record.fields.g], [{ num: "1" }, { num: "0.5" }]);
  const version = () => p.node(id).inputs.shader.fn.version;
  const was = version();
  let last;
  p.onChange((c) => (last = c));
  p.set({ other: { num: "1" } });
  assert.equal(version(), was);
  p.setPage({ theme: "DARK" });
  assert.equal(version(), was);
  p.set({ red: { num: "0.25" } });
  assert.equal(version(), was + 1);
  assert.deepEqual(last, [{ id, inputs: { shader: { fn: { version: was + 1 } } } }]);
  assert.deepEqual(p.call(id, "shader", [point, { num: "0.01" }, { num: "0" }]).record.fields.r, { num: "0.25" });
});

test("a page's data takes out what it holds and puts it back", () => {
  const p = page(TASKS);
  p.event(find(p, "core/press", 2).id, "click");
  const data = JSON.parse(JSON.stringify(p.data()));
  const q = page(TASKS);
  q.set(data.state);
  assert.deepEqual(q.restoreOwn(data.own), []);
  assert.equal(find(q, "core/press", 2).inputs.label.text, "b: 1");
  assert.deepEqual(q.data(), data);
});

test("a page that starts from its data is drawn once, with it, and one put back later once more", () => {
  // a widget that is drawn only with the $state put back, and one whose own value is an extern's
  const text = `$state {
    open: bool = false;
    n: int 0..10 = 0;
}

widget counter() {
    mut k: int 0..5 = 0;
    view {
        @press(label = "\\{k}", click = fn() { k = (k + 1).clamp(0, 5); });
    }
    markdown {
        x
    }
}
@@@
@if ($state.open) {
@counter;

@overlay(label = "More") {
@slot anchor {
@label(value = "Open");
}
@slot body {
In it.
}
}
}

\\{$state.n}
`;
  const built = ir(text);
  const p = new Page(built);
  p.set({ open: { bool: true }, n: { num: "3" } });
  p.event(find(p, "core/press", 0).id, "click");
  p.write(find(p, "core/overlay", 0).id, "open", { bool: true });
  const data = JSON.parse(JSON.stringify(p.data()));
  assert.equal(Object.keys(data.own).length, 2, JSON.stringify(data.own));
  const draws = (x) => coreOf(x).draws;
  const q = new Page(built, { data });
  assert.equal(draws(q), 1);
  assert.deepEqual(q.left, []);
  assert.deepEqual(q.data(), data);
  assert.equal(find(q, "core/press", 0).inputs.label.text, "1");
  assert.deepEqual(find(q, "core/overlay", 0).inputs.open, { bool: true });
  assert.equal(q.markdown(), p.markdown());
  // later, on a page that shows something else: one draw, and its listeners hear it once
  const r = new Page(built);
  let heard = 0;
  r.onChange(() => heard++);
  assert.deepEqual(r.restore(data), []);
  assert.deepEqual([draws(r), heard], [2, 1]);
  assert.deepEqual(r.data(), data);
  // and again, over what it holds now
  r.event(find(r, "core/press", 0).id, "click");
  assert.deepEqual(r.restore(data), []);
  assert.deepEqual(r.data(), data);
  // what is drawn now is what is drawn with everything made anew
  const anew = new Page(built, { data });
  assert.deepEqual(r.tree().nodes.map((n) => n.widget), anew.tree().nodes.map((n) => n.widget));
});

test("data that doesn't fit: $state is refused whole, and own variables are left out one by one", () => {
  const built = ir(TASKS);
  const p = new Page(built);
  p.event(find(p, "core/press", 2).id, "click");
  const data = JSON.parse(JSON.stringify(p.data()));
  const [place] = Object.keys(data.own).filter((k) => data.own[k].votes.num === "1");
  const odd = { ...data, own: { ...data.own, [place]: { votes: { text: "many" }, gone: { num: "1" } }, "s9": { k: { num: "1" } } } };
  const q = new Page(built, { data: odd });
  assert.deepEqual([...q.left].sort(), [`${place}.gone`, `${place}.votes`, "s9.k"].sort());
  assert.equal(find(q, "core/press", 2).inputs.label.text, "b: 0");
  // a $state that doesn't fit: nothing goes back, and it is told
  const r = new Page(built, { data: { state: { tasks: { text: "none" } }, own: data.own } });
  assert.equal(r.left, undefined);
  assert.equal(coreOf(r).draws, 1);
  assert.deepEqual(r.takeOutcome().refused.map((x) => x.site), ["$state.tasks"]);
  assert.equal(find(r, "core/press", 2).inputs.label.text, "b: 0");
  assert.equal(r.restore({ state: { tasks: { text: "none" } }, own: data.own }), undefined);
  assert.equal(find(r, "core/press", 2).inputs.label.text, "b: 0");
  // a page that started from no data has none left
  assert.equal(new Page(built).left, undefined);
});

test("a change works out only what reads it, whatever else is on the page", () => {
  const doc = (rows) => `$state {
    n: int = 0;
    other: int = 0;
    rows: list<int> = [${Array.from({ length: rows }, (_, k) => k).join(", ")}];
}

widget one(k: int) {
    view {
        if (k > 3) {
            @label(value = "row \\{k} of \\{$state.rows.len()}");
        } else {
            @label(value = "small \\{k}");
        }
    }
}

widget all() {
    view {
        for (r in $state.rows) {
            @one(k = r);
        }
        @label(value = "n is \\{$state.n}");
    }
}
@@@
@all;

${Array.from({ length: rows }, (_, k) => `Paragraph ${k} with **\\{$state.other}** in it.`).join("\n\n")}
`;
  const worked = (rows, change) => {
    const p = new HostPage(ir(doc(rows)), { supplies: () => true });
    // the runtime's own count of what it worked out
    const machine = p.core.machine;
    const before = machine.worked;
    const tree = p.tree();
    change(p);
    assert.notEqual(p.tree(), tree);
    return machine.worked - before;
  };
  const n = (p) => p.set({ n: { num: "1" } });
  const few = worked(8, n);
  assert.ok(few <= 12, `${few} worked out for one label`);
  assert.equal(worked(80, n), few);
  // what reads it, however many that is, and nothing else
  const other = (p) => p.set({ other: { num: "1" } });
  assert.equal(worked(40, other) - worked(20, other), 20 * (worked(21, other) - worked(20, other)));
  // the same without the host's side of it: nothing at all for a value nothing reads
  const c = new Core(ir("$state {\n    unread: int = 0;\n}\n@@@\nHi.\n"));
  const at = c.machine.worked;
  c.set({ unread: c.value({ num: "3" }) });
  assert.ok(c.machine.worked - at <= 1, `${c.machine.worked - at}`);
});

