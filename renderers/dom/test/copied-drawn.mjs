// Run by copied.test.mjs with the copied set: what draws a page is the copy, and not ours.

import { test } from "node:test";
import assert from "node:assert/strict";
import { launch, open } from "./browser.mjs";

test("the copied set is what draws the page", async () => {
  const browser = await launch();
  try {
    const page = await open(browser, "# A title\n\nSome words.\n");
    const drawn = await page.locator("#app .xtmd-heading, #app .xtmd-paragraph").count();
    assert.equal(drawn, 2);
    assert.equal(await page.locator('#app [data-set="copied"]').count() >= drawn, true);
  } finally {
    await browser.close();
  }
});
