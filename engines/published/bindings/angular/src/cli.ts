#!/usr/bin/env node
// xtmd-angular: what `xtmd build --engine angular` runs.

import type { ComponentOptions } from "@extended-mark/web/components";
import { command } from "@extended-mark/web/run";
import { build } from "./engine.js";

await command<ComponentOptions>("xtmd-angular", "pages into Angular components", build, {
  "--components": { help: "a/b,c/d   the widgets the host gives Angular components for, by full name", take: (o, v) => void (o.components = v().split(",")) },
});
