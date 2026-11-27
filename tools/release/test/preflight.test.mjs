// The release preflight's decisions, on npm's answers as they came on 20 and 21 November 2026: the name
// `xtmd` refused, a token limited to the @extended-mark scope, and the one that may write everything.

import assert from "node:assert/strict";
import { test } from "node:test";
import { migrationFindings, migrationTree, npmFindings, releaseBefore, tokenFindings } from "../preflight.mjs";

const names = ["@extended-mark/runtime", "extended-mark", "@extended-mark/react"];
const now = Date.parse("2026-11-21T12:00:00Z");

test("an unscoped name npm has never seen stops the release: npm may refuse it", () => {
  const seen = { xtmd: { status: 404 }, "@extended-mark/runtime": { status: 200, versions: { "0.3.0": {} } } };
  const f = npmFindings(["xtmd", "@extended-mark/runtime"], seen, { version: "0.3.1", trusted: false });
  assert.equal(f.errors.length, 1);
  assert.match(f.errors[0], /^xtmd: npm has no package of this name/);
  assert.deepEqual(f.fresh, []);
});

test("a new scoped package is staged by the token, and is no problem", () => {
  const seen = { "@extended-mark/new": { status: 404 }, "extended-mark": { status: 200, versions: {} } };
  const f = npmFindings(["@extended-mark/new", "extended-mark"], seen, { version: "0.3.1", trusted: false });
  assert.deepEqual(f.errors, []);
  assert.deepEqual(f.fresh, ["@extended-mark/new"]);
});

test("under trusted publishing no package may be new", () => {
  const f = npmFindings(["@extended-mark/new"], { "@extended-mark/new": { status: 404 } }, { version: "0.3.1", trusted: true });
  assert.match(f.errors[0], /is new: trusted publishing is set up per package/);
});

test("a version npm has already is passed over, with a note", () => {
  const f = npmFindings(["extended-mark"], { "extended-mark": { status: 200, versions: { "0.3.1": {} } } }, { version: "0.3.1", trusted: false });
  assert.deepEqual(f.errors, []);
  assert.deepEqual(f.notes, ["extended-mark@0.3.1 is on npm already: passed over"]);
});

test("a token limited to the @extended-mark scope may not write extended-mark", () => {
  const scoped = { expiry: "2026-11-27T18:37:48.449Z", permissions: [{ name: "package", action: "write" }, { name: "org", action: "write" }], scopes: [{ name: "@extended-mark", type: "package" }, { name: "extended-mark", type: "org" }] };
  const t = tokenFindings(scoped, names, now);
  assert.deepEqual(t.errors, ["NPM_TOKEN may not write extended-mark: give it Read and write on all packages"]);
  assert.equal(t.ok, null);
});

test("a token on all packages may write every one", () => {
  const all = { expiry: "2026-11-28T06:04:53.064Z", permissions: [{ name: "package", action: "write" }, { name: "org", action: "write" }], scopes: [{ name: null, type: "package" }, { name: "extended-mark", type: "org" }] };
  const t = tokenFindings(all, names, now);
  assert.deepEqual(t.errors, []);
  assert.equal(t.ok, "NPM_TOKEN may write all 3 packages; it expires 2026-11-28");
});

test("a token that runs out within two days, or only reads, stops the release", () => {
  const soon = { expiry: "2026-11-22T00:00:00Z", permissions: [{ name: "package", action: "write" }], scopes: [{ name: null, type: "package" }] };
  assert.match(tokenFindings(soon, names, now).errors[0], /expires 2026-11-22/);
  const reads = { permissions: [{ name: "package", action: "read" }], scopes: [{ name: null, type: "package" }] };
  assert.match(tokenFindings(reads, names, now).errors[0], /can't write packages/);
});

// xtmd migrate: a release can't ship without what moves a tree written for the release right before it

const tags = ["v0.2.9", "v0.3.0", "v0.3.1", "v0.3.2", "v0.3.2^{}"];
const whole = { copied: "v0.3.2", reads: "0.3.2", step: true, ready: true, corpora: ["migrate-0.3"] };

test("the release before is the newest tag below the version, patch releases too", () => {
  assert.equal(releaseBefore("0.4.0", tags), "v0.3.2");
  assert.equal(releaseBefore("0.3.2", tags), "v0.3.1");
  assert.equal(releaseBefore("0.4.1", [...tags, "v0.4.0"]), "v0.4.0");
  assert.equal(releaseBefore("0.1.0", tags), null);
});

test("a release with the release before's front end, its step, on, and its corpus can ship", () => {
  const f = migrationFindings("0.4.0", tags, whole);
  assert.deepEqual(f.errors, []);
  assert.match(f.ok, /^a tree written for 0\.3\.2 moves/);
});

test("a release without its step, or with it off, can't ship", () => {
  assert.match(migrationFindings("0.4.0", tags, { ...whole, step: false }).errors.join("\n"), /there is no step from 0\.3\.2/);
  assert.match(migrationFindings("0.4.0", tags, { ...whole, ready: false }).errors.join("\n"), /isn't on \(rir::READY\)/);
});

test("a release whose front end is an older release's, or whose readers read another, can't ship", () => {
  const f = migrationFindings("0.4.1", [...tags, "v0.4.0"], whole);
  assert.match(f.errors[0], /^compiler\/xtmd\/previous\/ is v0\.3\.2's front end, not v0\.4\.0's: run node tools\/release\/previous\.mjs v0\.4\.0/);
  assert.match(f.errors.join("\n"), /readers::PREVIOUS .* is "0\.3\.2", not "0\.4\.0"/);
  assert.match(f.errors.join("\n"), /checks\/migrate-0\.4\/ has no corpus/);
  assert.match(migrationFindings("0.4.0", tags, { ...whole, copied: null }).errors[0], /^compiler\/xtmd\/previous\/ holds no front end, not v0\.3\.2's/);
});

test("this repository can ship its VERSION: 0.4.0 moves 0.3.2's trees", () => {
  const f = migrationFindings("0.4.0", tags, migrationTree());
  assert.deepEqual(f.errors, []);
});
