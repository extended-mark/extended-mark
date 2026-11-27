// The ways in to the preview, and the file icons: the preview button in a document's title, Ctrl+Shift+V, Extended
// Mark Preview among the editors a document can be reopened with, and the explorer's menu.
//
// The file icons show in an icon theme that shows a language's own icon where it has none of its
// own, as Seti and Material Icon Theme do (VS Code's Minimal and Modern icons don't), so the run picks
// ICON_THEME (default Seti) as a person would. For Material Icon Theme, install it into a folder:
// EXTENSIONS=<that folder> ICON_THEME="Material Icon Theme".

const fail = (why) => {
  throw new Error(why);
};

export default async function ({ win, pause, shot, palette, line, file }) {
  /** The editor group's tab that reads `text`. */
  const tab = (text) => win.locator(".tab", { hasText: text });
  await file("poll.xtmd", "@tally");

  // 1. the file icons: a page for a document, a box for a pack, in an icon theme that shows them
  await win.keyboard.press("Control+Shift+P");
  await win.locator(".quick-input-widget input").waitFor();
  await win.keyboard.type("Preferences: File Icon Theme", { delay: 15 });
  await pause(600);
  await win.keyboard.press("Enter");
  await pause(1000);
  await win.keyboard.type(process.env.ICON_THEME ?? "Seti", { delay: 30 });
  await pause(1200);
  await win.keyboard.press("Enter");
  await pause(3000);
  for (const folder of ["kit", "docs"]) {
    const row = win.locator(".explorer-folders-view .monaco-list-row", { hasText: new RegExp(`^${folder}$`) }).first();
    if ((await row.getAttribute("aria-expanded")) !== "true") await row.click();
  }
  await win.locator(".explorer-folders-view .monaco-list-row", { hasText: "kit.xtmdpack" }).waitFor();
  {
    const icons = await win.locator(".explorer-folders-view .monaco-list-row .monaco-icon-label").evaluateAll((ls) =>
      ls.map((l) => [l.getAttribute("aria-label")?.split(",")[0] ?? "", getComputedStyle(l, "::before").backgroundImage]),
    );
    for (const [name, image] of icons.filter(([n]) => /\.xtmd(pack)?$/.test(n))) {
      const want = name.endsWith("pack") ? "xtmdpack-" : "xtmd-";
      if (!image.includes(want)) fail(`${name}'s icon is ${image}, not media/${want}*.svg`);
    }
  }
  await win.locator(".explorer-folders-view .monaco-list-row", { hasText: "poll.xtmd" }).first().click();
  await shot("p1-file-icons", "The explorer: poll.xtmd has the document's icon, a page with the @; kit.xtmdpack the pack's, a box with the @");

  // 2. the preview button at the top right of the document
  const button = win.locator(".editor-actions .action-label[aria-label^='Open Preview to the Side']").first();
  await button.waitFor({ timeout: 10000 });
  await button.hover();
  await pause(1200);
  await shot("p2-button", "The preview button at the document's top right, its tooltip showing");
  await button.click();
  await tab("Preview poll.xtmd").waitFor({ timeout: 15000 });
  await pause(5000);
  await shot("p3-beside", "The button opens the preview beside the document");
  await tab("Preview poll.xtmd").click();
  await win.keyboard.press("Control+W");
  await pause(800);

  // 3. Ctrl+Shift+V: the preview in the document's place, as Markdown's own
  await win.locator(".view-line", { hasText: "@tally" }).first().click();
  await win.keyboard.press("Control+Shift+V");
  await tab("Preview poll.xtmd").waitFor({ timeout: 15000 });
  await pause(5000);
  await shot("p4-ctrl-shift-v", "Ctrl+Shift+V: the preview opens in the document's place");
  await win.keyboard.press("Control+W");
  await pause(800);

  // 4. Reopen Editor With…: Extended Mark Preview is one of the document's editors. The editor's own
  // menu of them (Text Editor ⌄, right of the breadcrumbs, there for an editor opened once the
  // extension runs) lists the same; it is a native menu, which a screenshot doesn't catch.
  await win.locator(".explorer-folders-view .monaco-list-row", { hasText: "poll.xtmd" }).first().click();
  await win.locator(".breadcrumbs-editor-type").first().waitFor({ timeout: 10000 });
  await win.keyboard.press("Control+Shift+P");
  await win.locator(".quick-input-widget input").waitFor();
  await win.keyboard.type("View: Reopen Editor With...", { delay: 15 });
  await pause(600);
  await win.locator(".quick-input-list .monaco-list-row", { hasText: "Reopen Editor With..." }).first().click();
  const choice = win.locator(".quick-input-list .monaco-list-row", { hasText: "Extended Mark Preview" }).first();
  await choice.waitFor({ timeout: 10000 });
  await pause(600);
  await shot("p5-reopen-with", "Reopen Editor With…: Extended Mark Preview among the document's editors (the Text Editor ⌄ menu at the top right lists the same)");
  await choice.click();
  await pause(6000);
  if (!(await win.frameLocator("iframe.webview").first().frameLocator("iframe").locator("[data-xtmd-root]").count())) fail("the preview editor drew nothing");
  await shot("p6-preview-editor", "Extended Mark Preview as the document's editor: the page, drawn in VS Code's own components");
  // as it was: VS Code opens a document again in the editor it was last open in
  await palette("View: Reopen Editor with Text Editor");
  await line("@tally").waitFor({ timeout: 15000 });
  await win.keyboard.press("Control+W");
  await pause(800);

  // 5. the explorer's menu on a document
  await win.locator(".explorer-folders-view .monaco-list-row", { hasText: "poll.xtmd" }).first().click({ button: "right" });
  const item = win.locator(".context-view .action-label", { hasText: "Open Preview to the Side" });
  await item.waitFor({ timeout: 10000 });
  await item.hover();
  await shot("p7-explorer-menu", "The explorer's menu on poll.xtmd: Open Preview to the Side");
  await win.keyboard.press("Escape");
}
