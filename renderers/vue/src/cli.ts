#!/usr/bin/env node
// xtmd-vue: what `xtmd build --engine vue` runs.

import type { ComponentOptions } from "@extended-mark/web/components";
import { command } from "@extended-mark/web/run";
import { build } from "./engine.js";

await command<ComponentOptions>("xtmd-vue", "pages into Vue components", build, {
  "--components": { help: "a/b,c/d   the widgets the host gives Vue components for, by full name", take: (o, v) => void (o.components = v().split(",")) },
});
