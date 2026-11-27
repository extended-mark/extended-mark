// Real VS Code (downloaded by @vscode/test-electron into .vscode-test) with this extension loaded, on a copy of
// test/workspace, driven by Playwright. One is opened for a whole run (drive.mjs): each part starts in it with no
// editor open.

import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { downloadAndUnzipVSCode } from "@vscode/test-electron";
import { _electron as electron } from "playwright-core";

const here = dirname(fileURLToPath(import.meta.url));
const [WIDTH, HEIGHT] = (process.env.SIZE ?? "1600x1000").split("x").map(Number);
/** How long each step stays on the screen, in milliseconds. */
const STEP = Number(process.env.STEP ?? 400);
const THEME = process.env.THEME ?? "Default Dark Modern";

export async function open() {
  const user = mkdtempSync(join(tmpdir(), "xtmd-vscode-"));
  const workspace = join(user, "workspace");
  cpSync(join(here, "workspace"), workspace, { recursive: true });
  const settingsFile = join(user, "data", "User", "settings.json");
  const settings = {
    "workbench.colorTheme": THEME,
    "workbench.startupEditor": "none",
    "workbench.tips.enabled": false,
    "workbench.enableExperiments": false,
    "security.workspace.trust.enabled": false,
    "telemetry.telemetryLevel": "off",
    "update.mode": "none",
    "extensions.ignoreRecommendations": true,
    "editor.minimap.enabled": false,
    "editor.fontSize": 15,
    "editor.hover.delay": 200,
    "window.titleBarStyle": "custom",
    "window.menuStyle": "custom",
    "window.dialogStyle": "custom",
    "chat.commandCenter.enabled": false,
    "workbench.secondarySideBar.defaultVisibility": "hidden",
    "git.openRepositoryInParentFolders": "never",
    "git.enabled": false,
    "github.copilot.enable": { "*": false },
  };
  mkdirSync(dirname(settingsFile), { recursive: true });
  writeFileSync(settingsFile, JSON.stringify(settings));

  const code = await downloadAndUnzipVSCode("stable");
  const app = await electron.launch({
    executablePath: code,
    args: [
      `--extensionDevelopmentPath=${join(here, "..")}`,
      `--user-data-dir=${join(user, "data")}`,
      `--extensions-dir=${process.env.EXTENSIONS ?? join(user, "extensions")}`,
      "--disable-workspace-trust",
      "--skip-welcome",
      "--skip-release-notes",
      "--disable-gpu",
      "--no-sandbox",
      workspace,
      join(workspace, "docs", "poll.xtmd"),
    ],
    // no xtmd anywhere VS Code could find one: the extension has all it needs
    env: { ...process.env, PATH: "/usr/bin:/bin" },
    timeout: 120000,
  });
  const win = await app.firstWindow();
  await app.evaluate(({ BrowserWindow }, [w, h]) => {
    const window = BrowserWindow.getAllWindows()[0];
    window.setSize(w, h);
    window.center();
  }, [WIDTH, HEIGHT]);

  const pause = (ms) => win.waitForTimeout(ms);
  async function palette(command) {
    await win.keyboard.press("Control+Shift+P");
    await win.locator(".quick-input-widget input").waitFor();
    await win.keyboard.type(command, { delay: 15 });
    await pause(500);
    await win.keyboard.press("Enter");
  }
  /** The editor line holding `text`, as an element. */
  const line = (text) => win.locator(".view-line", { hasText: text }).first();
  const status = win.locator(".statusbar-item", { hasText: "Extended Mark" }).first();

  await win.locator(".monaco-workbench").waitFor({ timeout: 60000 });
  // the server compiles core once as it starts, then the document
  await status.waitFor({ timeout: 60000 });
  await line("@tally").waitFor({ timeout: 30000 });
  await pause(9000);

  return {
    app,
    win,
    workspace,
    pause,
    palette,
    line,
    /** The token on a line that is exactly `word`. */
    word: (l, w) => line(l).locator("span[class*='mtk']", { hasText: new RegExp(`^${w}$`) }).first(),
    theme(name = THEME) {
      writeFileSync(settingsFile, JSON.stringify({ ...JSON.parse(readFileSync(settingsFile, "utf8")), "workbench.colorTheme": name }));
    },
    /** A file opened by its name. */
    async file(name, text) {
      await win.keyboard.press("Control+P");
      await win.locator(".quick-input-widget input").waitFor();
      await win.keyboard.type(name, { delay: 20 });
      await pause(700);
      await win.keyboard.press("Enter");
      await win.locator(".tab.active", { hasText: name }).waitFor({ timeout: 15000 });
      if (text) await line(text).waitFor({ timeout: 30000 });
    },
    /** As a part finds it: no editor, nothing unsaved, the explorer's folders in view. */
    async fresh() {
      await win.keyboard.press("Escape");
      await palette("View: Close All Editors");
      await pause(500);
      const drop = win.locator(".monaco-dialog-box .monaco-button", { hasText: "Don't Save" });
      if (await drop.count()) await drop.first().click();
      await win.locator(".tab").first().waitFor({ state: "detached", timeout: 15000 });
      await palette("View: Show Explorer");
      const outline = win.locator(".pane-header[aria-expanded='true']", { hasText: "Outline" }).first();
      if (await outline.count()) await outline.click();
    },
    /** A part's screenshots, each a step of it, in test/out/<part>. */
    steps(part) {
      const out = join(here, "out", part);
      rmSync(out, { recursive: true, force: true });
      mkdirSync(out, { recursive: true });
      const steps = [];
      return {
        steps,
        async shot(name, what) {
          await pause(STEP);
          await win.screenshot({ path: join(out, `${name}.png`) });
          steps.push({ name, what });
          console.log(`${name}: ${what}`);
        },
        done: () => writeFileSync(join(out, "steps.json"), JSON.stringify(steps, null, 2)),
        failed: () => win.screenshot({ path: join(out, "failed.png") }).catch(() => {}),
      };
    },
    async close() {
      await app.close();
      rmSync(user, { recursive: true, force: true });
    },
  };
}
