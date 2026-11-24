// What comes to a page from outside is one of its type, or is refused and nothing changes: a host's writes
// and calls, its extern values, what a widget writes and the arguments of its events (page 6). The changes
// are checks/pages/outside.json's, which the Rust runtime is given too.

import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Page } from "../dist/index.js";
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
