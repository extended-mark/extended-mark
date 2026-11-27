#!/usr/bin/env node
// xtmd-next: what `xtmd export --engine next` runs.

import type { ComponentOptions } from "@extended-mark/web/components";
import { command } from "@extended-mark/web/run";
import { build } from "./engine.js";

await command<ComponentOptions>("xtmd-next", "pages into React components for Next.js", build, {
  "--components": { help: "a/b,c/d   the widgets the host gives React components for, by full name", take: (o, v) => void (o.components = v().split(",")) },
});
