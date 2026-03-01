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
                            addImportSnippet(result)
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
        AFTER_METHOD_COLON,
        AFTER_TYPE_COLON,
        AFTER_ANNOTATION_AT,
        INSIDE_MODEL,
        GENERIC
    }

    private fun detectContext(before: String, fullLine: String, file: PsiFile): CompletionContext {
        // After "import " -> suggest import paths
        if (before.startsWith("import ") || before == "import") {
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
            val text = file.text
            val lines = text.split("\n")
            var depth = 0
            var inModule = false
            var inAction = false
            var inModel = false

            for (line in lines) {
                val trimmed = line.trim()
                if (trimmed.startsWith("module ") && trimmed.contains("{")) inModule = true
                if (trimmed.startsWith("model ") && trimmed.contains("{")) inModel = true
                if (trimmed.startsWith("action ") && trimmed.contains("{")) inAction = true

                for (ch in trimmed) {
                    if (ch == '{') depth++
                    if (ch == '}') {
                        depth--
                        if (depth <= 0) { inModule = false; inModel = false; inAction = false; depth = 0 }
                        if (depth <= 1) inAction = false
                    }
                }
            }

            when {
                inAction -> CompletionContext.INSIDE_ACTION
                inModule -> CompletionContext.INSIDE_MODULE
                inModel -> CompletionContext.INSIDE_MODEL
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

    private fun addImportSnippet(result: CompletionResultSet) {
        result.addElement(
            LookupElementBuilder.create("import @models/")
                .withIcon(AllIcons.Nodes.Include)
                .withTypeText("import models")
                .withInsertHandler { ctx, _ ->
                    ctx.editor.caretModel.moveToOffset(ctx.tailOffset)
                }
        )
        result.addElement(
            LookupElementBuilder.create("import @modules/")
                .withIcon(AllIcons.Nodes.Include)
                .withTypeText("import modules")
                .withInsertHandler { ctx, _ ->
                    ctx.editor.caretModel.moveToOffset(ctx.tailOffset)
                }
        )
    }

    private fun addImportPaths(result: CompletionResultSet, service: VeldProjectService, file: com.intellij.openapi.vfs.VirtualFile) {
        val root = service.findProjectRoot(file) ?: return
        // Scan all standard alias directories
        for (dirName in listOf("models", "modules", "types", "enums", "schemas", "services", "lib", "common")) {
            val dir = root.findChild(dirName) ?: continue
            for (child in dir.children) {
                if (child.extension == "veld") {
                    val name = child.nameWithoutExtension
                    // @alias/name style (recommended)
                    result.addElement(
                        LookupElementBuilder.create("@$dirName/$name")
                            .withIcon(AllIcons.FileTypes.Any_type)
                            .withTypeText("$dirName/$name.veld")
                            .withTailText("  (alias)", true)
                    )
                    // "./path" style (relative)
                    result.addElement(
                        LookupElementBuilder.create("\"$dirName/$name.veld\"")
                            .withIcon(AllIcons.FileTypes.Any_type)
                            .withTypeText("$dirName/$name.veld")
                            .withTailText("  (relative)", true)
                    )
                }
            }
        }
    }

    private fun addModuleDirectives(result: CompletionResultSet) {
        for (d in listOf("description", "prefix")) {
            result.addElement(
                LookupElementBuilder.create("$d: ")
                    .withIcon(AllIcons.Nodes.Property)
                    .withTypeText("directive")
            )
        }
    }

    private fun addActionDirectives(result: CompletionResultSet) {
        for (d in listOf("method", "path", "input", "output", "query", "middleware", "description")) {
            result.addElement(
                LookupElementBuilder.create("$d: ")
                    .withIcon(AllIcons.Nodes.Property)
                    .withTypeText("directive")
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
            Annotation("@required",   "@required",       "Mark this field as required (non-nullable)"),
            Annotation("@min",        "@min(\$1)",       "Minimum value constraint (int/float)"),
            Annotation("@max",        "@max(\$1)",       "Maximum value constraint (int/float)"),
            Annotation("@minLength",  "@minLength(\$1)", "Minimum string length constraint"),
            Annotation("@maxLength",  "@maxLength(\$1)", "Maximum string length constraint"),
            Annotation("@regex",      "@regex(\$1)",     "Regular expression constraint for strings"),
            Annotation("@unique",     "@unique",         "Mark this field as unique in the data store"),
            Annotation("@deprecated", "@deprecated",     "Mark this field as deprecated")
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
