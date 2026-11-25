// The editor: hover, completion, errors, definitions, the outline, formatting, and the preview with a button pressed.

export default async function ({ win, pause, shot, palette, line, word, file, theme }) {
  // 1. the document: highlighting, and the server's errors (none)
  await file("poll.xtmd", "@tally");
  await win.locator(".statusbar-item", { hasText: "Extended Mark" }).first().waitFor({ timeout: 30000 });
  await pause(1500);
  await shot("01-document", "poll.xtmd open: code above @@@, Markdown and widget calls below, highlighted; the status bar says no errors");

  // 2. hover: a pack's widget, how it is declared and its doc
  // the cursor on the name, then Show Hover (Ctrl+K Ctrl+I), as the keyboard does it
  await word('@tally(name = "Yes"', "tally").click();
  await win.keyboard.press("Control+K");
  await win.keyboard.press("Control+I");
  await win.locator(".monaco-hover-content:visible", { hasText: "widget tally" }).first().waitFor({ timeout: 30000 });
  await shot("02-hover-widget", "Hover on @tally: the widget kit declares, with its inputs and defaults, and its doc comment");

  // 3. hover: core's press
  await win.keyboard.press("Escape");
  await pause(300);
  await word("@press(", "press").click();
  await win.keyboard.press("Control+K");
  await win.keyboard.press("Control+I");
  await win.locator(".monaco-hover-content:visible", { hasText: "extern widget press" }).first().waitFor({ timeout: 30000 });
  await shot("03-hover-core", "Hover on @press: core's own declaration and its doc, from the resolved IR with core in it");

  // 4. completion after @: the widgets there are, each with what it needs
  await win.keyboard.press("Escape");
  await win.keyboard.press("Control+End");
  await win.keyboard.press("Enter");
  await win.keyboard.press("Enter");
  await win.keyboard.type("@tal", { delay: 60 });
  await win.keyboard.press("Control+Space");
  await win.locator(".suggest-widget .monaco-list-row").first().waitFor({ timeout: 15000 });
  await shot("04-complete-widget", "Typing @tal: completion offers tally, kit's widget, with how it is declared");
  await win.keyboard.press("Enter");
  await pause(500);
  await shot("05-complete-snippet", "Accepting it writes the call with its required input as a stop to fill in");

  // 5. an error as you type, with its rule's link
  await win.keyboard.type("3", { delay: 60 });
  await pause(2500);
  await win.keyboard.press("Control+Shift+M");
  await pause(1500);
  await shot("06-error", "name = 3: the server's error under it and in Problems, with its code linking to the spec's rule");
  await win.keyboard.press("Control+Shift+M");

  // back to the document as it was
  await palette("File: Revert File");
  await pause(1500);

  // 5b. a slip, and the quick fix that writes the name meant
  await word("@press(", "press").click();
  await win.keyboard.press("End");
  await win.keyboard.press("Home");
  await win.keyboard.press("Shift+End");
  await win.keyboard.type('@prss(label = "I have read the rules", kind = press_kind.CHECK, selected = $state.agree, click = fn() {', { delay: 10 });
  await pause(2500);
  await word("@prss(", "prss").click();
  await win.keyboard.press("Control+.");
  await win.locator(".action-widget", { hasText: "Change to" }).first().waitFor({ timeout: 15000 });
  await shot("06b-quick-fix", "@prss: the error says did you mean press?, and Ctrl+. offers the fix");
  await win.keyboard.press("Enter");
  await pause(2000);
  await shot("06c-fixed", "The fix applied: @press, and no error left");
  await palette("File: Revert File");
  await pause(1500);

  // 6. go to definition: into the pack
  await word('@tally(name = "No"', "tally").click();
  await win.keyboard.press("F12");
  await win.locator(".tab.active", { hasText: "kit.xtmdpack" }).waitFor({ timeout: 15000 });
  await shot("07-definition", "F12 on tally: kit.xtmdpack opens at the widget's declaration");

  // 7. the outline of the pack
  await palette("Explorer: Focus on Outline View");
  await pause(1500);
  await shot("08-outline", "The outline of kit.xtmdpack: its pack and widget, and the widget's inputs");

  // 8. format document
  await win.keyboard.press("Control+Home");
  await win.keyboard.press("Control+End");
  await win.keyboard.press("Control+Shift+Tab");
  await win.keyboard.press("Control+P");
  await win.keyboard.type("poll.xtmd", { delay: 20 });
  await pause(500);
  await win.keyboard.press("Enter");
  await pause(800);
  await line("yes: int").click();
  await win.keyboard.press("Home");
  await win.keyboard.type("      ", { delay: 20 });
  await pause(800);
  await shot("09-messy", "Indentation broken by hand");
  await palette("Format Document");
  await pause(1500);
  await shot("10-formatted", "Format Document: xtmd's own layout back");

  // 9. the preview: the vscode renderer, VS Code's components in the editor's theme
  await palette("Extended Mark: Open Preview to the Side");
  const webview = async () => {
    for (let k = 0; k < 60; k++) {
      for (const f of win.frames()) {
        try {
          if (await f.locator("[data-xtmd-root]").count()) return f;
        } catch {}
      }
      await pause(500);
    }
    throw new Error("the preview didn't draw");
  };
  const preview = await webview();
  await pause(1500);
  await shot("11-preview", "The preview beside the editor: the page drawn by the vscode renderer");
  const vote = preview.locator("button", { hasText: "Vote yes" });
  if (await vote.isEnabled()) throw new Error("a vote is open before the rules are read");
  await preview.locator("button", { hasText: "I have read the rules" }).click();
  await vote.click();
  await vote.click();
  await preview.locator("text=5 votes").waitFor({ timeout: 10000 });
  await preview.locator("text=83%").first().waitFor({ timeout: 10000 });
  await shot("12-preview-pressed", "The checkbox ticked and Yes voted for twice: the count, its bar and the share follow");

  // 10. a draft that doesn't compile: the preview keeps the last page that did, and says why
  await line("Should the team").click();
  await win.keyboard.press("End");
  await win.keyboard.type(" \\{$state.nope}", { delay: 30 });
  await pause(2000);
  await shot("13-preview-broken", "A broken draft: the preview keeps the last page and says why at its top");
  await palette("File: Revert File");
  await pause(1500);

  // 11. the light theme: the preview follows the editor
  theme("Default Light Modern");
  await pause(4000);
  await shot("14-light", "The editor in Light Modern: the preview's components follow its theme");
  theme();
}
