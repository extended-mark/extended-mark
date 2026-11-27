// The extension in real VS Code, driven as a person would and photographed into test/out, every part in one VS Code:
//
//   xvfb-run -a -s "-screen 0 1600x1000x24" node test/drive.mjs              all of them
//   … node test/drive.mjs preview look                                       those
//
// or on a screen to watch, in its size and slower: SIZE=1366x768 STEP=2500. THEME="Default Light Modern" for light.

import { open } from "./instance.mjs";

const PARTS = ["vscode", "preview", "look", "commands"];
const asked = process.argv.slice(2);
const unknown = asked.find((p) => !PARTS.includes(p));
if (unknown) {
  console.error(`no part ${unknown}: there are ${PARTS.join(", ")}`);
  process.exit(2);
}

const v = await open();
let failed = 0;
for (const part of PARTS.filter((p) => !asked.length || asked.includes(p))) {
  const { default: run } = await import(`./parts/${part}.mjs`);
  const s = v.steps(part);
  try {
    await v.fresh();
    await run({ ...v, shot: s.shot });
    s.done();
    console.log(`${part}: ${s.steps.length} steps, all passed`);
  } catch (e) {
    failed++;
    await s.failed();
    console.error(`${part}: failed after ${s.steps.length} steps: ${e.message}`);
  }
}
await v.close();
process.exit(failed ? 1 : 0);
