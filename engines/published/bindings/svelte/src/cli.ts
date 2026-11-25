#!/usr/bin/env node
// xtmd-svelte: what `xtmd build --engine svelte` runs.

import type { ComponentOptions } from "@extended-mark/web/components";
import { command } from "@extended-mark/web/run";
import { build } from "./engine.js";

await command<ComponentOptions>("xtmd-svelte", "pages into Svelte components", build, {
  "--components": { help: "a/b,c/d   the widgets the host gives Svelte components for, by full name", take: (o, v) => void (o.components = v().split(",")) },
});
