// A thin client: every language feature lives in `veld lsp`.
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

function serverOptions(): ServerOptions {
  const command = vscode.workspace
    .getConfiguration("veld")
    .get<string>("path", "veld")
    .trim() || "veld";
  return { command, args: ["lsp"] };
}

function startClient(context: vscode.ExtensionContext): void {
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "veld" }],
    synchronize: {
      fileEvents: [
        vscode.workspace.createFileSystemWatcher("**/*.veld"),
        vscode.workspace.createFileSystemWatcher("**/veld.toml"),
      ],
    },
  };
  client = new LanguageClient("veld", "Veld Language Server", serverOptions(), clientOptions);
  context.subscriptions.push(client);
  client.start().catch((err: unknown) => {
    const detail = err instanceof Error ? err.message : String(err);
    void vscode.window.showErrorMessage(
      `Could not start \`veld lsp\` (${detail}). Install Veld or set "veld.path".`,
    );
  });
}

async function restartClient(context: vscode.ExtensionContext): Promise<void> {
  if (client) {
    const old = client;
    client = undefined;
    await old.stop().catch(() => undefined);
  }
  startClient(context);
}

export function activate(context: vscode.ExtensionContext): void {
  startClient(context);
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("veld.path")) {
        void restartClient(context);
      }
    }),
  );
}

export async function deactivate(): Promise<void> {
  await client?.stop();
}
