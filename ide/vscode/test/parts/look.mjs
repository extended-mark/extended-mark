// Every control the preview draws, to hold against VS Code's own components: the page docs/controls.xtmd in the preview.

const fail = (why) => {
  throw new Error(why);
};

export default async function ({ win, pause, shot, file }) {
  /** The page in the preview's webview. */
  const page = () => win.frameLocator("iframe.webview").first().frameLocator("iframe");
  await file("controls.xtmd", "Controls");
  await win.locator(".view-line", { hasText: "Controls" }).first().click();
  await win.keyboard.press("Control+Shift+V");
  await page().locator("[data-xtmd-root]").waitFor({ timeout: 30000 });
  await pause(3000);
  // the tick and the dot are the theme's own colour, and the first block is at the page's top
  for (const [kind, property] of [["check", "borderBottomColor"], ["radio", "backgroundColor"]]) {
    const [drawn, theme] = await page()
      .locator(`.vsc-${kind}.xtmd-selected .xtmd-press-indicator`)
      .first()
      .evaluate((e, p) => {
        const probe = document.body.appendChild(document.createElement("i"));
        probe.style.color = "var(--vscode-checkbox-foreground)";
        const colour = getComputedStyle(probe).color;
        probe.remove();
        return [getComputedStyle(e, "::after")[p], colour];
      }, property);
    if (drawn !== theme) fail(`a ${kind}'s mark is ${drawn}, and the theme's is ${theme}`);
  }
  const above = await page().locator("[data-xtmd-root] > [data-part]").first().evaluate((e) => getComputedStyle(e).marginTop);
  if (above !== "0px") fail(`the first block has ${above} above it`);
  await shot("1-buttons-choices", "Buttons, a checkbox, a switch, radios, tabs and list options");
  // the slider: hovered, then focused by the keyboard
  const slider = page().locator("input[type=range]").first();
  await slider.scrollIntoViewIfNeeded();
  await slider.hover();
  await pause(500);
  await shot("2-input-hover", "Input boxes and the slider, the pointer over the slider");
  await slider.focus();
  await win.keyboard.press("ArrowRight");
  await pause(400);
  await shot("3-slider-focus", "The slider focused from the keyboard and moved one step");
  await page().locator("text=Show more").first().click();
  await pause(800);
  await shot("4-overlay", "The popover open over the page");
  await page().locator("table").first().scrollIntoViewIfNeeded();
  await pause(400);
  await shot("5-text", "Markdown's blocks: text, a quote, a list, a table and code");
}
