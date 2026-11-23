// node --test test/  — the migration on two real 0.2 bundles (built by mkp 0.2.10), and the prose
// escapes. Where the 0.3 compiler is built (compiler/target/release/mkp), its output is also
// formatted and checked by it.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { migrate } from '../src/migrate.mjs';
import { inlineProse, tex } from '../src/prose.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const mkp = join(here, '../../../compiler/target/release/mkp');
const bundle = (name) => JSON.parse(readFileSync(join(here, 'fixtures', name), 'utf8'));

/** Writes a migration's files, then runs mkp fmt --check and mkp check on them. */
function check03(r) {
  if (!existsSync(mkp)) return null;
  const dir = mkdtempSync(join(tmpdir(), 'migrate-0.2-'));
  writeFileSync(join(dir, 'doc.mkp.md'), r.doc);
  const packs = [];
  for (const [p, text] of r.packs) {
    mkdirSync(join(dir, p));
    writeFileSync(join(dir, p, `${p}.mkp`), text);
    packs.push(join(dir, p));
  }
  const fmt = spawnSync(mkp, ['fmt', '--check', join(dir, 'doc.mkp.md'), ...packs.map((p) => join(p, `${p.split('/').pop()}.mkp`))], { encoding: 'utf8' });
  const check = spawnSync(mkp, ['check', ...(packs.length ? ['--pack', packs.join(',')] : []), join(dir, 'doc.mkp.md')], { encoding: 'utf8' });
  return { fmt: fmt.status, check: check.status, out: check.stdout + check.stderr };
}

test('prose: what 0.3 reads differently is escaped, the rest passes through', () => {
  assert.equal(inlineProse('@ops is on call'), '\\@ops is on call');
  assert.equal(inlineProse('a\n}\nb'), 'a\n\\}\nb');
  assert.equal(inlineProse('CommonMark \\{brace\\}'), 'CommonMark {brace\\}');
  assert.equal(inlineProse('code `\\{x}` stays'), 'code `\\{x}` stays');
  assert.equal(inlineProse('set $\\{ x \\}$ in maths'), 'set $\\\\{ x \\}$ in maths');
  assert.equal(inlineProse('a hard break\\\nnext'), 'a hard break\\\nnext');
  assert.equal(tex('\\{ x \\}'), '\\\\{ x \\}');
});

test('a document using a pack: blocks become calls, YAML becomes record literals, containers take prose', () => {
  const r = migrate(bundle('pack-small.bundle.json'));
  assert.match(r.doc, /^import kit;/);
  assert.match(r.doc, /@callout\(title = "Deploys", tone = color\.WARN\) \{\nWe shipped \*\*42\*\* times\.\n\}/);
  assert.match(r.doc, /stat \{ label = "Rollbacks", value = "1", tone = color\.BAD \}/);
  assert.match(r.doc, /summary = "Two numbers from this week\."/);
  assert.match(r.doc, /@tabs \{\n@tab\(title = "Web"\) \{\nThe web app\.\n\}/);
  assert.match(r.doc, /\\@ops is on call/);
  assert.match(r.packs.get('kit'), /extern widget stats\(items: list<stat>, summary: text = ""\)/);
  assert.equal(r.counts.fallbacks, 0);
  const c = check03(r);
  if (c) assert.deepEqual([c.fmt, c.check], [0, 0], c.out);
});

test("a document's own widget: inputs, a mut, events as fns, project md as a markdown block", () => {
  const r = migrate(bundle('counter.bundle.json'));
  assert.match(r.doc, /widget counter\(start: int = 0\) \{/);
  assert.match(r.doc, /mut n: int = start;/);
  assert.match(r.doc, /@press\(label = "one less", click = fn\(\) \{ n = n - 1; \}\)/);
  assert.match(r.doc, /markdown \{\n\s+Count: \\\{n\.fmt\(0\)\}/);
  assert.match(r.doc, /@counter\(start = 3\);/);
  assert.equal(r.counts.defsMigrated, 1);
  const c = check03(r);
  if (c) assert.deepEqual([c.fmt, c.check], [0, 0], c.out);
});

test('the pack widgets mode writes a view where the definition migrates', () => {
  const r = migrate(bundle('pack-small.bundle.json'), { packWidgets: true });
  assert.match(r.packs.get('kit'), /^widget callout\(/m);
  assert.ok(r.counts.packWidgetsWritten >= 1);
  const c = check03(r);
  if (c) assert.deepEqual([c.fmt, c.check], [0, 0], c.out);
});

test('with the 0.2 source beside the IR, every comment and link definition is kept', async () => {
  const { sourceComments, kept } = await import('../src/source.mjs');
  const source = readFileSync(join(here, 'fixtures', 'comments.mkp.md'), 'utf8');
  const r = migrate(bundle('comments.bundle.json'), { source });
  const cs = sourceComments(source);
  assert.equal(cs.length, 16);
  assert.deepEqual(cs.filter((c) => !kept(c, [r.doc, ...r.packs.values()])), []);
  // YAML comments sit beside what they described, in the author's key order
  assert.match(r.doc, /\/\/ deploys first\n\s+stat \{ label = "Deploys", value = "42", change = "\+12%" \}, \/\/ up from 37/);
  assert.match(r.doc, /summary = "Two numbers from this week\.", \/\/ the alt text/);
  // link definitions stay in the prose; HTML comments stay in place as `@//`, prose's comment
  assert.match(r.doc, /\n\[p\]: https:\/\/example\.com\/plan\n/);
  assert.match(r.doc, /\n *@\/\/ /);
  assert.doesNotMatch(r.doc, /<!--/);
  // or go to the header, with where they stood, when asked
  const header = migrate(bundle('comments.bundle.json'), { source, htmlComments: 'header' });
  assert.match(header.doc, /\/\/ line 54, at the end of the callout block:/);
  // code comments come from the IR itself
  assert.match(r.doc, /\/\/ what the tile is for, above the definition\n\/\/@about One number in a box\./);
  const c = check03(r);
  if (c) assert.deepEqual([c.fmt, c.check], [0, 0], c.out);
});

test('records: two of one name with other fields stay two, and every rename is said', async () => {
  const { Migration } = await import('../src/context.mjs');
  const { Decls } = await import('../src/decls.mjs');
  const m = new Migration({ defs: {}, types: {} });
  const d = new Decls(m, 'devbench');
  // gantt's `record task {name, start, end}` and checklist's `record task {text, done}`, as XTMD
  // Integration's devbench@1 pack has them
  const gantt = d.recordFor('task', [['name', { t: 'text' }], ['start', { t: 'num' }], ['end', { t: 'num' }]], {}, { line: 3, owner: 'gantt' });
  const checklist = d.recordFor('task', [['text', { t: 'text' }], ['done', { t: 'bool' }]], {}, { line: 9, owner: 'checklist' });
  assert.equal(gantt, 'task');
  assert.equal(checklist, 'checklist_task');
  // a value of either is the same record again
  assert.equal(d.recordFor('task', [['text', { t: 'text' }], ['done', { t: 'bool' }]], {}, { line: 20 }), 'checklist_task');
  assert.equal(d.recordFor('task', [['name', { t: 'text' }], ['start', { t: 'num' }], ['end', { t: 'num' }]], {}, { line: 21 }), 'task');
  const fields = (name) => d.decls().find((x) => x.k === 'record' && x.name === name).fields.map((f) => f.name);
  assert.deepEqual(fields('task'), ['name', 'start', 'end']);
  assert.deepEqual(fields('checklist_task'), ['text', 'done']);
  // a keyword can't be a name: `step` is `step_`, and the migration says so
  assert.equal(d.recordFor('step', [['title', { t: 'text' }]], {}, { line: 30 }), 'step_');
  const said = m.notes.map((n) => n.what);
  assert.ok(said.some((w) => w.includes('`checklist`') && w.includes('`checklist_task`')), said.join('\n'));
  assert.ok(said.some((w) => w.includes('`step` is `step_`') && w.includes('is a keyword')), said.join('\n'));
  // a name kept as it was says nothing
  assert.ok(!said.some((w) => w.includes('`task` is')), said.join('\n'));
});

test("a stand-in's markdown reads each list's own record, not another of the same name", async () => {
  const { Migration } = await import('../src/context.mjs');
  const { PackShim } = await import('../src/packs.mjs');
  const m = new Migration({ defs: {}, types: {} });
  const shim = new PackShim(m, 'devbench', 1);
  const ganttT = { t: 'records', kind: 'task', fields: [['name', { t: 'text' }], ['start', { t: 'num' }], ['end', { t: 'num' }]] };
  const checklistT = { t: 'records', kind: 'task', fields: [['text', { t: 'text' }], ['done', { t: 'bool' }]] };
  shim.recordFor(ganttT.kind, ganttT.fields, {}, { line: 3, owner: 'gantt' });
  shim.recordFor(checklistT.kind, checklistT.fields, {}, { line: 9, owner: 'checklist' });
  const shown = (t) => JSON.stringify(shim.markdown({ slots: [] }, [['tasks', t]]));
  assert.match(shown(checklistT), /x\.text/);
  assert.doesNotMatch(shown(checklistT), /x\.name|x\.start/);
  assert.match(shown(ganttT), /x\.name/);
});
