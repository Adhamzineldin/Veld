// Language features live in `veld lsp`. This client also adds the CLI commands (validate, generate,
// dry run, setup, export, watch), an output channel, a status-bar item and a problem matcher.
import { spawn } from "child_process";
import * as path from "path";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";
import {
  EXPORT_FORMATS,
  RUN_COMMANDS,
  VeldCommand,
  WATCH,
  executable,
  exportArgs,
  exportFileName,
  parseSummary,
  shellQuote,
} from "./cli";

let client: LanguageClient | undefined;
let output: vscode.OutputChannel;
let status: vscode.StatusBarItem;
let running = false;

function veldPath(): string {
  return executable(vscode.workspace.getConfiguration("veld").get<string>("path"));
}

function serverOptions(): ServerOptions {
  return { command: veldPath(), args: ["lsp"] };
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

/**
 * The directory to run `veld` in: the folder of the active file when it is a Veld file, else the
 * only (or a picked) folder that holds a `veld.toml`, else the first workspace folder.
 * `veld` itself searches upward for `veld.toml`.
 */
async function projectDir(): Promise<string | undefined> {
  const active = vscode.window.activeTextEditor?.document;
  if (active && active.uri.scheme === "file" && /\.veld$|veld\.toml$/.test(active.fileName)) {
    return path.dirname(active.fileName);
  }
  const configs = await vscode.workspace.findFiles("**/veld.toml", "**/node_modules/**", 20);
  if (configs.length === 1) {
    return path.dirname(configs[0].fsPath);
  }
  if (configs.length > 1) {
    const picked = await vscode.window.showQuickPick(
      configs.map((c) => ({ label: vscode.workspace.asRelativePath(c), dir: path.dirname(c.fsPath) })),
      { placeHolder: "Which Veld project?" },
    );
    return picked?.dir;
  }
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) {
    void vscode.window.showErrorMessage("Open a folder that contains a veld.toml first.");
  }
  return folder?.uri.fsPath;
}

/** Runs `veld <args>` and streams its output into the channel. Resolves with the exit code and text. */
function runVeld(args: string[], cwd: string): Promise<{ code: number; text: string }> {
  const exe = veldPath();
  const all = ["--no-color", ...args];
  output.appendLine(`$ ${[exe, ...all].map(shellQuote).join(" ")}`);
  return new Promise((resolve) => {
    let text = "";
    const child = spawn(exe, all, { cwd, shell: false });
    const sink = (chunk: Buffer): void => {
      const s = chunk.toString();
      text += s;
      output.append(s);
    };
    child.stdout.on("data", sink);
    child.stderr.on("data", sink);
    child.on("error", (err) => {
      const message = `Could not run \`${exe}\` (${err.message}). Install Veld or set "veld.path".`;
      output.appendLine(message);
      void vscode.window.showErrorMessage(message);
      resolve({ code: 127, text });
    });
    child.on("close", (code) => {
      output.appendLine("");
      resolve({ code: code ?? 1, text });
    });
  });
}

function setStatus(text: string, tooltip: string, failed = false): void {
  status.text = text;
  status.tooltip = tooltip;
  status.backgroundColor = failed
    ? new vscode.ThemeColor("statusBarItem.errorBackground")
    : undefined;
  status.show();
}

async function runCommand(command: VeldCommand, quietOnSuccess = false): Promise<number | undefined> {
  if (running) {
    void vscode.window.showInformationMessage("Veld is already running a command.");
    return undefined;
  }
  const cwd = await projectDir();
  if (!cwd) {
    return undefined;
  }
  running = true;
  setStatus("$(sync~spin) Veld", `Running veld ${command.args.join(" ")}`);
  if (!quietOnSuccess) {
    output.show(true);
  }
  try {
    const { code, text } = await runVeld(command.args, cwd);
    const summary = parseSummary(text);
    if (code === 0) {
      setStatus("$(check) Veld", `veld ${command.args.join(" ")} succeeded`);
      if (!quietOnSuccess) {
        void vscode.window.showInformationMessage(`veld ${command.args.join(" ")} succeeded.`);
      }
    } else {
      const detail = summary ? `${summary.errors} error(s), ${summary.warnings} warning(s)` : `exit code ${code}`;
      setStatus(`$(error) Veld`, `veld ${command.args.join(" ")} failed: ${detail}`, true);
      if (!quietOnSuccess) {
        void vscode.window.showErrorMessage(`veld ${command.args.join(" ")} failed (${detail}). See the Veld output.`);
      }
    }
    return code;
  } finally {
    running = false;
  }
}

async function exportCommand(format: string | undefined): Promise<void> {
  const chosen =
    format ?? (await vscode.window.showQuickPick([...EXPORT_FORMATS], { placeHolder: "Export format" }));
  if (!chosen) {
    return;
  }
  const cwd = await projectDir();
  if (!cwd) {
    return;
  }
  const target = await vscode.window.showSaveDialog({
    defaultUri: vscode.Uri.file(path.join(cwd, exportFileName(chosen))),
    saveLabel: `Export ${chosen}`,
  });
  if (!target) {
    return;
  }
  output.show(true);
  const { code } = await runVeld(exportArgs(chosen, target.fsPath), cwd);
  if (code === 0) {
    await vscode.window.showTextDocument(target);
  } else {
    void vscode.window.showErrorMessage(`veld export ${chosen} failed. See the Veld output.`);
  }
}

/** The "Veld: Watch" command is a task, so it runs in the terminal panel and can be stopped there. */
function watchTask(): vscode.Task {
  const definition = { type: "veld", command: "watch" };
  const exe = veldPath();
  const task = new vscode.Task(
    definition,
    vscode.TaskScope.Workspace,
    "watch",
    "veld",
    new vscode.ShellExecution([shellQuote(exe), ...WATCH.args].join(" ")),
    "$veld",
  );
  task.isBackground = true;
  task.group = vscode.TaskGroup.Build;
  return task;
}

function taskFor(definition: { command?: string }): vscode.Task | undefined {
  const exe = veldPath();
  if (definition.command === "watch") {
    return watchTask();
  }
  const found = RUN_COMMANDS.find((c) => c.args.join(" ") === definition.command);
  if (!found) {
    return undefined;
  }
  return new vscode.Task(
    { type: "veld", command: definition.command },
    vscode.TaskScope.Workspace,
    found.args.join(" "),
    "veld",
    new vscode.ShellExecution([shellQuote(exe), "--no-color", ...found.args].join(" ")),
    "$veld",
  );
}

function registerCommands(context: vscode.ExtensionContext): void {
  const sub = context.subscriptions;
  for (const command of RUN_COMMANDS) {
    sub.push(vscode.commands.registerCommand(command.id, () => runCommand(command)));
  }
  sub.push(
    vscode.commands.registerCommand("veld.exportOpenapi", () => exportCommand("openapi")),
    vscode.commands.registerCommand("veld.export", () => exportCommand(undefined)),
    vscode.commands.registerCommand("veld.watch", () => vscode.tasks.executeTask(watchTask())),
    vscode.commands.registerCommand("veld.restartServer", () => restartClient(context)),
    vscode.commands.registerCommand("veld.showOutput", () => output.show()),
    vscode.tasks.registerTaskProvider("veld", {
      provideTasks: () => [watchTask(), ...RUN_COMMANDS.map((c) => taskFor({ command: c.args.join(" ") }))].filter(
        (t): t is vscode.Task => t !== undefined,
      ),
      resolveTask: (task) => taskFor(task.definition as { command?: string }),
    }),
  );
}

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel("Veld");
  status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0);
  status.command = "veld.validate";
  setStatus("$(symbol-structure) Veld", "Validate the contracts (veld check)");
  context.subscriptions.push(output, status);

  registerCommands(context);
  startClient(context);

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("veld.path")) {
        void restartClient(context);
      }
    }),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      const enabled = vscode.workspace.getConfiguration("veld").get<boolean>("validateOnSave", true);
      if (enabled && doc.languageId === "veld") {
        void runCommand(RUN_COMMANDS[0], true);
      }
    }),
  );
}

export async function deactivate(): Promise<void> {
  await client?.stop();
}
