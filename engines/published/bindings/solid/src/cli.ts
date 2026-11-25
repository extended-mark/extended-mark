#!/usr/bin/env node
// xtmd-solid: what `xtmd build --engine solid` runs.

import type { ComponentOptions } from "@extended-mark/web/components";
import { command } from "@extended-mark/web/run";
import { build } from "./engine.js";

await command<ComponentOptions>("xtmd-solid", "pages into Solid components", build, {
  "--components": { help: "a/b,c/d   the widgets the host gives Solid components for, by full name", take: (o, v) => void (o.components = v().split(",")) },
});
