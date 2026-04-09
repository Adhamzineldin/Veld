package dev.veld.jetbrains

import com.intellij.codeInsight.completion.*
import com.intellij.codeInsight.lookup.LookupElementBuilder
import com.intellij.icons.AllIcons
import com.intellij.patterns.PlatformPatterns
import com.intellij.psi.PsiFile
import com.intellij.util.ProcessingContext

/**
 * Context-aware code completion for Veld language.
 * Provides keywords, directives, types, HTTP methods, and models/enums from imports.
 */
class VeldCompletionContributor : CompletionContributor() {

    init {
        extend(
            CompletionType.BASIC,
            PlatformPatterns.psiElement(),
            object : CompletionProvider<CompletionParameters>() {
                override fun addCompletions(
                    parameters: CompletionParameters,
                    context: ProcessingContext,
                    result: CompletionResultSet
                ) {
                    val file = parameters.originalFile
                    val project = file.project
                    val virtualFile = file.virtualFile ?: return
                    val service = VeldProjectService.getInstance(project)

                    val document = parameters.editor.document
                    val offset = parameters.offset
                    val lineNumber = document.getLineNumber(offset)
                    val lineStart = document.getLineStartOffset(lineNumber)
                    val lineText = document.getText(com.intellij.openapi.util.TextRange(lineStart, offset))
                    val fullLineText = document.getText(com.intellij.openapi.util.TextRange(
                        lineStart,
                        document.getLineEndOffset(lineNumber)
                    ))
                    val trimmedBefore = lineText.trimStart()

                    // Determine context
                    val ctx = detectContext(trimmedBefore, fullLineText.trim(), file)

                    when (ctx) {
                        CompletionContext.TOP_LEVEL -> {
                            addKeywords(result)
                            addImportSnippet(result, service, virtualFile)
                        }
                        CompletionContext.AFTER_IMPORT -> {
                            addImportPaths(result, service, virtualFile)
                        }
                        CompletionContext.INSIDE_MODULE -> {
                            addModuleDirectives(result)
                            result.addElement(LookupElementBuilder.create("action")
                                .bold()
                                .withIcon(AllIcons.Nodes.Method)
                                .withInsertHandler { ctx2, _ ->
                                    ctx2.document.insertString(ctx2.tailOffset, " Name {\n    method: \n    path: /\n  }")
                                    ctx2.editor.caretModel.moveToOffset(ctx2.tailOffset - 30)
                                })
                        }
                        CompletionContext.INSIDE_ACTION -> {
                            addActionDirectives(result)
                        }
                        CompletionContext.AFTER_METHOD_COLON -> {
                            addHttpMethods(result)
                        }
                        CompletionContext.AFTER_TYPE_COLON -> {
                            addTypes(result, service, virtualFile)
                        }
                        CompletionContext.AFTER_ANNOTATION_AT -> {
                            addAnnotationCompletions(result)
                        }
                        CompletionContext.INSIDE_MODEL -> {
                            addBuiltinTypes(result)
                            addCustomTypes(result, service, virtualFile)
                        }
                        CompletionContext.INSIDE_CONSTANTS -> {
                            addBuiltinTypes(result)
                        }
                        CompletionContext.GENERIC -> {
                            addKeywords(result)
                            addTypes(result, service, virtualFile)
                            addActionDirectives(result)
                            addModuleDirectives(result)
                        }
                    }
                }
            }
        )
    }

    private enum class CompletionContext {
        TOP_LEVEL,
        AFTER_IMPORT,
        INSIDE_MODULE,
        INSIDE_ACTION,
        INSIDE_CONSTANTS,
        AFTER_METHOD_COLON,
        AFTER_TYPE_COLON,
        AFTER_ANNOTATION_AT,
        INSIDE_MODEL,
        GENERIC
    }

    private fun detectContext(before: String, fullLine: String, file: PsiFile): CompletionContext {
        // After "import " or "from " -> suggest import paths
        if (before.startsWith("import ") || before == "import" ||
            before.startsWith("from ") || before == "from") {
            return CompletionContext.AFTER_IMPORT
        }

        // After "method: " -> suggest HTTP methods
        if (before.matches(Regex("""method:\s*\w*"""))) {
            return CompletionContext.AFTER_METHOD_COLON
        }

        // After "input: ", "output: ", "query: " -> suggest types
        if (before.matches(Regex("""(input|output|query):\s*\w*"""))) {
            return CompletionContext.AFTER_TYPE_COLON
        }

        // middleware: values are label names, not types — no special context

        // Annotation completion: "fieldname: Type @" or "fieldname?: Type @something"
        // Triggered when the user types "@" after the type in a field definition
        // Must NOT match import lines (those are handled above)
        if (before.matches(Regex("""[a-z_]\w*\??\s*:\s*\w+.*@\w*$""")) && !before.startsWith("import")) {
            return CompletionContext.AFTER_ANNOTATION_AT
        }

        // Standalone "@" typed inside a model block (annotation trigger)
        if (before.endsWith("@") && !before.startsWith("import")) {
            // Delegate to brace-depth check below — if inside model, return annotation context
            val nestCtx = detectNestingContext(file)
            if (nestCtx == CompletionContext.INSIDE_MODEL) {
                return CompletionContext.AFTER_ANNOTATION_AT
            }
        }

        // After "fieldname: " in a model context -> suggest types
        if (before.matches(Regex("""[a-z_]\w*:\s*\w*"""))) {
            return CompletionContext.AFTER_TYPE_COLON
        }

        // Determine nesting by counting braces up to the cursor line
        return detectNestingContext(file)
    }

    private fun detectNestingContext(file: PsiFile): CompletionContext {
        return try {
            val editor = com.intellij.openapi.fileEditor.FileEditorManager.getInstance(file.project)
                .selectedTextEditor
            val caretOffset = editor?.caretModel?.offset ?: file.textLength
            val textUpToCaret = file.text.substring(0, caretOffset.coerceAtMost(file.textLength))
            val lines = textUpToCaret.split("\n")

            var depth = 0
            var inModule = false
            var inAction = false
            var inModel = false
            var inConstants = false

            for (line in lines) {
                val trimmed = line.trim()
                if (trimmed.startsWith("module ") && trimmed.contains("{")) inModule = true
                if (trimmed.startsWith("model ") && trimmed.contains("{")) inModel = true
                if (trimmed.startsWith("constants ") && trimmed.contains("{")) inConstants = true
                if (trimmed.startsWith("constant ") && trimmed.contains("{")) inConstants = true
                if (trimmed.startsWith("action ") && trimmed.contains("{")) inAction = true

                for (ch in trimmed) {
                    if (ch == '{') depth++
                    if (ch == '}') {
                        depth--
                        if (depth <= 0) { inModule = false; inModel = false; inConstants = false; inAction = false; depth = 0 }
                        if (depth <= 1) inAction = false
                    }
                }
            }

            when {
                inAction -> CompletionContext.INSIDE_ACTION
                inModule -> CompletionContext.INSIDE_MODULE
                inModel -> CompletionContext.INSIDE_MODEL
                inConstants -> CompletionContext.INSIDE_CONSTANTS
                else -> CompletionContext.TOP_LEVEL
            }
        } catch (e: Exception) {
            CompletionContext.TOP_LEVEL
        }
    }

    private fun addKeywords(result: CompletionResultSet) {
        for (kw in VeldLanguageSpec.KEYWORDS) {
            result.addElement(
                LookupElementBuilder.create(kw)
                    .bold()
                    .withIcon(AllIcons.Nodes.Tag)
                    .withTypeText("keyword")
            )
        }
    }

    private fun addImportSnippet(result: CompletionResultSet, service: VeldProjectService, file: com.intellij.openapi.vfs.VirtualFile) {
        val root = service.findProjectRoot(file)
        val aliases = if (root != null) service.readAliases(root) else mapOf("models" to "models", "modules" to "modules")

        for ((aliasName, folder) in aliases) {
            // Only suggest aliases that have an existing directory
            val dirExists = root?.findFileByRelativePath(folder)?.isDirectory == true
            if (!dirExists && root != null) continue

            result.addElement(
                LookupElementBuilder.create("import @$aliasName/*")
                    .withIcon(AllIcons.Nodes.Include)
                    .withTypeText("import all from $folder/")
            )
            result.addElement(
                LookupElementBuilder.create("from @$aliasName import *")
                    .withIcon(AllIcons.Nodes.Include)
                    .withTypeText("from...import syntax")
            )
        }

        result.addElement(
            LookupElementBuilder.create("prefix: ")
                .withIcon(AllIcons.Nodes.Property)
                .withTypeText("global route prefix")
                .withInsertHandler { ctx, _ ->
                    ctx.editor.caretModel.moveToOffset(ctx.tailOffset)
                }
        )
    }

    private fun addImportPaths(result: CompletionResultSet, service: VeldProjectService, file: com.intellij.openapi.vfs.VirtualFile) {
        val root = service.findProjectRoot(file) ?: return
        // Read aliases from veld.config.json (includes defaults + custom aliases)
        val aliases = service.readAliases(root)
        for ((aliasName, folder) in aliases) {
            val dir = root.findFileByRelativePath(folder) ?: continue
            if (!dir.isDirectory) continue

            // Wildcard import for the whole folder
            result.addElement(
                LookupElementBuilder.create("@$aliasName/*")
                    .withIcon(AllIcons.Nodes.Folder)
                    .withTypeText("import all from $folder/")
                    .withTailText("  (wildcard)", true)
            )
            result.addElement(
                LookupElementBuilder.create("/$aliasName/*")
                    .withIcon(AllIcons.Nodes.Folder)
                    .withTypeText("import all from $folder/")
                    .withTailText("  (path wildcard)", true)
            )

            for (child in dir.children) {
                if (child.extension == "veld") {
                    val name = child.nameWithoutExtension
                    // @alias/name style (recommended)
                    result.addElement(
                        LookupElementBuilder.create("@$aliasName/$name")
                            .withIcon(AllIcons.FileTypes.Any_type)
                            .withTypeText("$folder/$name.veld")
                            .withTailText("  (alias)", true)
                    )
                    // /path/name style
                    result.addElement(
                        LookupElementBuilder.create("/$aliasName/$name")
                            .withIcon(AllIcons.FileTypes.Any_type)
                            .withTypeText("$folder/$name.veld")
                            .withTailText("  (path)", true)
                    )
                }
            }
        }
    }

    private fun addModuleDirectives(result: CompletionResultSet) {
        result.addElement(
            LookupElementBuilder.create("description: \"\"")
                .withPresentableText("description:")
                .withIcon(AllIcons.Nodes.Property)
                .withTypeText("Module description")
                .withInsertHandler { ctx, _ ->
                    ctx.editor.caretModel.moveToOffset(ctx.tailOffset - 1)
                }
        )
        result.addElement(
            LookupElementBuilder.create("prefix: /")
                .withPresentableText("prefix:")
                .withIcon(AllIcons.Nodes.Property)
                .withTypeText("Route prefix for all actions")
        )
    }

    private fun addActionDirectives(result: CompletionResultSet) {
        data class Directive(val name: String, val insert: String, val detail: String, val cursorBack: Int)
        val directives = listOf(
            Directive("method", "method: ", "HTTP method (GET, POST, ...)", 0),
            Directive("path", "path: /", "Route path", 0),
            Directive("input", "input: ", "Request body type", 0),
            Directive("output", "output: ", "Response body type", 0),
            Directive("query", "query: ", "Query parameters type", 0),
            Directive("middleware", "middleware: ", "Single middleware", 0),
            Directive("middleware []", "middleware: []", "Middleware list", 1),
            Directive("stream", "stream: ", "Stream output type", 0),
            Directive("errors", "errors: []", "Error codes list", 1),
            Directive("description", "description: \"\"", "Action description", 1),
        )
        for (d in directives) {
            result.addElement(
                LookupElementBuilder.create(d.insert)
                    .withPresentableText("${d.name}:")
                    .withIcon(AllIcons.Nodes.Property)
                    .withTypeText(d.detail)
                    .withInsertHandler(if (d.cursorBack > 0) { ctx, _ ->
                        ctx.editor.caretModel.moveToOffset(ctx.tailOffset - d.cursorBack)
                    } else null)
            )
        }
    }

    private fun addHttpMethods(result: CompletionResultSet) {
        for (m in VeldLanguageSpec.HTTP_METHODS) {
            result.addElement(
                LookupElementBuilder.create(m)
                    .bold()
                    .withIcon(AllIcons.Actions.Execute)
                    .withTypeText("HTTP method")
            )
        }
    }

    private fun addBuiltinTypes(result: CompletionResultSet) {
        for (t in VeldLanguageSpec.BUILTIN_TYPES) {
            result.addElement(
                LookupElementBuilder.create(t)
                    .withIcon(AllIcons.Nodes.Type)
                    .withTypeText("built-in type")
            )
        }
        for (t in VeldLanguageSpec.SPECIAL_TYPES) {
            result.addElement(
                LookupElementBuilder.create("$t<>")
                    .withPresentableText(t)
                    .withIcon(AllIcons.Nodes.Type)
                    .withTypeText("generic type")
                    .withInsertHandler { ctx, _ ->
                        ctx.editor.caretModel.moveToOffset(ctx.tailOffset - 1)
                    }
            )
        }
    }

    private fun addCustomTypes(result: CompletionResultSet, service: VeldProjectService, file: com.intellij.openapi.vfs.VirtualFile) {
        for (model in service.getVisibleModels(file)) {
            val fieldsSummary = model.fields.joinToString(", ") { "${it.name}: ${it.type}" }
            result.addElement(
                LookupElementBuilder.create(model.name)
                    .withIcon(AllIcons.Nodes.Class)
                    .withTypeText("model")
                    .withTailText(" { $fieldsSummary }", true)
            )
        }
        for (enum in service.getVisibleEnums(file)) {
            result.addElement(
                LookupElementBuilder.create(enum.name)
                    .withIcon(AllIcons.Nodes.Enum)
                    .withTypeText("enum")
                    .withTailText(" [${enum.values.joinToString(", ")}]", true)
            )
        }
    }

    private fun addTypes(result: CompletionResultSet, service: VeldProjectService, file: com.intellij.openapi.vfs.VirtualFile) {
        addBuiltinTypes(result)
        addCustomTypes(result, service, file)
    }

    private fun addAnnotationCompletions(result: CompletionResultSet) {
        data class Annotation(val name: String, val snippet: String, val detail: String)
        val annotations = listOf(
            Annotation("@default",    "@default(\$1)",   "Set a default value for this field"),
            Annotation("@example",    "@example(\$1)",   "Provide an example value (shown in docs/OpenAPI)"),
            Annotation("@required",   "@required",       "Mark this field as required (non-nullable)"),
            Annotation("@unique",     "@unique",         "Mark this field as unique in the data store"),
            Annotation("@index",      "@index",          "Add a database index hint for this field"),
            Annotation("@relation",   "@relation(\$1)",  "Define a foreign key relation to another model"),
            Annotation("@deprecated", "@deprecated",     "Mark this field as deprecated"),
            Annotation("@min",        "@min(\$1)",       "Minimum value constraint (int/float)"),
            Annotation("@max",        "@max(\$1)",       "Maximum value constraint (int/float)"),
            Annotation("@minLength",  "@minLength(\$1)", "Minimum string length constraint"),
            Annotation("@maxLength",  "@maxLength(\$1)", "Maximum string length constraint"),
            Annotation("@regex",      "@regex(\$1)",     "Regular expression constraint for strings")
        )
        for (ann in annotations) {
            val item = LookupElementBuilder.create(ann.name)
                .withIcon(AllIcons.Nodes.Annotationtype)
                .withTypeText("annotation")
                .withTailText("  ${ann.detail}", true)
                .withInsertHandler { ctx, _ ->
                    if (ann.snippet.contains("\$1")) {
                        val before = ann.snippet.substringBefore("\$1")
                        val after = ann.snippet.substringAfter("\$1")
                        ctx.document.replaceString(ctx.startOffset, ctx.tailOffset, before + after)
                        ctx.editor.caretModel.moveToOffset(ctx.startOffset + before.length)
                    }
                }
            result.addElement(item)
        }
    }
}
