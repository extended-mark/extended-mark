// The memo never changes what is drawn: every page is run with it and without it, through the same
// events, and must come to the same tree, state and Markdown after each.

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { Page } from "../dist/internal.js";
import { xtmdBinary, workdir, repo } from "./xtmd.mjs";

const dir = workdir();
let n = 0;
let why = "";

function build(file, packs = []) {
  const out = join(dir, `memo${n++}.ir.json`);
  try {
    execFileSync(xtmdBinary(), ["build", file, ...(packs.length ? ["--pack", packs.join(",")] : []), "--out", out], { stdio: ["ignore", "pipe", "pipe"] });
  } catch (e) {
    why = String(e.stderr);
    return undefined; // a case about a compile error
  }
  return JSON.parse(readFileSync(out, "utf8"));
}

function source(text) {
  const file = join(dir, `memo${n++}.xtmd`);
  writeFileSync(file, text);
  const ir = build(file);
  assert.ok(ir, `doesn't compile:\n${why}`);
  return ir;
}

/** A small seeded generator, so a failure is the same run again. */
function random(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function nodes(page) {
  const out = [];
  const walk = (ns) => ns.forEach((d) => (out.push(d), [...d.slots.values()].forEach(walk)));
  walk(page.tree());
  return out;
}

const seen = (p) => ({ tree: p.observeTree(), state: p.observeState(), markdown: p.markdown() });

/** One change a reader could make, as a fn of a page: the same on both. */
function change(page, rnd) {
  const acts = [];
  for (const d of nodes(page)) {
    for (const e of d.events.keys()) if (d.widget !== "core/plane") acts.push((p) => p.event(d.path, e, []));
    const write = (input, v) => acts.push((p) => p.write(d.path, input, p.value(v)));
    if (d.widget === "core/slider") write("value", { num: String(Math.floor(rnd() * 12)) });
    if (d.widget === "core/field") write("value", { text: ["", "a", "lemon"][Math.floor(rnd() * 3)] });
    if (d.widget === "core/overlay") write("open", { bool: rnd() < 0.5 });
  }
  return acts.length ? acts[Math.floor(rnd() * acts.length)] : undefined;
}

function same(ir, name, steps = 12) {
  const opts = { supplies: () => true };
  const on = new Page(ir, { ...opts, memo: true });
  const off = new Page(ir, { ...opts, memo: false });
  assert.deepEqual(seen(on), seen(off), `${name}: as it first draws`);
  const rnd = random(name.length * 7919 + 17);
  let did = 0;
  for (let k = 0; k < steps; k++) {
    const act = change(off, rnd);
    if (!act) break;
    act(on);
    act(off);
    did++;
    assert.deepEqual(seen(on), seen(off), `${name}: after change ${k + 1}`);
  }
  return did;
}

test("a view's let read in a slot's rows is followed (#14005)", () => {
  const ir = source(`widget pick() {
    mut chosen: int = -1;

    fn locked(): bool {
        return chosen >= 0;
    }

    view {
        let lock: bool = locked();
        @column {
            for (i in 0..2) {
                @press(label = lock ? "locked \\{i}" : "open \\{i}", click = fn() { chosen = i; });
            }
        }
    }

    markdown {
        chosen \\{chosen}
    }
}
@@@
@pick;
`);
  const p = new Page(ir);
  const labels = () => nodes(p).filter((d) => d.widget === "core/press").map((d) => d.props.get("label").s);
  assert.deepEqual(labels(), ["open 0", "open 1", "open 2"]);
  p.event(nodes(p).find((d) => d.props.get("label")?.s === "open 1").path, "click");
  assert.deepEqual(labels(), ["locked 0", "locked 1", "locked 2"]);
  assert.ok(same(ir, "pick") > 0);
});

test("a handler in a slot keeps up with the caller's values", () => {
  const ir = source(`$state {
    items: list<text> = ["a", "b", "c"];
    picked: number = -1;
}

widget rows(items: list<text>) {
    event pick(at: number);
    view {
        for (p, x in items) {
            @column {
                @press(label = "\\{p}: \\{x}", click = fn() { invoke pick(p); });
            }
        }
    }
    markdown {
        rows
    }
}
@@@
@rows(items = $state.items, pick = fn(at) { $state.picked = at; });

@press(label = "Swap", click = fn() { $state.items.move(0, 2); });

Picked \\{$state.picked}
`);
  const p = new Page(ir);
  const press = (label) => nodes(p).find((d) => d.props.get("label")?.s === label);
  p.event(press("Swap").path, "click");
  p.event(press("0: b").path, "click");
  assert.equal(p.observeState().picked.num, "0");
  assert.ok(same(ir, "rows", 30) > 0);
});

test("an optional list slot: left out, empty and filled", () => {
  const ir = source(`widget about(t: text) {
    view {
        @label(value = t);
    }
    markdown {
        \\{t}
    }
}

widget box(name: text) {
    slot abouts: opt<list<about>>;
    view {
        if (let list: list<about> = abouts) {
            @label(value = "\\{name} \\{list.len()}");
            for (a in list) {
                @a;
            }
        } else {
            @label(value = "\\{name} none");
        }
    }
    markdown {
        \\{name}
    }
}

widget page_box(show: bool) {
    view {
        @box(name = "a");
        @box(name = "b") {
        }
        @box(name = "c") {
            @about(t = "one");
            match (show) {
                case true {
                    @about(t = "two");
                }
                case false {
                }
            }
        }
    }
    markdown {
        x
    }
}
@@@
@page_box(show = true);
`);
  const p = new Page(ir);
  assert.deepEqual(p.tree().map((d) => d.props.get("value").s), ["a none", "b 0", "c 2", "one", "two"]);
  assert.deepEqual(p.takeOutcome().failures, []);
});

test("the runtime makes up no value for a slot the IR doesn't fill", () => {
  const ir = source(`widget box() {
    slot body: view;
    view {
        @body;
    }
    markdown {
        x
    }
}
@@@
@box {
    Hi
}
`);
  for (const t of ir.ir.templates) for (const v of t.view) if (v.use?.name === "doc/box") delete v.use.slots;
  const p = new Page(ir);
  assert.equal(p.takeOutcome().failures.length, 1);
});

// What reaches a slot only through the caller's own values: the cases have no page of this shape, so
// they don't find a memo that forgets it.
const SLOTS = {
  "a let, two slots deep": `@column {
            @row {
                @label(value = "on \\{on}");
            }
        }`,
  "a let's branch": `@column {
            if (on) {
                @label(value = "yes");
            } else {
                @label(value = "no");
            }
        }`,
  "a match on a let": `@frame(title = "f") {
            body {
                match (on) {
                    case true {
                        @label(value = "yes");
                    }
                    case false {
                        @label(value = "no");
                    }
                }
            }
        }`,
  "a let in a loop": `@column {
            for (i in 0..2) {
                let mine: text = on ? "yes \\{i}" : "no \\{i}";
                @row {
                    @label(value = mine);
                }
            }
        }`,
  "a handler": `@column {
            @press(label = "keep", click = fn() { kept = on; });
        }`,
  "a widget that places its slot twice": `@twice {
            @label(value = "on \\{on}");
        }`,
};

for (const [name, view] of Object.entries(SLOTS)) {
  test(`in a slot, ${name}: the same with the memo and without it`, () => {
    const ir = source(`widget twice() {
    slot body: view;
    view {
        @column {
            @body;
        }
        @column {
            @body;
        }
    }
    markdown {
        x
    }
}

widget w() {
    mut turned: bool = false;
    mut kept: bool = false;

    fn odd(): bool {
        return turned;
    }

    view {
        let on: bool = odd();
        @press(label = "turn", click = fn() { turned = !turned; });
        ${view}
        @label(value = "kept \\{kept}");
    }

    markdown {
        \\{turned} \\{kept}
    }
}
@@@
@w;
`.replaceAll("\\\\{", "\\{"));
    const turn = (p) => p.event(nodes(p).find((d) => d.props.get("label")?.s === "turn").path, "click");
    const [on, off] = [true, false].map((memo) => new Page(ir, { memo }));
    for (let k = 0; k < 3; k++) {
      turn(on);
      turn(off);
      assert.deepEqual(seen(on), seen(off), `turn ${k + 1}`);
    }
    assert.ok(same(ir, name, 30) > 0);
  });
}

test("every case draws the same with the memo and without it", () => {
  const root = join(repo, "checks/cases");
  const files = [];
  const walk = (d) => readdirSync(d).forEach((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : f.endsWith(".json") && files.push(join(d, f))));
  walk(root);
  walk(join(dirname(fileURLToPath(import.meta.url)), "cases"));
  let ran = 0;
  let acted = 0;
  for (const f of files) {
    const c = JSON.parse(readFileSync(f, "utf8"));
    const src = c.source && join(dirname(f), c.source);
    if (!src?.endsWith(".xtmd") || !existsSync(src)) continue;
    const ir = build(src, (c.packs ?? []).map((p) => join(dirname(f), p)));
    if (!ir) continue;
    ran++;
    acted += same(ir, relative(root, f));
  }
  assert.ok(ran > 40 && acted > 100, `ran ${ran} pages with ${acted} changes`);
});
