// The example host: the server's HTML works before any script and holds the whole page; the browser takes
// the same elements over; and each change touches only the elements it is about.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";
import { render } from "../server.mjs";

const here = (f) => fileURLToPath(new URL(f, import.meta.url));
const ir = JSON.parse(readFileSync(here("../dist/page.ir.json"), "utf8"));
const client = readFileSync(here("../dist/client.js"), "utf8");

async function open(html, opts = {}) {
  const browser = await chromium.launch({ executablePath: process.env.XTMD_CHROMIUM || "/usr/bin/chromium", args: ["--no-sandbox"] });
  const context = await browser.newContext(opts);
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e));
  await page.route("http://host.test/**", (r) => {
    const js = r.request().url().endsWith("client.js");
    return r.fulfill({ status: 200, contentType: js ? "text/javascript" : "text/html", body: js ? client : html });
  });
  await page.goto("http://host.test/");
  return { browser, page, errors };
}

test("nothing of ours draws the page, and what it doesn't draw is its Markdown", () => {
  for (const f of ["../draw.mjs", "../server.mjs", "../client.mjs"]) {
    const uses = [...readFileSync(here(f), "utf8").matchAll(/from "([^"]+)"/g)].map((m) => m[1]).filter((m) => m.startsWith("@extended-mark/"));
    assert.ok(uses.every((m) => m === "@extended-mark/runtime"), `${f} uses ${uses}`);
  }
  const html = render(ir);
  assert.match(html, /This table/);
  assert.ok(!html.includes("<table"));
});

test("the server's page is whole, and reads with no script at all", async () => {
  const { browser, page } = await open(render(ir), { javaScriptEnabled: false });
  try {
    assert.equal(await page.textContent("h1"), "Tasks for Tuesday");
    assert.equal(await page.textContent("strong"), "2");
    assert.deepEqual(await page.locator("[role=checkbox]").allTextContents(), ["Read the spec", "Write a host"]);
    assert.equal(await page.getAttribute("a", "href"), "https://extendedmark.org");
    assert.equal(await page.getAttribute("input[type=range]", "max"), "5");
  } finally {
    await browser.close();
  }
});

test("the browser takes the server's elements over, and a change touches only what it is about", async () => {
  const { browser, page, errors } = await open(render(ir));
  try {
    await page.waitForFunction(() => window.host);
    assert.deepEqual(await page.evaluate(() => window.host.missing), []);
    // every element the server wrote is still the one on the page
    await page.evaluate(() => {
      window.before = [...document.querySelectorAll("#page [data-id]")];
      window.seen = [];
      new MutationObserver((ms) => window.seen.push(...ms.map((m) => (m.target.closest?.("[data-id]") ?? m.target.parentElement.closest("[data-id]"))?.dataset.id ?? ""))).observe(document.getElementById("page"), { subtree: true, childList: true, characterData: true, attributes: true });
      window.calls = [];
      document.addEventListener("host-call", (e) => window.calls.push(e.detail));
    });
    const second = page.locator("[role=checkbox]").nth(1);
    await second.evaluate((el) => (el.mine = true));
    await second.click();
    assert.equal(await second.getAttribute("aria-checked"), "true");
    assert.deepEqual(await page.evaluate(() => window.calls), [{ call: { name: "doc/clock.finished", args: [{ text: "Write a host" }] } }]);
    // one element, and one attribute of it
    assert.deepEqual(await page.evaluate(() => [...new Set(window.seen)]), [await second.getAttribute("data-id")]);
    assert.ok(await page.evaluate(() => window.before.every((el) => el.isConnected)));

    // a row that goes leaves the others, and what is in them, as they were
    await page.getByText("More").nth(1).click();
    const title = page.locator("input[aria-label=Title]");
    await title.fill("Write a hostess");
    await title.focus();
    await page.evaluate(() => document.activeElement.setSelectionRange(3, 3));
    await page.evaluate(() => [...document.querySelectorAll("button")].find((b) => b.textContent === "Drop").click());
    assert.deepEqual(await page.locator("[role=checkbox]").allTextContents(), ["Write a hostess"]);
    assert.equal(await page.locator("[role=checkbox]").evaluate((el) => el.mine), true);
    assert.deepEqual(await page.evaluate(() => [document.activeElement.value, document.activeElement.selectionStart]), ["Write a hostess", 3]);
    assert.equal(await page.textContent("strong"), "1");

    // a row added, a write that fits, one that doesn't
    const next = page.locator("input[aria-label=Next]");
    await next.fill("Test it");
    await next.press("Enter");
    assert.deepEqual(await page.locator("[role=checkbox]").allTextContents(), ["Write a hostess", "Test it"]);
    assert.equal(await next.inputValue(), "");
    await page.locator("input[type=range]").fill("5");
    assert.deepEqual(await page.evaluate(() => window.host.page.state().size), { num: "5" });

    // a widget that fails says why in its place, and is drawn again once it can be
    await page.evaluate(() => window.host.page.expose("doc/clock", "clear"));
    assert.equal(await page.textContent("[role=alert]"), "share failed: a division by zero");
    await next.fill("Again");
    await next.press("Enter");
    assert.equal(await page.locator("[role=alert]").count(), 0);
    assert.equal(await page.locator(".label").last().textContent(), "Each task is 100 percent");
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});

test("a reader's data comes back on the server, and the browser goes on from it", async () => {
  const first = await open(render(ir));
  let data;
  try {
    await first.page.waitForFunction(() => window.host);
    await first.page.getByText("More").first().click();
    data = JSON.parse(await first.page.evaluate(() => JSON.stringify(window.host.page.data())));
  } finally {
    await first.browser.close();
  }
  const html = render(ir, data);
  assert.match(html, /aria-label="Title"/);
  const { browser, page, errors } = await open(html);
  try {
    await page.waitForFunction(() => window.host);
    assert.deepEqual(await page.evaluate(() => window.host.missing), []);
    assert.equal(await page.locator("input[aria-label=Title]").inputValue(), "Read the spec");
    await page.getByText("Less").first().click();
    assert.equal(await page.locator("input[aria-label=Title]").count(), 0);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
