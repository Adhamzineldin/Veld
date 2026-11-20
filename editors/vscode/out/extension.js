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
// A thin client: every language feature lives in `veld lsp`.
const vscode = __importStar(require("vscode"));
const node_1 = require("vscode-languageclient/node");
let client;
function serverOptions() {
    const command = vscode.workspace
        .getConfiguration("veld")
        .get("path", "veld")
        .trim() || "veld";
    return { command, args: ["lsp"] };
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
function activate(context) {
    startClient(context);
    context.subscriptions.push(vscode.workspace.onDidChangeConfiguration((e) => {
        if (e.affectsConfiguration("veld.path")) {
            void restartClient(context);
        }
    }));
}
async function deactivate() {
    await client?.stop();
}
//# sourceMappingURL=extension.js.map