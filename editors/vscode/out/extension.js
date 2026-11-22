"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
// Language features live in `veld lsp`. This client also adds the CLI commands (validate, generate,
// dry run, setup, export, watch), an output channel, a status-bar item and a problem matcher.
const child_process_1 = require("child_process");
const path = __importStar(require("path"));
const vscode = __importStar(require("vscode"));
const node_1 = require("vscode-languageclient/node");
const cli_1 = require("./cli");
let client;
let output;
let status;
let running = false;
function veldPath() {
    return (0, cli_1.executable)(vscode.workspace.getConfiguration("veld").get("path"));
}
function serverOptions() {
    return { command: veldPath(), args: ["lsp"] };
}
function startClient(context) {
    const clientOptions = {
        documentSelector: [{ scheme: "file", language: "veld" }],
        synchronize: {
            fileEvents: [
                vscode.workspace.createFileSystemWatcher("**/*.veld"),
                vscode.workspace.createFileSystemWatcher("**/veld.toml"),
            ],
        },
    };
    client = new node_1.LanguageClient("veld", "Veld Language Server", serverOptions(), clientOptions);
    context.subscriptions.push(client);
    client.start().catch((err) => {
        const detail = err instanceof Error ? err.message : String(err);
        void vscode.window.showErrorMessage(`Could not start \`veld lsp\` (${detail}). Install Veld or set "veld.path".`);
    });
}
async function restartClient(context) {
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
async function projectDir() {
    const active = vscode.window.activeTextEditor?.document;
    if (active && active.uri.scheme === "file" && /\.veld$|veld\.toml$/.test(active.fileName)) {
        return path.dirname(active.fileName);
    }
    const configs = await vscode.workspace.findFiles("**/veld.toml", "**/node_modules/**", 20);
    if (configs.length === 1) {
        return path.dirname(configs[0].fsPath);
    }
    if (configs.length > 1) {
        const picked = await vscode.window.showQuickPick(configs.map((c) => ({ label: vscode.workspace.asRelativePath(c), dir: path.dirname(c.fsPath) })), { placeHolder: "Which Veld project?" });
        return picked?.dir;
    }
    const folder = vscode.workspace.workspaceFolders?.[0];
    if (!folder) {
        void vscode.window.showErrorMessage("Open a folder that contains a veld.toml first.");
    }
    return folder?.uri.fsPath;
}
/** Runs `veld <args>` and streams its output into the channel. Resolves with the exit code and text. */
function runVeld(args, cwd) {
    const exe = veldPath();
    const all = ["--no-color", ...args];
    output.appendLine(`$ ${[exe, ...all].map(cli_1.shellQuote).join(" ")}`);
    return new Promise((resolve) => {
        let text = "";
        const child = (0, child_process_1.spawn)(exe, all, { cwd, shell: false });
        const sink = (chunk) => {
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
function setStatus(text, tooltip, failed = false) {
    status.text = text;
    status.tooltip = tooltip;
    status.backgroundColor = failed
        ? new vscode.ThemeColor("statusBarItem.errorBackground")
        : undefined;
    status.show();
}
async function runCommand(command, quietOnSuccess = false) {
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
        const summary = (0, cli_1.parseSummary)(text);
        if (code === 0) {
            setStatus("$(check) Veld", `veld ${command.args.join(" ")} succeeded`);
            if (!quietOnSuccess) {
                void vscode.window.showInformationMessage(`veld ${command.args.join(" ")} succeeded.`);
            }
        }
        else {
            const detail = summary ? `${summary.errors} error(s), ${summary.warnings} warning(s)` : `exit code ${code}`;
            setStatus(`$(error) Veld`, `veld ${command.args.join(" ")} failed: ${detail}`, true);
            if (!quietOnSuccess) {
                void vscode.window.showErrorMessage(`veld ${command.args.join(" ")} failed (${detail}). See the Veld output.`);
            }
        }
        return code;
    }
    finally {
        running = false;
    }
}
async function exportCommand(format) {
    const chosen = format ?? (await vscode.window.showQuickPick([...cli_1.EXPORT_FORMATS], { placeHolder: "Export format" }));
    if (!chosen) {
        return;
    }
    const cwd = await projectDir();
    if (!cwd) {
        return;
    }
    const target = await vscode.window.showSaveDialog({
        defaultUri: vscode.Uri.file(path.join(cwd, (0, cli_1.exportFileName)(chosen))),
        saveLabel: `Export ${chosen}`,
    });
    if (!target) {
        return;
    }
    output.show(true);
    const { code } = await runVeld((0, cli_1.exportArgs)(chosen, target.fsPath), cwd);
    if (code === 0) {
        await vscode.window.showTextDocument(target);
    }
    else {
        void vscode.window.showErrorMessage(`veld export ${chosen} failed. See the Veld output.`);
    }
}
/** The "Veld: Watch" command is a task, so it runs in the terminal panel and can be stopped there. */
function watchTask() {
    const definition = { type: "veld", command: "watch" };
    const exe = veldPath();
    const task = new vscode.Task(definition, vscode.TaskScope.Workspace, "watch", "veld", new vscode.ShellExecution([(0, cli_1.shellQuote)(exe), ...cli_1.WATCH.args].join(" ")), "$veld");
    task.isBackground = true;
    task.group = vscode.TaskGroup.Build;
    return task;
}
function taskFor(definition) {
    const exe = veldPath();
    if (definition.command === "watch") {
        return watchTask();
    }
    const found = cli_1.RUN_COMMANDS.find((c) => c.args.join(" ") === definition.command);
    if (!found) {
        return undefined;
    }
    return new vscode.Task({ type: "veld", command: definition.command }, vscode.TaskScope.Workspace, found.args.join(" "), "veld", new vscode.ShellExecution([(0, cli_1.shellQuote)(exe), "--no-color", ...found.args].join(" ")), "$veld");
}
function registerCommands(context) {
    const sub = context.subscriptions;
    for (const command of cli_1.RUN_COMMANDS) {
        sub.push(vscode.commands.registerCommand(command.id, () => runCommand(command)));
    }
    sub.push(vscode.commands.registerCommand("veld.exportOpenapi", () => exportCommand("openapi")), vscode.commands.registerCommand("veld.export", () => exportCommand(undefined)), vscode.commands.registerCommand("veld.watch", () => vscode.tasks.executeTask(watchTask())), vscode.commands.registerCommand("veld.restartServer", () => restartClient(context)), vscode.commands.registerCommand("veld.showOutput", () => output.show()), vscode.tasks.registerTaskProvider("veld", {
        provideTasks: () => [watchTask(), ...cli_1.RUN_COMMANDS.map((c) => taskFor({ command: c.args.join(" ") }))].filter((t) => t !== undefined),
        resolveTask: (task) => taskFor(task.definition),
    }));
}
function activate(context) {
    output = vscode.window.createOutputChannel("Veld");
    status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0);
    status.command = "veld.validate";
    setStatus("$(symbol-structure) Veld", "Validate the contracts (veld check)");
    context.subscriptions.push(output, status);
    registerCommands(context);
    startClient(context);
    context.subscriptions.push(vscode.workspace.onDidChangeConfiguration((e) => {
        if (e.affectsConfiguration("veld.path")) {
            void restartClient(context);
        }
    }), vscode.workspace.onDidSaveTextDocument((doc) => {
        const enabled = vscode.workspace.getConfiguration("veld").get("validateOnSave", true);
        if (enabled && doc.languageId === "veld") {
            void runCommand(cli_1.RUN_COMMANDS[0], true);
        }
    }));
}
async function deactivate() {
    await client?.stop();
}
//# sourceMappingURL=extension.js.map