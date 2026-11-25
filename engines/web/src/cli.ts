#!/usr/bin/env node
// xtmd-web: what `xtmd build --engine web` runs.

import { build, type Options } from "./index.js";
import { command, defaultOf } from "./run.js";

await command<Options>("xtmd-web", "pages into HTML, CSS and scripts", build, {
  "--shell": {
    help: "FILE     a module whose default is the page around a page: a fn of it to HTML",
    take: async (o, v) => void (o.shell = await defaultOf(v())),
  },
});
