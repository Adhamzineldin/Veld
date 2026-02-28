import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import { KEYWORDS, HTTP_METHODS, BUILTIN_TYPES, DIRECTIVES } from './veld-language-spec';

/**
 * PROFESSIONAL VELD LANGUAGE SERVER FOR VS CODE
 * Complete semantic analysis with proper import/module resolution
 *
 * Language constants are auto-generated from internal/language/constants.go
 * Run: go run cmd/generate-language/main.go
 */

// ── Interfaces ────────────────────────────────────────

interface VeldDocument {
    uri: vscode.Uri;
    content: string;
    models: Map<string, ModelDef>;
    modules: Map<string, ModuleDef>;
    enums: Map<string, EnumDef>;
    imports: Map<string, string>;
    workspaceFolder?: string;
}

interface ModelDef {
    name: string;
    line: number;
    fields: Map<string, string>;
}

interface ModuleDef {
    name: string;
    line: number;
    actions: Map<string, ActionDef>;
}

interface ActionDef {
    name: string;
    method: string;
    path: string;
    input?: string;
    output?: string;
}

interface EnumDef {
    name: string;
    line: number;
    values: string[];
}

// ── Language Server ──────────────────────────────────

export class VeldLanguageServer {
    private documents: Map<string, VeldDocument> = new Map();
    private diagnosticCollection: vscode.DiagnosticCollection;

    constructor() {
        this.diagnosticCollection = vscode.languages.createDiagnosticCollection('veld');
    }

    parseDocument(uri: vscode.Uri, content: string): VeldDocument {
        const doc: VeldDocument = {
            uri,
            content,
            models: new Map(),
            modules: new Map(),
            enums: new Map(),
            imports: new Map(),
            workspaceFolder: vscode.workspace.getWorkspaceFolder(uri)?.uri.fsPath
        };

        const lines = content.split('\n');

        for (let i = 0; i < lines.length; i++) {
            const line = lines[i].trim();

            // Parse imports: @models/user, @modules/auth, etc.
            if (line.startsWith('import')) {
                const match = line.match(/import\s+@([\w]+)\/([\w]+)/);
                if (match) {
                    const alias = match[1];   // "models", "modules", etc.
                    const name = match[2];    // "user", "auth", etc.
                    const importPath = `@${alias}/${name}`;
                    doc.imports.set(importPath, `./${alias}/${name}.veld`);
                }
            }

            if (line.startsWith('model')) {
                const match = line.match(/model\s+([A-Za-z_]\w*)/);
                if (match) {
                    const modelName = match[1];
                    const fields = new Map<string, string>();
                    let j = i + 1;
                    while (j < lines.length && lines[j].trim() !== '}') {
                        const fieldLine = lines[j].trim();
                        const fieldMatch = fieldLine.match(/^([a-z_]\w*):\s*(.+?)(?:\s*\/\/.*)?$/);
                        if (fieldMatch) fields.set(fieldMatch[1], fieldMatch[2]);
                        j++;
                    }
                    doc.models.set(modelName, { name: modelName, line: i, fields });
                }
            }

            if (line.startsWith('enum')) {
                const match = line.match(/enum\s+([A-Za-z_]\w*)/);
                if (match) {
                    const enumName = match[1];
                    const values: string[] = [];
                    let j = i + 1;
                    while (j < lines.length && lines[j].trim() !== '}') {
                        const enumLine = lines[j].trim();
                        const enumMatch = enumLine.match(/^([a-z_]\w*)/);
                        if (enumMatch) values.push(enumMatch[1]);
                        j++;
                    }
                    doc.enums.set(enumName, { name: enumName, line: i, values });
                }
            }

            if (line.startsWith('module')) {
                const match = line.match(/module\s+([A-Za-z_]\w*)/);
                if (match) {
                    const moduleName = match[1];
                    const actions = new Map<string, ActionDef>();
                    let j = i + 1;
                    let inAction = false;
                    let currentAction: Partial<ActionDef> = {};

                    while (j < lines.length && lines[j].trim() !== '}') {
                        const actionLine = lines[j].trim();
                        if (actionLine.startsWith('action')) {
                            if (inAction && currentAction.name) {
                                actions.set(currentAction.name, currentAction as ActionDef);
                            }
                            const actionMatch = actionLine.match(/action\s+([A-Za-z_]\w*)/);
                            if (actionMatch) {
                                inAction = true;
                                currentAction = { name: actionMatch[1] };
                            }
                        } else if (inAction && actionLine === '}') {
                            if (currentAction.name) {
                                actions.set(currentAction.name, currentAction as ActionDef);
                            }
                            inAction = false;
                        } else if (inAction) {
                            const dirMatch = actionLine.match(/^\s*(\w+):\s*(.+)/);
                            if (dirMatch) {
                                const [, key, value] = dirMatch;
                                if (key === 'method') currentAction.method = value.trim();
                                if (key === 'path') currentAction.path = value.trim();
                                if (key === 'input') currentAction.input = value.trim();
                                if (key === 'output') currentAction.output = value.trim();
                            }
                        }
                        j++;
                    }
                    doc.modules.set(moduleName, { name: moduleName, line: i, actions });
                }
            }
        }

        this.loadImports(doc);
        this.documents.set(uri.toString(), doc);
        return doc;
    }

    private loadImports(doc: VeldDocument): void {
        // Find the veld project root by looking for veld.config.json or app.veld
        const projectRoot = this.findProjectRoot(doc.uri.fsPath);
        if (!projectRoot) return;

        const loadedFiles = new Set<string>();
        loadedFiles.add(doc.uri.fsPath);

        for (const [, relativePath] of doc.imports) {
            // @models/auth -> ./models/auth.veld -> resolve from PROJECT ROOT
            const fullPath = path.resolve(projectRoot, relativePath);

            if (loadedFiles.has(fullPath)) continue;
            loadedFiles.add(fullPath);

            try {
                if (!fs.existsSync(fullPath)) continue;
                const importedContent = fs.readFileSync(fullPath, 'utf-8');
                const lines = importedContent.split('\n');

                for (let i = 0; i < lines.length; i++) {
                    const line = lines[i].trim();

                    if (line.startsWith('model')) {
                        const match = line.match(/model\s+([A-Za-z_]\w*)/);
                        if (match) {
                            const modelName = match[1];
                            const fields = new Map<string, string>();
                            let j = i + 1;
                            while (j < lines.length && lines[j].trim() !== '}') {
                                const fieldLine = lines[j].trim();
                                const fieldMatch = fieldLine.match(/^([a-z_]\w*):\s*(.+?)(?:\s*\/\/.*)?$/);
                                if (fieldMatch) fields.set(fieldMatch[1], fieldMatch[2]);
                                j++;
                            }
                            if (!doc.models.has(modelName)) {
                                doc.models.set(modelName, { name: modelName, line: i, fields });
                            }
                        }
                    }

                    if (line.startsWith('enum')) {
                        const match = line.match(/enum\s+([A-Za-z_]\w*)/);
                        if (match) {
                            const enumName = match[1];
                            const values: string[] = [];
                            let j = i + 1;
                            while (j < lines.length && lines[j].trim() !== '}') {
                                const enumLine = lines[j].trim();
                                const enumMatch = enumLine.match(/^([a-z_]\w*)/);
                                if (enumMatch) values.push(enumMatch[1]);
                                j++;
                            }
                            if (!doc.enums.has(enumName)) {
                                doc.enums.set(enumName, { name: enumName, line: i, values });
                            }
                        }
                    }
                }
            } catch (e) {
                // import failed silently
            }
        }
    }

    private findProjectRoot(filePath: string): string | null {
        let dir = path.dirname(filePath);
        // Walk up until we find veld.config.json or app.veld
        for (let i = 0; i < 10; i++) {
            if (fs.existsSync(path.join(dir, 'veld.config.json'))) return dir;
            if (fs.existsSync(path.join(dir, 'app.veld'))) return dir;
            const parent = path.dirname(dir);
            if (parent === dir) break;
            dir = parent;
        }
        // Fallback: workspace folder
        return null;
    }

    validateDocument(uri: vscode.Uri, content: string): vscode.Diagnostic[] {
        const doc = this.parseDocument(uri, content);
        const diagnostics: vscode.Diagnostic[] = [];
        const lines = content.split('\n');

        for (let i = 0; i < lines.length; i++) {
            const line = lines[i];
            const trimmed = line.trim();

            // Check for imports that aren't used
            if (trimmed.startsWith('import')) {
                const importMatch = trimmed.match(/import\s+@([\w]+)\/([\w]+)/);
                if (importMatch) {
                    const alias = importMatch[1];
                    const name = importMatch[2];
                    const importPath = `@${alias}/${name}`;

                    // Check if imported name is used anywhere
                    let isUsed = false;
                    for (let j = 0; j < lines.length; j++) {
                        if (j !== i && lines[j].includes(name)) {
                            isUsed = true;
                            break;
                        }
                    }

                    if (!isUsed) {
                        diagnostics.push(new vscode.Diagnostic(
                            new vscode.Range(i, 0, i, trimmed.length),
                            `Import '${importPath}' is not used`,
                            vscode.DiagnosticSeverity.Warning
                        ));
                    }
                } else if (trimmed.match(/import\s+/)) {
                    // Invalid import syntax
                    diagnostics.push(new vscode.Diagnostic(
                        new vscode.Range(i, 0, i, trimmed.length),
                        `Invalid import syntax. Use: import @alias/name (e.g., import @models/user)`,
                        vscode.DiagnosticSeverity.Error
                    ));
                }
            }

            // Only check types in specific directives (input, output)
            // Skip method, path, description, prefix
            if (trimmed.startsWith('input:') || trimmed.startsWith('output:')) {
                const typeMatches = trimmed.matchAll(/:\s*(?:List<)?([A-Za-z_]\w*)(?:>)?/g);
                for (const match of typeMatches) {
                    const typeName = match[1];
                    if (!BUILTIN_TYPES.has(typeName) && !doc.models.has(typeName) && !doc.enums.has(typeName)) {
                        const suggestions = Array.from(doc.models.keys()).concat(Array.from(doc.enums.keys()));
                        const imports = Array.from(doc.imports.keys());

                        let msg = `Type '${typeName}' not found.`;
                        if (suggestions.length > 0) {
                            msg += ` Did you mean: ${suggestions.join(', ')}?`;
                        } else if (imports.length > 0) {
                            msg += ` Did you forget to import? Available: ${imports.join(', ')}`;
                        } else {
                            msg += ` Use: import @alias/${typeName.toLowerCase()}`;
                        }

                        diagnostics.push(new vscode.Diagnostic(
                            new vscode.Range(i, match.index!, i, match.index! + typeName.length),
                            msg,
                            vscode.DiagnosticSeverity.Error
                        ));
                    }
                }
            }

            // Check for field type definitions in models/enums
            if (!trimmed.startsWith('method:') &&
                !trimmed.startsWith('path:') &&
                !trimmed.startsWith('description:') &&
                !trimmed.startsWith('prefix:') &&
                !trimmed.startsWith('input:') &&
                !trimmed.startsWith('output:')) {

                const fieldMatch = trimmed.match(/^([a-z_]\w*):\s*(?:List<)?([A-Za-z_]\w*)(?:>)?/);
                if (fieldMatch) {
                    const typeName = fieldMatch[2];
                    if (!BUILTIN_TYPES.has(typeName) && !doc.models.has(typeName) && !doc.enums.has(typeName)) {
                        const suggestions = Array.from(doc.models.keys()).concat(Array.from(doc.enums.keys()));

                        let msg = `Type '${typeName}' not found.`;
                        if (suggestions.length > 0) {
                            msg += ` Did you mean: ${suggestions.join(', ')}?`;
                        }

                        diagnostics.push(new vscode.Diagnostic(
                            new vscode.Range(i, trimmed.indexOf(typeName), i, trimmed.indexOf(typeName) + typeName.length),
                            msg,
                            vscode.DiagnosticSeverity.Error
                        ));
                    }
                }
            }

            // Check HTTP methods
            if (trimmed.startsWith('method:')) {
                const methodMatch = trimmed.match(/method:\s*(\w+)/);
                if (methodMatch && !HTTP_METHODS.has(methodMatch[1])) {
                    const methodStart = line.indexOf(methodMatch[1]);
                    diagnostics.push(new vscode.Diagnostic(
                        new vscode.Range(i, methodStart, i, methodStart + methodMatch[1].length),
                        `Invalid HTTP method '${methodMatch[1]}'. Valid: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS`,
                        vscode.DiagnosticSeverity.Error
                    ));
                }
            }

            // Check for unknown directives
            const dirMatch = trimmed.match(/^(\w+):/);
            if (dirMatch && trimmed.length > 0 && !trimmed.startsWith('//')) {
                const directive = dirMatch[1];
                // Skip if it's a field definition (lowercase) or a known directive
                if (directive[0] !== directive[0].toLowerCase() ||
                    (!DIRECTIVES.has(directive) && !KEYWORDS.has(directive))) {
                    // Only warn if it looks like a directive (not a field)
                    if (directive[0] === directive[0].toLowerCase() && directive.length > 0) {
                        // It's a field, skip
                    } else if (!DIRECTIVES.has(directive) && !KEYWORDS.has(directive)) {
                        diagnostics.push(new vscode.Diagnostic(
                            new vscode.Range(i, 0, i, directive.length),
                            `Unknown directive '${directive}'. Valid: ${Array.from(DIRECTIVES).join(', ')}`,
                            vscode.DiagnosticSeverity.Warning
                        ));
                    }
                }
            }
        }

        this.diagnosticCollection.set(uri, diagnostics);
        return diagnostics;
    }

    getCompletions(uri: vscode.Uri, position: vscode.Position, content: string): vscode.CompletionItem[] {
        const doc = this.parseDocument(uri, content);
        const lines = content.split('\n');
        const lineText = lines[position.line];
        const beforeCursor = lineText.substring(0, position.character);

        const completions: vscode.CompletionItem[] = [];

        if (beforeCursor.trim().length === 0) {
            for (const kw of KEYWORDS) {
                const item = new vscode.CompletionItem(kw, vscode.CompletionItemKind.Keyword);
                item.insertText = kw + ' ';
                item.detail = 'Veld keyword';
                completions.push(item);
            }
        }

        if (beforeCursor.includes(':')) {
            for (const type of BUILTIN_TYPES) {
                completions.push(new vscode.CompletionItem(type, vscode.CompletionItemKind.TypeParameter));
            }
            for (const [modelName] of doc.models) {
                const item = new vscode.CompletionItem(modelName, vscode.CompletionItemKind.Class);
                item.documentation = new vscode.MarkdownString(`**Model** - line ${doc.models.get(modelName)!.line + 1}`);
                completions.push(item);
            }
            for (const [enumName] of doc.enums) {
                const item = new vscode.CompletionItem(enumName, vscode.CompletionItemKind.Enum);
                item.documentation = new vscode.MarkdownString(`**Enum** - line ${doc.enums.get(enumName)!.line + 1}`);
                completions.push(item);
            }
        }

        if (beforeCursor.includes('action') || beforeCursor.includes('module')) {
            for (const directive of DIRECTIVES) {
                completions.push(new vscode.CompletionItem(directive + ':', vscode.CompletionItemKind.Property));
            }
        }

        if (beforeCursor.includes('method:')) {
            for (const method of HTTP_METHODS) {
                completions.push(new vscode.CompletionItem(method, vscode.CompletionItemKind.Constant));
            }
        }

        return completions;
    }

    getHoverInfo(uri: vscode.Uri, position: vscode.Position, content: string): vscode.Hover | null {
        const doc = this.parseDocument(uri, content);
        const lines = content.split('\n');
        const line = lines[position.line];
        const word = this.getWordAt(line, position.character);

        if (!word) return null;

        if (doc.models.has(word)) {
            const model = doc.models.get(word)!;
            let fields = '```\n';
            for (const [fieldName, fieldType] of model.fields) {
                fields += `  ${fieldName}: ${fieldType}\n`;
            }
            fields += '```';
            return new vscode.Hover(new vscode.MarkdownString(`**Model** \`${word}\`\n\n${fields}`));
        }

        if (doc.enums.has(word)) {
            const enumDef = doc.enums.get(word)!;
            return new vscode.Hover(new vscode.MarkdownString(`**Enum** \`${word}\`\n\n**Values:** \`${enumDef.values.join('`, `')}\``));
        }

        if (doc.modules.has(word)) {
            const module = doc.modules.get(word)!;
            let actions = '';
            for (const [, action] of module.actions) {
                actions += `  - **${action.name}** (${action.method} ${action.path})\n`;
            }
            return new vscode.Hover(new vscode.MarkdownString(`**Module** \`${word}\`\n\n**Actions:**\n${actions}`));
        }

        if (BUILTIN_TYPES.has(word)) {
            return new vscode.Hover(new vscode.MarkdownString(`**Built-in Type** \`${word}\``));
        }

        return null;
    }

    getDefinition(uri: vscode.Uri, position: vscode.Position, content: string): vscode.Location | null {
        const doc = this.parseDocument(uri, content);
        const lines = content.split('\n');
        const line = lines[position.line];
        const trimmed = line.trim();

        // Import line: jump to the imported file
        if (trimmed.startsWith('import')) {
            const match = trimmed.match(/import\s+@([\w]+)\/([\w]+)/);
            if (match) {
                const relativePath = `./${match[1]}/${match[2]}.veld`;
                const projectRoot = this.findProjectRoot(uri.fsPath);
                if (projectRoot) {
                    const fullPath = path.resolve(projectRoot, relativePath);
                    if (fs.existsSync(fullPath)) {
                        return new vscode.Location(vscode.Uri.file(fullPath), new vscode.Position(0, 0));
                    }
                }
            }
            return null;
        }

        const word = this.getWordAt(line, position.character);
        if (!word) return null;

        // Jump to model/enum/module definition (works across imported files too)
        if (doc.models.has(word)) {
            const model = doc.models.get(word)!;
            // If the model came from an import, find which file it's in
            const projectRoot = this.findProjectRoot(uri.fsPath);
            if (projectRoot) {
                for (const [, relativePath] of doc.imports) {
                    const fullPath = path.resolve(projectRoot, relativePath);
                    if (fs.existsSync(fullPath)) {
                        const importedContent = fs.readFileSync(fullPath, 'utf-8');
                        const importedLines = importedContent.split('\n');
                        for (let i = 0; i < importedLines.length; i++) {
                            if (importedLines[i].trim().startsWith(`model ${word}`)) {
                                return new vscode.Location(vscode.Uri.file(fullPath), new vscode.Position(i, 0));
                            }
                        }
                    }
                }
            }
            // Fallback: definition in current file
            return new vscode.Location(uri, new vscode.Position(model.line, 0));
        }
        if (doc.enums.has(word)) {
            const enumDef = doc.enums.get(word)!;
            const projectRoot = this.findProjectRoot(uri.fsPath);
            if (projectRoot) {
                for (const [, relativePath] of doc.imports) {
                    const fullPath = path.resolve(projectRoot, relativePath);
                    if (fs.existsSync(fullPath)) {
                        const importedContent = fs.readFileSync(fullPath, 'utf-8');
                        const importedLines = importedContent.split('\n');
                        for (let i = 0; i < importedLines.length; i++) {
                            if (importedLines[i].trim().startsWith(`enum ${word}`)) {
                                return new vscode.Location(vscode.Uri.file(fullPath), new vscode.Position(i, 0));
                            }
                        }
                    }
                }
            }
            return new vscode.Location(uri, new vscode.Position(enumDef.line, 0));
        }
        if (doc.modules.has(word)) {
            return new vscode.Location(uri, new vscode.Position(doc.modules.get(word)!.line, 0));
        }

        return null;
    }

    getReferences(uri: vscode.Uri, position: vscode.Position, content: string): vscode.Location[] {
        const lines = content.split('\n');
        const line = lines[position.line];
        const word = this.getWordAt(line, position.character);

        if (!word) return [];

        const references: vscode.Location[] = [];
        for (let i = 0; i < lines.length; i++) {
            const regex = new RegExp(`\\b${word}\\b`, 'g');
            let match;
            while ((match = regex.exec(lines[i])) !== null) {
                references.push(new vscode.Location(uri, new vscode.Position(i, match.index)));
            }
        }
        return references;
    }

    private getWordAt(line: string, position: number): string | null {
        const match = line.substring(0, position).match(/([A-Za-z_]\w*)$/);
        return match ? match[1] : null;
    }
}

// ── VS Code Extension Activation ────────────────────

export function activate(context: vscode.ExtensionContext): void {
    const server = new VeldLanguageServer();

    vscode.workspace.onDidChangeTextDocument(event => {
        if (event.document.languageId === 'veld') {
            server.validateDocument(event.document.uri, event.document.getText());
        }
    });

    vscode.workspace.onDidOpenTextDocument(doc => {
        if (doc.languageId === 'veld') {
            server.validateDocument(doc.uri, doc.getText());
        }
    });

    context.subscriptions.push(
        vscode.languages.registerCompletionItemProvider('veld', {
            provideCompletionItems(doc, pos) {
                return server.getCompletions(doc.uri, pos, doc.getText());
            }
        }, ':', ' ', '{')
    );

    context.subscriptions.push(
        vscode.languages.registerHoverProvider('veld', {
            provideHover(doc, pos) {
                return server.getHoverInfo(doc.uri, pos, doc.getText());
            }
        })
    );

    context.subscriptions.push(
        vscode.languages.registerDefinitionProvider('veld', {
            provideDefinition(doc, pos) {
                return server.getDefinition(doc.uri, pos, doc.getText());
            }
        })
    );

    context.subscriptions.push(
        vscode.languages.registerReferenceProvider('veld', {
            provideReferences(doc, pos, context) {
                return server.getReferences(doc.uri, pos, doc.getText());
            }
        })
    );
}

export function deactivate(): void {}













