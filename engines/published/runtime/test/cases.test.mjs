// What a page does, case by case, on the runtime alone. The pages with planes, which whoever draws them lays
// out, are run with the web's widgets (engines/published/dom/test/planes.test.mjs).

import { test } from "node:test";
import assert from "node:assert/strict";
import { cases, run } from "./run-case.mjs";

for (const c of cases.filter((c) => !c.planes)) test(`${c.id}: ${c.about}`, () => run(c));

test("most cases need no layout", () => assert.ok(cases.filter((c) => !c.planes).length > 20));
