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
const vscode = __importStar(require("vscode"));
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
const veld_language_spec_1 = require("./veld-language-spec");
// ── Language Server ──────────────────────────────────
class VeldLanguageServer {
    constructor() {
        this.documents = new Map();
        this.diagnosticCollection = vscode.languages.createDiagnosticCollection('veld');
    }
    findProjectRoot(filePath) {
        let dir = path.dirname(filePath);
        for (let i = 0; i < 15; i++) {
            if (fs.existsSync(path.join(dir, 'veld.config.json')))
                return dir;
            if (fs.existsSync(path.join(dir, 'app.veld')))
                return dir;
            const parent = path.dirname(dir);
            if (parent === dir)
                break;
            dir = parent;
        }
        return null;
    }
    parseFile(filePath, content) {
        const doc = {
            uri: vscode.Uri.file(filePath).toString(),
            filePath,
            content,
            models: new Map(),
            enums: new Map(),
            modules: new Map(),
            imports: [],
        };
        const lines = content.split('\n');
        for (let i = 0; i < lines.length; i++) {
            const trimmed = lines[i].trim();
            if (trimmed.startsWith('import')) {
                const match = trimmed.match(/import\s+@(\w+)\/(\w+)/);
                if (match) {
                    const alias = match[1];
                    const name = match[2];
                    const raw = `@${alias}/${name}`;
                    const projectRoot = this.findProjectRoot(filePath);
                    let resolvedPath;
                    if (projectRoot) {
                        const candidate = path.resolve(projectRoot, alias, `${name}.veld`);
                        if (fs.existsSync(candidate)) {
                            resolvedPath = candidate;
                        }
                    }
                    doc.imports.push({ raw, alias, name, line: i, resolvedPath });
                }
            }
            if (trimmed.startsWith('model ')) {
                const match = trimmed.match(/model\s+([A-Za-z_]\w*)/);
                if (match) {
                    const modelName = match[1];
                    const fields = [];
                    let j = i + 1;
                    while (j < lines.length && lines[j].trim() !== '}') {
                        const fieldLine = lines[j].trim();
                        const fieldMatch = fieldLine.match(/^([a-z_]\w*)\??:\s*(.+?)(?:\s*\/\/.*)?$/);
                        if (fieldMatch) {
                            // Strip @annotation(...) so hover shows clean type e.g. "Role" not "Role @default(user)"
                            const cleanType = fieldMatch[2].trim().replace(/@\w+(?:\([^)]*\))?/g, '').trim();
                            fields.push({ name: fieldMatch[1], type: cleanType, line: j });
                        }
                        j++;
                    }
                    doc.models.set(modelName, { name: modelName, line: i, file: filePath, fields });
                }
            }
            if (trimmed.startsWith('enum ')) {
                const match = trimmed.match(/enum\s+([A-Za-z_]\w*)/);
                if (match) {
                    const enumName = match[1];
                    const values = [];
                    let j = i + 1;
                    while (j < lines.length && lines[j].trim() !== '}') {
                        const val = lines[j].trim();
                        if (val && !val.startsWith('//')) {
                            val.split(/\s+/).forEach(v => { if (v)
                                values.push(v); });
                        }
                        j++;
                    }
                    doc.enums.set(enumName, { name: enumName, line: i, file: filePath, values });
                }
            }
            if (trimmed.startsWith('module ')) {
                const match = trimmed.match(/module\s+([A-Za-z_]\w*)/);
                if (match) {
                    const moduleName = match[1];
                    const actions = [];
                    let j = i + 1;
                    let braceDepth = 1;
                    let currentAction = null;
                    let moduleDescription;
                    let modulePrefix;
                    while (j < lines.length && braceDepth > 0) {
                        const actionLine = lines[j].trim();
                        if (actionLine === '}') {
                            braceDepth--;
                            if (braceDepth === 1 && currentAction?.name) {
                                actions.push(currentAction);
                                currentAction = null;
                            }
                        }
                        else if (actionLine.includes('{')) {
                            braceDepth++;
                            const actionMatch = actionLine.match(/action\s+([A-Za-z_]\w*)/);
                            if (actionMatch) {
                                currentAction = { name: actionMatch[1], line: j };
                            }
                        }
                        else if (currentAction) {
                            const dirMatch = actionLine.match(/^\s*(\w+):\s*(.+)/);
                            if (dirMatch) {
                                const [, key, value] = dirMatch;
                                if (key === 'method')
                                    currentAction.method = value.trim();
                                if (key === 'path')
                                    currentAction.path = value.trim();
                                if (key === 'input')
                                    currentAction.input = value.trim();
                                if (key === 'output')
                                    currentAction.output = value.trim();
                            }
                        }
                        else if (braceDepth === 1) {
                            const dirMatch = actionLine.match(/^\s*(\w+):\s*(.+)/);
                            if (dirMatch) {
                                if (dirMatch[1] === 'description')
                                    moduleDescription = dirMatch[2].replace(/"/g, '').trim();
                                if (dirMatch[1] === 'prefix')
                                    modulePrefix = dirMatch[2].trim();
                            }
                        }
                        j++;
                    }
                    if (currentAction?.name) {
                        actions.push(currentAction);
                    }
                    doc.modules.set(moduleName, {
                        name: moduleName, line: i, file: filePath,
                        description: moduleDescription, prefix: modulePrefix, actions,
                    });
                }
            }
        }
        return doc;
    }
    parseDocument(uri, content) {
        const filePath = uri.fsPath;
        const doc = this.parseFile(filePath, content);
        for (const imp of doc.imports) {
            if (!imp.resolvedPath || !fs.existsSync(imp.resolvedPath))
                continue;
            try {
                const importedContent = fs.readFileSync(imp.resolvedPath, 'utf-8');
                const importedDoc = this.parseFile(imp.resolvedPath, importedContent);
                for (const [name, model] of importedDoc.models) {
                    if (!doc.models.has(name))
                        doc.models.set(name, model);
                }
                for (const [name, enumDef] of importedDoc.enums) {
                    if (!doc.enums.has(name))
                        doc.enums.set(name, enumDef);
                }
                for (const [name, moduleDef] of importedDoc.modules) {
                    if (!doc.modules.has(name))
                        doc.modules.set(name, moduleDef);
                }
            }
            catch {
                // import failed silently
            }
        }
        this.documents.set(uri.toString(), doc);
        return doc;
    }
    validateDocument(uri, content) {
        const doc = this.parseDocument(uri, content);
        const diagnostics = [];
        const lines = content.split('\n');
        for (let i = 0; i < lines.length; i++) {
            const line = lines[i];
            const trimmed = line.trim();
            if (trimmed.startsWith('import')) {
                const importMatch = trimmed.match(/import\s+@(\w+)\/(\w+)/);
                if (importMatch) {
                    const imp = doc.imports.find(im => im.line === i);
                    if (imp && !imp.resolvedPath) {
                        const atIdx = line.indexOf('@');
                        diagnostics.push(new vscode.Diagnostic(new vscode.Range(i, atIdx >= 0 ? atIdx : 0, i, line.length), `Cannot resolve import '@${importMatch[1]}/${importMatch[2]}'. File not found.`, vscode.DiagnosticSeverity.Error));
                    }
                }
                else if (trimmed.match(/import\s+/)) {
                    diagnostics.push(new vscode.Diagnostic(new vscode.Range(i, 0, i, line.length), `Invalid import syntax. Use: import @models/name`, vscode.DiagnosticSeverity.Error));
                }
            }
            if (trimmed.startsWith('method:')) {
                const methodMatch = trimmed.match(/method:\s*(\w+)/);
                if (methodMatch && !veld_language_spec_1.HTTP_METHODS.has(methodMatch[1])) {
                    const methodStart = line.indexOf(methodMatch[1]);
                    diagnostics.push(new vscode.Diagnostic(new vscode.Range(i, methodStart, i, methodStart + methodMatch[1].length), `Invalid HTTP method '${methodMatch[1]}'. Valid: ${Array.from(veld_language_spec_1.HTTP_METHODS).join(', ')}`, vscode.DiagnosticSeverity.Error));
                }
            }
            if (trimmed.startsWith('input:') || trimmed.startsWith('output:') ||
                trimmed.startsWith('query:') || trimmed.startsWith('middleware:')) {
                const colonIdx = trimmed.indexOf(':');
                const typeExpr = trimmed.substring(colonIdx + 1).trim();
                this.validateTypeExpr(typeExpr, line, i, doc, diagnostics);
            }
            if (!trimmed.startsWith('method:') &&
                !trimmed.startsWith('path:') &&
                !trimmed.startsWith('description:') &&
                !trimmed.startsWith('prefix:') &&
                !trimmed.startsWith('input:') &&
                !trimmed.startsWith('output:') &&
                !trimmed.startsWith('query:') &&
                !trimmed.startsWith('middleware:') &&
                !trimmed.startsWith('default:') &&
                !trimmed.startsWith('import') &&
                !trimmed.startsWith('//') &&
                !trimmed.startsWith('model ') &&
                !trimmed.startsWith('module ') &&
                !trimmed.startsWith('enum ') &&
                !trimmed.startsWith('action ') &&
                trimmed !== '{' && trimmed !== '}' && trimmed.length > 0) {
                // Field definition: fieldName: Type  or  fieldName?: Type @default(val)
                const fieldMatch = trimmed.match(/^([a-z_]\w*)\??:\s*(.+?)(?:\s*\/\/.*)?$/);
                if (fieldMatch) {
                    const typeExpr = fieldMatch[2].trim();
                    // validateTypeExpr strips @annotations internally
                    this.validateTypeExpr(typeExpr, line, i, doc, diagnostics);
                }
            }
        }
        let braceDepth = 0;
        for (let i = 0; i < lines.length; i++) {
            for (const ch of lines[i]) {
                if (ch === '{')
                    braceDepth++;
                if (ch === '}')
                    braceDepth--;
            }
        }
        if (braceDepth > 0) {
            const lastLine = lines.length - 1;
            diagnostics.push(new vscode.Diagnostic(new vscode.Range(lastLine, 0, lastLine, lines[lastLine].length), `Unclosed brace: ${braceDepth} opening brace(s) without matching close`, vscode.DiagnosticSeverity.Error));
        }
        else if (braceDepth < 0) {
            diagnostics.push(new vscode.Diagnostic(new vscode.Range(0, 0, 0, 1), `Extra closing brace(s): ${-braceDepth} without matching open`, vscode.DiagnosticSeverity.Error));
        }
        this.diagnosticCollection.set(uri, diagnostics);
        return diagnostics;
    }
    validateTypeExpr(typeExpr, line, lineNum, doc, diagnostics) {
        // Validate @annotation names
        for (const annMatch of typeExpr.matchAll(/@(\w+)/g)) {
            const annName = annMatch[1];
            if (!VeldLanguageServer.KNOWN_ANNOTATIONS.has(annName)) {
                const annStart = line.indexOf('@' + annName);
                if (annStart >= 0) {
                    diagnostics.push(new vscode.Diagnostic(new vscode.Range(lineNum, annStart, lineNum, annStart + annName.length + 1), `Unknown annotation '@${annName}'. Known annotations: ${[...VeldLanguageServer.KNOWN_ANNOTATIONS].map(n => '@' + n).join(', ')}`, vscode.DiagnosticSeverity.Warning));
                }
            }
        }
        // Strip @annotation(...) clauses (e.g. @default(user)) before type validation
        const clean = typeExpr.replace(/@\w+(?:\([^)]*\))?/g, '').trim();
        const typeNames = clean.matchAll(/[A-Za-z_]\w*/g);
        for (const match of typeNames) {
            const typeName = match[0];
            if (veld_language_spec_1.BUILTIN_TYPES.has(typeName))
                continue;
            if (veld_language_spec_1.SPECIAL_TYPES.has(typeName))
                continue;
            if (doc.models.has(typeName))
                continue;
            if (doc.enums.has(typeName))
                continue;
            const colonIdx = line.indexOf(':');
            const typeStart = line.indexOf(typeName, colonIdx >= 0 ? colonIdx : 0);
            if (typeStart < 0)
                continue;
            const suggestions = [...doc.models.keys(), ...doc.enums.keys()];
            let msg = `Type '${typeName}' not found.`;
            if (suggestions.length > 0) {
                msg += ` Available types: ${suggestions.join(', ')}`;
            }
            diagnostics.push(new vscode.Diagnostic(new vscode.Range(lineNum, typeStart, lineNum, typeStart + typeName.length), msg, vscode.DiagnosticSeverity.Error));
        }
    }
    getCompletions(uri, position, content) {
        const doc = this.parseDocument(uri, content);
        const lines = content.split('\n');
        const lineText = lines[position.line];
        const beforeCursor = lineText.substring(0, position.character);
        const trimmedBefore = beforeCursor.trimStart();
        const completions = [];
        if (trimmedBefore.startsWith('import ') || trimmedBefore === 'import') {
            const projectRoot = this.findProjectRoot(uri.fsPath);
            // Detect if the user already typed "@" — if so, don't prepend it again
            const alreadyHasAt = trimmedBefore.includes('@');
            if (projectRoot) {
                for (const dirName of ['models', 'modules']) {
                    const dirPath = path.join(projectRoot, dirName);
                    if (fs.existsSync(dirPath)) {
                        try {
                            const files = fs.readdirSync(dirPath);
                            for (const file of files) {
                                if (file.endsWith('.veld')) {
                                    const name = file.replace('.veld', '');
                                    const label = `@${dirName}/${name}`;
                                    const item = new vscode.CompletionItem(label, vscode.CompletionItemKind.Module);
                                    item.detail = `${dirName}/${file}`;
                                    item.documentation = new vscode.MarkdownString(`Import from \`${dirName}/${file}\``);
                                    item.filterText = label;
                                    // If user already typed "@", insert without "@" to avoid double-@
                                    if (alreadyHasAt) {
                                        item.insertText = `${dirName}/${name}`;
                                    }
                                    completions.push(item);
                                }
                            }
                        }
                        catch { /* ignore */ }
                    }
                }
            }
            return completions;
        }
        if (trimmedBefore.match(/method:\s*\w*$/)) {
            for (const method of veld_language_spec_1.HTTP_METHODS) {
                const item = new vscode.CompletionItem(method, vscode.CompletionItemKind.Constant);
                item.detail = 'HTTP method';
                completions.push(item);
            }
            return completions;
        }
        if (trimmedBefore.match(/(input|output|query|middleware):\s*\w*$/)) {
            return this.getTypeCompletions(doc);
        }
        // Annotation completion: field has a type and user typed "@"
        // Matches: "fieldName: Type @" or "fieldName?: Type @word"
        if (trimmedBefore.match(/^[a-z_]\w*\??\s*:\s*\w+.*@\w*$/)) {
            return this.getAnnotationCompletions();
        }
        if (trimmedBefore.match(/^[a-z_]\w*:\s*\w*$/)) {
            return this.getTypeCompletions(doc);
        }
        const ctx = this.detectContext(lines, position.line);
        if (ctx === 'top') {
            for (const kw of veld_language_spec_1.KEYWORDS) {
                const item = new vscode.CompletionItem(kw, vscode.CompletionItemKind.Keyword);
                item.detail = 'Veld keyword';
                completions.push(item);
            }
            const importSnippet = new vscode.CompletionItem('import', vscode.CompletionItemKind.Snippet);
            importSnippet.insertText = new vscode.SnippetString('import @${1|models,modules|}/${2:name}');
            importSnippet.detail = 'Import a model or module file';
            completions.push(importSnippet);
            const modelSnippet = new vscode.CompletionItem('model', vscode.CompletionItemKind.Snippet);
            modelSnippet.insertText = new vscode.SnippetString('model ${1:Name} {\n  ${2:field}: ${3:string}\n}');
            modelSnippet.detail = 'Define a new model';
            completions.push(modelSnippet);
            const moduleSnippet = new vscode.CompletionItem('module', vscode.CompletionItemKind.Snippet);
            moduleSnippet.insertText = new vscode.SnippetString('module ${1:Name} {\n  description: "${2:description}"\n  prefix: /${3:path}\n\n  action ${4:ActionName} {\n    method: ${5|GET,POST,PUT,DELETE,PATCH|}\n    path: /${6:path}\n  }\n}');
            moduleSnippet.detail = 'Define a new module';
            completions.push(moduleSnippet);
        }
        else if (ctx === 'module') {
            for (const d of ['description', 'prefix']) {
                const item = new vscode.CompletionItem(`${d}: `, vscode.CompletionItemKind.Property);
                item.detail = 'Module directive';
                completions.push(item);
            }
            const actionSnippet = new vscode.CompletionItem('action', vscode.CompletionItemKind.Snippet);
            actionSnippet.insertText = new vscode.SnippetString('action ${1:Name} {\n    method: ${2|GET,POST,PUT,DELETE,PATCH|}\n    path: /${3:path}\n    ${4:input: ${5:InputType}\n    }output: ${6:OutputType}\n  }');
            actionSnippet.detail = 'Define a new action';
            completions.push(actionSnippet);
        }
        else if (ctx === 'action') {
            for (const d of ['method', 'path', 'input', 'output', 'query', 'middleware', 'description']) {
                const item = new vscode.CompletionItem(`${d}: `, vscode.CompletionItemKind.Property);
                item.detail = 'Action directive';
                completions.push(item);
            }
        }
        else if (ctx === 'model') {
            return this.getTypeCompletions(doc);
        }
        return completions;
    }
    getTypeCompletions(doc) {
        const completions = [];
        for (const t of veld_language_spec_1.BUILTIN_TYPES) {
            const item = new vscode.CompletionItem(t, vscode.CompletionItemKind.TypeParameter);
            item.detail = 'Built-in type';
            completions.push(item);
        }
        for (const t of veld_language_spec_1.SPECIAL_TYPES) {
            const item = new vscode.CompletionItem(t, vscode.CompletionItemKind.TypeParameter);
            item.detail = 'Generic type';
            item.insertText = new vscode.SnippetString(`${t}<\${1:Type}>`);
            completions.push(item);
        }
        for (const [, model] of doc.models) {
            const item = new vscode.CompletionItem(model.name, vscode.CompletionItemKind.Class);
            const fieldsSummary = model.fields.map(f => `  ${f.name}: ${f.type}`).join('\n');
            const sourceFile = path.basename(model.file);
            item.detail = `model (from ${sourceFile})`;
            item.documentation = new vscode.MarkdownString(`**Model** \`${model.name}\`\n\n` +
                `**Source:** \`${sourceFile}\`\n\n` +
                '```veld\nmodel ' + model.name + ' {\n' + fieldsSummary + '\n}\n```');
            completions.push(item);
        }
        for (const [, enumDef] of doc.enums) {
            const item = new vscode.CompletionItem(enumDef.name, vscode.CompletionItemKind.Enum);
            const sourceFile = path.basename(enumDef.file);
            item.detail = `enum (from ${sourceFile})`;
            item.documentation = new vscode.MarkdownString(`**Enum** \`${enumDef.name}\`\n\n` +
                `**Source:** \`${sourceFile}\`\n\n` +
                `**Values:** ${enumDef.values.map(v => `\`${v}\``).join(', ')}`);
            completions.push(item);
        }
        return completions;
    }
    getAnnotationCompletions() {
        // Label includes "@" for display; insertText omits it because "@" is the trigger
        // character already typed by the user. filterText includes "@" for correct filtering.
        const annotations = [
            { label: '@default', insert: 'default(${1:value})', detail: 'Set a default value for this field' },
            { label: '@required', insert: 'required', detail: 'Mark this field as required' },
            { label: '@min', insert: 'min(${1:value})', detail: 'Minimum value constraint (int/float)' },
            { label: '@max', insert: 'max(${1:value})', detail: 'Maximum value constraint (int/float)' },
            { label: '@minLength', insert: 'minLength(${1:length})', detail: 'Minimum string length' },
            { label: '@maxLength', insert: 'maxLength(${1:length})', detail: 'Maximum string length' },
            { label: '@regex', insert: 'regex(${1:pattern})', detail: 'Regex pattern constraint for strings' },
            { label: '@unique', insert: 'unique', detail: 'Mark this field as unique' },
            { label: '@deprecated', insert: 'deprecated', detail: 'Mark this field as deprecated' },
        ];
        return annotations.map(ann => {
            const item = new vscode.CompletionItem(ann.label, vscode.CompletionItemKind.Event);
            item.detail = ann.detail;
            item.insertText = new vscode.SnippetString(ann.insert);
            item.filterText = ann.label; // allow filtering by "@default" even though label is same
            return item;
        });
    }
    detectContext(lines, cursorLine) {
        let depth = 0;
        let inModule = false;
        let inAction = false;
        let inModel = false;
        let inEnum = false;
        for (let i = 0; i <= cursorLine; i++) {
            const trimmed = lines[i].trim();
            if (trimmed.startsWith('module ') && trimmed.includes('{')) {
                inModule = true;
            }
            if (trimmed.startsWith('model ') && trimmed.includes('{')) {
                inModel = true;
            }
            if (trimmed.startsWith('enum ') && trimmed.includes('{')) {
                inEnum = true;
            }
            if (trimmed.startsWith('action ') && trimmed.includes('{')) {
                inAction = true;
            }
            for (const ch of trimmed) {
                if (ch === '{')
                    depth++;
                if (ch === '}') {
                    depth--;
                    if (depth <= 0) {
                        inModule = false;
                        inModel = false;
                        inEnum = false;
                        inAction = false;
                        depth = 0;
                    }
                    if (depth <= 1)
                        inAction = false;
                }
            }
        }
        if (inAction)
            return 'action';
        if (inModule)
            return 'module';
        if (inModel)
            return 'model';
        if (inEnum)
            return 'enum';
        return 'top';
    }
    getHoverInfo(uri, position, content) {
        const doc = this.parseDocument(uri, content);
        const lines = content.split('\n');
        const line = lines[position.line];
        const trimmed = line.trim();
        // Import path hover
        if (trimmed.startsWith('import')) {
            const importMatch = trimmed.match(/import\s+@(\w+)\/(\w+)/);
            if (importMatch) {
                const imp = doc.imports.find(im => im.line === position.line);
                if (imp) {
                    const md = new vscode.MarkdownString();
                    md.appendMarkdown(`**Import** \`${imp.raw}\`\n\n`);
                    if (imp.resolvedPath) {
                        md.appendMarkdown(`**Resolves to:** \`${path.basename(imp.resolvedPath)}\`\n\n`);
                        const importedModels = [...doc.models.values()].filter(m => m.file === imp.resolvedPath);
                        const importedEnums = [...doc.enums.values()].filter(e => e.file === imp.resolvedPath);
                        if (importedModels.length > 0) {
                            md.appendMarkdown(`**Models:** ${importedModels.map(m => `\`${m.name}\``).join(', ')}\n\n`);
                        }
                        if (importedEnums.length > 0) {
                            md.appendMarkdown(`**Enums:** ${importedEnums.map(e => `\`${e.name}\``).join(', ')}\n\n`);
                        }
                    }
                    else {
                        md.appendMarkdown(`*File not found*\n`);
                    }
                    return new vscode.Hover(md);
                }
            }
        }
        const word = this.getWordAt(line, position.character);
        if (!word)
            return null;
        if (doc.models.has(word)) {
            const model = doc.models.get(word);
            const sourceFile = path.basename(model.file);
            const fieldsSummary = model.fields.map(f => `  ${f.name}: ${f.type}`).join('\n');
            const md = new vscode.MarkdownString();
            md.appendMarkdown(`**Model** \`${model.name}\`\n\n`);
            md.appendMarkdown(`**Defined in:** \`${sourceFile}\` (line ${model.line + 1})\n\n`);
            md.appendMarkdown(`**Fields:** ${model.fields.length}\n\n`);
            md.appendCodeblock(`model ${model.name} {\n${fieldsSummary}\n}`, 'veld');
            return new vscode.Hover(md);
        }
        if (doc.enums.has(word)) {
            const enumDef = doc.enums.get(word);
            const sourceFile = path.basename(enumDef.file);
            const md = new vscode.MarkdownString();
            md.appendMarkdown(`**Enum** \`${enumDef.name}\`\n\n`);
            md.appendMarkdown(`**Defined in:** \`${sourceFile}\` (line ${enumDef.line + 1})\n\n`);
            md.appendMarkdown(`**Values:** ${enumDef.values.map(v => `\`${v}\``).join(', ')}\n\n`);
            md.appendCodeblock(`enum ${enumDef.name} {\n  ${enumDef.values.join('\n  ')}\n}`, 'veld');
            return new vscode.Hover(md);
        }
        if (doc.modules.has(word)) {
            const mod = doc.modules.get(word);
            const sourceFile = path.basename(mod.file);
            const md = new vscode.MarkdownString();
            md.appendMarkdown(`**Module** \`${mod.name}\`\n\n`);
            if (mod.description)
                md.appendMarkdown(`*${mod.description}*\n\n`);
            md.appendMarkdown(`**Defined in:** \`${sourceFile}\` (line ${mod.line + 1})\n\n`);
            if (mod.prefix)
                md.appendMarkdown(`**Prefix:** \`${mod.prefix}\`\n\n`);
            md.appendMarkdown(`**Actions:** ${mod.actions.length}\n\n`);
            for (const action of mod.actions) {
                md.appendMarkdown(`- **${action.name}** \`${action.method || '?'} ${action.path || '?'}\``);
                if (action.input)
                    md.appendMarkdown(` | input: \`${action.input}\``);
                if (action.output)
                    md.appendMarkdown(` | output: \`${action.output}\``);
                md.appendMarkdown('\n');
            }
            return new vscode.Hover(md);
        }
        if (veld_language_spec_1.BUILTIN_TYPES.has(word)) {
            return new vscode.Hover(new vscode.MarkdownString(`**Built-in Type** \`${word}\``));
        }
        if (veld_language_spec_1.HTTP_METHODS.has(word)) {
            return new vscode.Hover(new vscode.MarkdownString(`**HTTP Method** \`${word}\``));
        }
        if (veld_language_spec_1.SPECIAL_TYPES.has(word)) {
            return new vscode.Hover(new vscode.MarkdownString(`**Generic Type** \`${word}<T>\``));
        }
        if (veld_language_spec_1.KEYWORDS.has(word)) {
            const descriptions = {
                model: 'Defines a data model with typed fields',
                module: 'Groups related API actions under a common prefix',
                action: 'Defines an API endpoint with method, path, input, and output',
                enum: 'Defines an enumeration of named values',
                import: 'Imports models or modules from other .veld files',
                extends: 'Inherits fields from a parent model',
            };
            return new vscode.Hover(new vscode.MarkdownString(`**Keyword** \`${word}\`\n\n${descriptions[word] || ''}`));
        }
        if (veld_language_spec_1.DIRECTIVES.has(word)) {
            const descriptions = {
                description: 'A human-readable description of the model, module, or action',
                prefix: 'The URL prefix for all actions in a module',
                method: 'The HTTP method (GET, POST, PUT, DELETE, PATCH)',
                path: 'The URL path for this action (relative to module prefix)',
                input: 'The input/request body type for this action',
                output: 'The output/response body type for this action',
                default: 'The default value for a field or enum',
            };
            return new vscode.Hover(new vscode.MarkdownString(`**Directive** \`${word}\`\n\n${descriptions[word] || ''}`));
        }
        return null;
    }
    getDefinition(uri, position, content) {
        const doc = this.parseDocument(uri, content);
        const lines = content.split('\n');
        const line = lines[position.line];
        const trimmed = line.trim();
        // Import line: click anywhere on the line navigates to the imported file
        if (trimmed.startsWith('import')) {
            const imp = doc.imports.find(im => im.line === position.line);
            if (imp?.resolvedPath && fs.existsSync(imp.resolvedPath)) {
                return new vscode.Location(vscode.Uri.file(imp.resolvedPath), new vscode.Position(0, 0));
            }
            return null;
        }
        // Try to get import path at cursor position (for @models/auth style)
        const importPathAtCursor = this.getImportPathAt(line, position.character);
        if (importPathAtCursor) {
            const projectRoot = this.findProjectRoot(uri.fsPath);
            if (projectRoot) {
                const match = importPathAtCursor.match(/@(\w+)\/(\w+)/);
                if (match) {
                    const candidate = path.resolve(projectRoot, match[1], `${match[2]}.veld`);
                    if (fs.existsSync(candidate)) {
                        return new vscode.Location(vscode.Uri.file(candidate), new vscode.Position(0, 0));
                    }
                }
            }
        }
        const word = this.getWordAt(line, position.character);
        if (!word)
            return null;
        // Model definition
        if (doc.models.has(word)) {
            const model = doc.models.get(word);
            return new vscode.Location(vscode.Uri.file(model.file), new vscode.Position(model.line, 0));
        }
        // Enum definition
        if (doc.enums.has(word)) {
            const enumDef = doc.enums.get(word);
            return new vscode.Location(vscode.Uri.file(enumDef.file), new vscode.Position(enumDef.line, 0));
        }
        // Module definition
        if (doc.modules.has(word)) {
            const mod = doc.modules.get(word);
            return new vscode.Location(vscode.Uri.file(mod.file), new vscode.Position(mod.line, 0));
        }
        return null;
    }
    getImportPathAt(line, position) {
        // Check if cursor is within an @alias/name token
        const importMatch = line.match(/@\w+\/\w+/);
        if (!importMatch || importMatch.index === undefined)
            return null;
        const start = importMatch.index;
        const end = start + importMatch[0].length;
        if (position >= start && position <= end) {
            return importMatch[0];
        }
        return null;
    }
    getReferences(uri, position, content) {
        const lines = content.split('\n');
        const line = lines[position.line];
        const word = this.getWordAt(line, position.character);
        if (!word)
            return [];
        const references = [];
        for (let i = 0; i < lines.length; i++) {
            const regex = new RegExp(`\\b${word}\\b`, 'g');
            let match;
            while ((match = regex.exec(lines[i])) !== null) {
                references.push(new vscode.Location(uri, new vscode.Position(i, match.index)));
            }
        }
        const projectRoot = this.findProjectRoot(uri.fsPath);
        if (projectRoot) {
            this.searchVeldFilesForRefs(projectRoot, word, uri.fsPath, references);
        }
        return references;
    }
    searchVeldFilesForRefs(dir, word, excludePath, references) {
        try {
            const entries = fs.readdirSync(dir, { withFileTypes: true });
            for (const entry of entries) {
                const fullPath = path.join(dir, entry.name);
                if (entry.isDirectory()) {
                    this.searchVeldFilesForRefs(fullPath, word, excludePath, references);
                }
                else if (entry.name.endsWith('.veld') && fullPath !== excludePath) {
                    try {
                        const content = fs.readFileSync(fullPath, 'utf-8');
                        const lines = content.split('\n');
                        for (let i = 0; i < lines.length; i++) {
                            const regex = new RegExp(`\\b${word}\\b`, 'g');
                            let match;
                            while ((match = regex.exec(lines[i])) !== null) {
                                references.push(new vscode.Location(vscode.Uri.file(fullPath), new vscode.Position(i, match.index)));
                            }
                        }
                    }
                    catch { /* ignore */ }
                }
            }
        }
        catch { /* ignore */ }
    }
    getWordAt(line, position) {
        const before = line.substring(0, position).match(/([A-Za-z_]\w*)$/);
        if (!before)
            return null;
        const wordStart = position - before[1].length;
        const after = line.substring(position).match(/^(\w*)/);
        const wordEnd = position + (after ? after[1].length : 0);
        const word = line.substring(wordStart, wordEnd);
        return word.length > 0 ? word : null;
    }
}
VeldLanguageServer.KNOWN_ANNOTATIONS = new Set([
    'default', 'required', 'min', 'max', 'minLength', 'maxLength',
    'regex', 'unique', 'deprecated', 'nullable', 'index', 'primaryKey',
]);
// ── Semantic Tokens ──────────────────────────────────
const SEMANTIC_TOKEN_TYPES = [
    'type', // 0: model/enum type references
    'class', // 1: model declarations
    'enum', // 2: enum declarations
    'namespace', // 3: module declarations
    'function', // 4: action declarations
    'property', // 5: field names
    'keyword', // 6: keywords (model, module, action, enum, import, extends)
    'parameter', // 7: directives (method, path, input, output, etc.)
    'string', // 8: strings, paths
    'number', // 9: numbers
    'comment', // 10: comments
    'variable', // 11: import path parts
    'decorator', // 12: annotations
    'enumMember', // 13: enum values, HTTP methods
];
const SEMANTIC_TOKEN_MODIFIERS = [
    'declaration',
    'definition',
    'readonly',
    'defaultLibrary',
];
const semanticTokensLegend = new vscode.SemanticTokensLegend(SEMANTIC_TOKEN_TYPES, SEMANTIC_TOKEN_MODIFIERS);
class VeldSemanticTokensProvider {
    constructor(server) {
        this.server = server;
    }
    provideDocumentSemanticTokens(document) {
        const builder = new vscode.SemanticTokensBuilder(semanticTokensLegend);
        const content = document.getText();
        const lines = content.split('\n');
        const doc = this.server.parseDocument(document.uri, content);
        for (let i = 0; i < lines.length; i++) {
            const line = lines[i];
            const trimmed = line.trim();
            // Comments
            if (trimmed.startsWith('//')) {
                const start = line.indexOf('//');
                builder.push(i, start, line.length - start, 10, 0);
                continue;
            }
            // Import statements
            if (trimmed.startsWith('import')) {
                const importIdx = line.indexOf('import');
                builder.push(i, importIdx, 6, 6, 0); // keyword
                const pathMatch = line.match(/@(\w+)\/(\w+)/);
                if (pathMatch && pathMatch.index !== undefined) {
                    const fullStart = line.indexOf('@', importIdx);
                    builder.push(i, fullStart, 1, 12, 0); // @ decorator
                    builder.push(i, fullStart + 1, pathMatch[1].length, 3, 0); // namespace
                    builder.push(i, fullStart + 1 + pathMatch[1].length, 1, 8, 0); // / separator
                    builder.push(i, fullStart + 1 + pathMatch[1].length + 1, pathMatch[2].length, 11, 0); // name
                }
                continue;
            }
            // Model declaration
            const modelMatch = trimmed.match(/^model\s+([A-Za-z_]\w*)(?:\s+(extends)\s+([A-Za-z_]\w*))?/);
            if (modelMatch) {
                const kwIdx = line.indexOf('model');
                builder.push(i, kwIdx, 5, 6, 1); // keyword + declaration
                const nameIdx = line.indexOf(modelMatch[1], kwIdx + 5);
                builder.push(i, nameIdx, modelMatch[1].length, 1, 3); // class + declaration + definition
                if (modelMatch[2] && modelMatch[3]) {
                    const extIdx = line.indexOf('extends', nameIdx);
                    builder.push(i, extIdx, 7, 6, 0); // keyword
                    const parentIdx = line.indexOf(modelMatch[3], extIdx + 7);
                    builder.push(i, parentIdx, modelMatch[3].length, 0, 0); // type
                }
                continue;
            }
            // Enum declaration
            const enumMatch = trimmed.match(/^enum\s+([A-Za-z_]\w*)/);
            if (enumMatch) {
                const kwIdx = line.indexOf('enum');
                builder.push(i, kwIdx, 4, 6, 1);
                const nameIdx = line.indexOf(enumMatch[1], kwIdx + 4);
                builder.push(i, nameIdx, enumMatch[1].length, 2, 3);
                continue;
            }
            // Module declaration
            const moduleMatch = trimmed.match(/^module\s+([A-Za-z_]\w*)/);
            if (moduleMatch) {
                const kwIdx = line.indexOf('module');
                builder.push(i, kwIdx, 6, 6, 1);
                const nameIdx = line.indexOf(moduleMatch[1], kwIdx + 6);
                builder.push(i, nameIdx, moduleMatch[1].length, 3, 3);
                continue;
            }
            // Action declaration
            const actionMatch = trimmed.match(/^action\s+([A-Za-z_]\w*)/);
            if (actionMatch) {
                const kwIdx = line.indexOf('action');
                builder.push(i, kwIdx, 6, 6, 1);
                const nameIdx = line.indexOf(actionMatch[1], kwIdx + 6);
                builder.push(i, nameIdx, actionMatch[1].length, 4, 3);
                continue;
            }
            // method: GET
            const methodDirective = trimmed.match(/^method:\s*(GET|POST|PUT|DELETE|PATCH|HEAD|OPTIONS)\b/);
            if (methodDirective) {
                const dirIdx = line.indexOf('method');
                builder.push(i, dirIdx, 6, 7, 0); // directive
                const valIdx = line.indexOf(methodDirective[1], dirIdx + 6);
                builder.push(i, valIdx, methodDirective[1].length, 13, 2); // enumMember + readonly
                continue;
            }
            // path: /... or prefix: /...
            const pathDirective = trimmed.match(/^(path|prefix):\s*(\/\S*)/);
            if (pathDirective) {
                const dirIdx = line.indexOf(pathDirective[1]);
                builder.push(i, dirIdx, pathDirective[1].length, 7, 0);
                const valIdx = line.indexOf(pathDirective[2], dirIdx + pathDirective[1].length);
                builder.push(i, valIdx, pathDirective[2].length, 8, 0);
                continue;
            }
            // description: "..."
            const descDirective = trimmed.match(/^description:\s*(".*")/);
            if (descDirective) {
                const dirIdx = line.indexOf('description');
                builder.push(i, dirIdx, 11, 7, 0);
                const valIdx = line.indexOf(descDirective[1], dirIdx + 11);
                builder.push(i, valIdx, descDirective[1].length, 8, 0);
                continue;
            }
            // input: Type or output: Type
            const ioDirective = trimmed.match(/^(input|output):\s*(.+?)\s*$/);
            if (ioDirective) {
                const dirIdx = line.indexOf(ioDirective[1]);
                builder.push(i, dirIdx, ioDirective[1].length, 7, 0);
                this.pushTypeTokens(builder, i, line, ioDirective[2], dirIdx + ioDirective[1].length + 1, doc);
                continue;
            }
            // default: value
            const defaultDirective = trimmed.match(/^default:\s*(.+?)\s*$/);
            if (defaultDirective) {
                const dirIdx = line.indexOf('default');
                builder.push(i, dirIdx, 7, 7, 0);
                continue;
            }
            // Field definitions: fieldName: Type
            const fieldMatch = trimmed.match(/^([a-z_]\w*)(\??):\s*(.+?)(?:\s*\/\/.*)?$/);
            if (fieldMatch) {
                const fieldIdx = line.indexOf(fieldMatch[1]);
                builder.push(i, fieldIdx, fieldMatch[1].length, 5, 0); // property
                this.pushTypeTokens(builder, i, line, fieldMatch[3], fieldIdx + fieldMatch[1].length + (fieldMatch[2].length) + 1, doc);
                continue;
            }
            // Enum values (inside an enum block - plain identifiers)
            if (trimmed !== '{' && trimmed !== '}' && trimmed.length > 0) {
                const words = trimmed.split(/\s+/);
                let searchFrom = line.indexOf(trimmed);
                for (const w of words) {
                    if (!w)
                        continue;
                    const wIdx = line.indexOf(w, searchFrom);
                    if (wIdx >= 0) {
                        builder.push(i, wIdx, w.length, 13, 0); // enumMember
                        searchFrom = wIdx + w.length;
                    }
                }
            }
        }
        return builder.build();
    }
    pushTypeTokens(builder, lineNum, line, typeExpr, searchFrom, doc) {
        const typeTokens = typeExpr.matchAll(/[A-Za-z_]\w*/g);
        for (const match of typeTokens) {
            const typeName = match[0];
            const colonIdx = line.indexOf(':', searchFrom - typeExpr.length);
            const typeIdx = line.indexOf(typeName, colonIdx >= 0 ? colonIdx : searchFrom);
            if (typeIdx < 0)
                continue;
            if (veld_language_spec_1.BUILTIN_TYPES.has(typeName)) {
                builder.push(lineNum, typeIdx, typeName.length, 0, 3); // type + defaultLibrary
            }
            else if (veld_language_spec_1.SPECIAL_TYPES.has(typeName)) {
                builder.push(lineNum, typeIdx, typeName.length, 1, 3); // class + defaultLibrary
            }
            else if (doc.models.has(typeName)) {
                builder.push(lineNum, typeIdx, typeName.length, 1, 0); // class (user model)
            }
            else if (doc.enums.has(typeName)) {
                builder.push(lineNum, typeIdx, typeName.length, 2, 0); // enum
            }
            else {
                builder.push(lineNum, typeIdx, typeName.length, 0, 0); // type (unknown)
            }
        }
    }
}
// ── VS Code Extension Activation ────────────────────
function activate(context) {
    const server = new VeldLanguageServer();
    const validateDoc = (doc) => {
        if (doc.languageId === 'veld') {
            server.validateDocument(doc.uri, doc.getText());
        }
    };
    vscode.workspace.onDidChangeTextDocument(event => validateDoc(event.document), null, context.subscriptions);
    vscode.workspace.onDidOpenTextDocument(validateDoc, null, context.subscriptions);
    vscode.workspace.onDidSaveTextDocument(validateDoc, null, context.subscriptions);
    vscode.workspace.textDocuments.forEach(validateDoc);
    context.subscriptions.push(vscode.languages.registerCompletionItemProvider('veld', {
        provideCompletionItems(doc, pos) {
            return server.getCompletions(doc.uri, pos, doc.getText());
        }
    }, ':', ' ', '@', '/'));
    context.subscriptions.push(vscode.languages.registerHoverProvider('veld', {
        provideHover(doc, pos) {
            return server.getHoverInfo(doc.uri, pos, doc.getText());
        }
    }));
    context.subscriptions.push(vscode.languages.registerDefinitionProvider('veld', {
        provideDefinition(doc, pos) {
            return server.getDefinition(doc.uri, pos, doc.getText());
        }
    }));
    context.subscriptions.push(vscode.languages.registerReferenceProvider('veld', {
        provideReferences(doc, pos) {
            return server.getReferences(doc.uri, pos, doc.getText());
        }
    }));
    // Semantic Tokens Provider for rich coloring
    context.subscriptions.push(vscode.languages.registerDocumentSemanticTokensProvider('veld', new VeldSemanticTokensProvider(server), semanticTokensLegend));
    context.subscriptions.push(vscode.languages.registerDocumentSymbolProvider('veld', {
        provideDocumentSymbols(doc) {
            const content = doc.getText();
            const symbols = [];
            const lines = content.split('\n');
            for (let i = 0; i < lines.length; i++) {
                const trimmed = lines[i].trim();
                const modelMatch = trimmed.match(/^model\s+([A-Za-z_]\w*)/);
                if (modelMatch) {
                    let endLine = i;
                    let depth = 0;
                    for (let j = i; j < lines.length; j++) {
                        for (const ch of lines[j]) {
                            if (ch === '{')
                                depth++;
                            if (ch === '}')
                                depth--;
                        }
                        if (depth <= 0) {
                            endLine = j;
                            break;
                        }
                    }
                    const range = new vscode.Range(i, 0, endLine, lines[endLine].length);
                    symbols.push(new vscode.DocumentSymbol(modelMatch[1], 'model', vscode.SymbolKind.Class, range, range));
                }
                const moduleMatch = trimmed.match(/^module\s+([A-Za-z_]\w*)/);
                if (moduleMatch) {
                    let endLine = i;
                    let depth = 0;
                    for (let j = i; j < lines.length; j++) {
                        for (const ch of lines[j]) {
                            if (ch === '{')
                                depth++;
                            if (ch === '}')
                                depth--;
                        }
                        if (depth <= 0) {
                            endLine = j;
                            break;
                        }
                    }
                    const range = new vscode.Range(i, 0, endLine, lines[endLine].length);
                    symbols.push(new vscode.DocumentSymbol(moduleMatch[1], 'module', vscode.SymbolKind.Module, range, range));
                }
                const enumMatch = trimmed.match(/^enum\s+([A-Za-z_]\w*)/);
                if (enumMatch) {
                    let endLine = i;
                    let depth = 0;
                    for (let j = i; j < lines.length; j++) {
                        for (const ch of lines[j]) {
                            if (ch === '{')
                                depth++;
                            if (ch === '}')
                                depth--;
                        }
                        if (depth <= 0) {
                            endLine = j;
                            break;
                        }
                    }
                    const range = new vscode.Range(i, 0, endLine, lines[endLine].length);
                    symbols.push(new vscode.DocumentSymbol(enumMatch[1], 'enum', vscode.SymbolKind.Enum, range, range));
                }
                const actionMatch = trimmed.match(/^action\s+([A-Za-z_]\w*)/);
                if (actionMatch) {
                    let endLine = i;
                    let depth = 0;
                    for (let j = i; j < lines.length; j++) {
                        for (const ch of lines[j]) {
                            if (ch === '{')
                                depth++;
                            if (ch === '}')
                                depth--;
                        }
                        if (depth <= 0) {
                            endLine = j;
                            break;
                        }
                    }
                    const range = new vscode.Range(i, 0, endLine, lines[endLine].length);
                    symbols.push(new vscode.DocumentSymbol(actionMatch[1], 'action', vscode.SymbolKind.Function, range, range));
                }
            }
            return symbols;
        }
    }));
}
function deactivate() { }
//# sourceMappingURL=extension.js.map