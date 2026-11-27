// The extension in a browser (vscode.dev, github.dev): the language server built in, in a web
// worker.

import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/browser";
import { activate as start } from "./common.js";

export function activate(context: vscode.ExtensionContext): Promise<void> {
  return start(
    context,
    async (options) => {
      const worker = new Worker(vscode.Uri.joinPath(context.extensionUri, "dist", "web", "worker.js").toString(true));
      return { client: new LanguageClient("xtmd", "Extended Mark", worker, options), server: "built in (xtmd.wasm, in a web worker)" };
    },
  );
}

export function deactivate(): void {}
