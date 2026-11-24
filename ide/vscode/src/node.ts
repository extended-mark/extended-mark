// The extension on the desktop: the language server built in, xtmd.wasm in a Node process. Nothing
// needs to be installed.

import * as vscode from "vscode";
import { LanguageClient, TransportKind } from "vscode-languageclient/node";
import { activate as start } from "./common.js";

export function activate(context: vscode.ExtensionContext): Promise<void> {
  return start(
    context,
    async (options) => {
      const module = vscode.Uri.joinPath(context.extensionUri, "dist", "node", "server.js").fsPath;
      const run = { module, transport: TransportKind.ipc };
      return { client: new LanguageClient("xtmd", "Extended Mark", { run, debug: run }, options), server: "built in (xtmd.wasm)" };
    },
  );
}

export function deactivate(): void {}
