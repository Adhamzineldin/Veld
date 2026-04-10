package dev.veld.jetbrains

import com.intellij.lang.documentation.AbstractDocumentationProvider
import com.intellij.lang.documentation.DocumentationMarkup
import com.intellij.openapi.editor.Editor
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFile

/**
 * Documentation provider for Veld language.
 * Shows rich hover info for models, enums, modules, types, keywords, and import paths.
 */
class VeldDocumentationProvider : AbstractDocumentationProvider() {

    override fun generateDoc(element: PsiElement?, originalElement: PsiElement?): String? {
        val target = originalElement ?: element ?: return null
        val file = target.containingFile?.virtualFile ?: return null
        val project = target.project
        val service = VeldProjectService.getInstance(project)

        val text = target.text ?: return null
        val document = target.containingFile?.viewProvider?.document ?: return null
        val offset = target.textOffset
        val lineNumber = document.getLineNumber(offset)
        val lineStart = document.getLineStartOffset(lineNumber)
        val lineEnd = document.getLineEndOffset(lineNumber)
        val lineText = document.getText(com.intellij.openapi.util.TextRange(lineStart, lineEnd)).trim()

        // Import path hover: show what the import resolves to and its contents
        if (lineText.startsWith("import")) {
            val importMatch = Regex("""import\s+@(\w+)/(\w+)""").find(lineText)
            if (importMatch != null) {
                val alias = importMatch.groupValues[1]
                val name = importMatch.groupValues[2]
                val importPath = "@$alias/$name"
                val resolved = service.resolveImport(importPath, file)

                val sb = StringBuilder()
                sb.append(DocumentationMarkup.DEFINITION_START)
                sb.append("import <b>$importPath</b>")
                sb.append(DocumentationMarkup.DEFINITION_END)

                sb.append(DocumentationMarkup.CONTENT_START)
                if (resolved != null && resolved.exists()) {
                    sb.append("Resolves to: <code>${resolved.name}</code><br/><br/>")

                    service.reindexFile(resolved)
                    val importedIndex = service.getIndex(resolved)
                    if (importedIndex != null) {
                        if (importedIndex.models.isNotEmpty()) {
                            sb.append("<b>Models:</b> ")
                            sb.append(importedIndex.models.joinToString(", ") { "<code>${it.name}</code>" })
                            sb.append("<br/>")
                        }
                        if (importedIndex.enums.isNotEmpty()) {
                            sb.append("<b>Enums:</b> ")
                            sb.append(importedIndex.enums.joinToString(", ") { "<code>${it.name}</code>" })
                            sb.append("<br/>")
                        }
                    }
                } else {
                    sb.append("<i>File not found</i>")
                }
                sb.append(DocumentationMarkup.CONTENT_END)
                return sb.toString()
            }
        }

        // Find the word under cursor
        val word = extractWord(target)
        if (word.isNullOrEmpty()) return null

        // Model hover
        val models = service.getVisibleModels(file)
        val model = models.find { it.name == word }
        if (model != null) {
            return buildModelDoc(model)
        }

        // Enum hover
        val enums = service.getVisibleEnums(file)
        val enumDef = enums.find { it.name == word }
        if (enumDef != null) {
            return buildEnumDoc(enumDef)
        }

        // Module hover
        val modules = service.getVisibleModules(file)
        val moduleDef = modules.find { it.name == word }
        if (moduleDef != null) {
            return buildModuleDoc(moduleDef)
        }

        // Built-in type hover
        if (VeldLanguageSpec.isBuiltinType(word)) {
            return buildSimpleDoc("Built-in Type", word, "Primitive type in the Veld language")
        }

        // HTTP method hover
        if (VeldLanguageSpec.isHttpMethod(word)) {
            val descriptions = mapOf(
                "GET" to "Retrieve a resource",
                "POST" to "Create a new resource",
                "PUT" to "Replace a resource",
                "DELETE" to "Remove a resource",
                "PATCH" to "Partially update a resource",
                "HEAD" to "Retrieve headers only",
                "OPTIONS" to "Retrieve supported methods"
            )
            return buildSimpleDoc("HTTP Method", word, descriptions[word] ?: "")
        }

        // Special type hover
        if (VeldLanguageSpec.isSpecialType(word)) {
            val descriptions = mapOf(
                "List" to "A generic list/array type: List&lt;T&gt;",
                "Map" to "A generic key-value map type: Map&lt;K, V&gt;"
            )
            return buildSimpleDoc("Generic Type", "$word<T>", descriptions[word] ?: "")
        }

        // Keyword hover
        if (VeldLanguageSpec.isKeyword(word)) {
            val descriptions = mapOf(
                "model" to "Defines a data model with typed fields",
                "module" to "Groups related API actions under a common prefix",
                "action" to "Defines an API endpoint with method, path, input, and output",
                "enum" to "Defines an enumeration of named values",
                "import" to "Imports models or modules from other .veld files",
                "extends" to "Inherits fields from a parent model"
            )
            return buildSimpleDoc("Keyword", word, descriptions[word] ?: "")
        }

        // Directive hover
        if (VeldLanguageSpec.isDirective(word)) {
            val descriptions = mapOf(
                "description" to "A human-readable description of the model, module, or action",
                "prefix" to "The URL prefix for all actions in a module",
                "method" to "The HTTP method (GET, POST, PUT, DELETE, PATCH)",
                "path" to "The URL path for this action (relative to module prefix)",
                "input" to "The input/request body type for this action",
                "output" to "The output/response body type for this action",
                "response" to "The output/response body type for this action (alias for output)",
                "default" to "The default value for a field or enum"
            )
            return buildSimpleDoc("Directive", word, descriptions[word] ?: "")
        }

        return null
    }

    override fun getCustomDocumentationElement(editor: Editor, file: PsiFile, contextElement: PsiElement?, targetOffset: Int): PsiElement? {
        return contextElement
    }

    private fun extractWord(element: PsiElement): String? {
        val text = element.text ?: return null
        // If the element text is already a single word, use it
        if (text.matches(Regex("""[A-Za-z_]\w*"""))) return text
        // Try to find a word around the element
        val match = Regex("""[A-Za-z_]\w*""").find(text)
        return match?.value
    }

    private fun buildModelDoc(model: VeldProjectService.ModelDef): String {
        val sb = StringBuilder()
        sb.append(DocumentationMarkup.DEFINITION_START)
        sb.append("model <b>${model.name}</b>")
        sb.append(DocumentationMarkup.DEFINITION_END)

        sb.append(DocumentationMarkup.CONTENT_START)
        sb.append("<b>Defined in:</b> <code>${model.file.name}</code>, line ${model.line + 1}<br/><br/>")

        if (model.fields.isEmpty()) {
            sb.append("<i>No fields defined</i>")
        } else {
            sb.append("<table style='border-collapse:collapse'>")
            sb.append("<tr><th align='left'>Field</th><th align='left'>Type</th></tr>")
            for (field in model.fields) {
                val optional = if (field.type.endsWith("?") || field.name.endsWith("?")) "?" else ""
                sb.append("<tr>")
                sb.append("<td><code><b>${field.name}</b>$optional</code></td>")
                sb.append("<td><code>${field.type.removeSuffix("?")}</code></td>")
                sb.append("</tr>")
            }
            sb.append("</table>")
        }
        sb.append(DocumentationMarkup.CONTENT_END)

        return sb.toString()
    }

    private fun buildEnumDoc(enumDef: VeldProjectService.EnumDef): String {
        val sb = StringBuilder()
        sb.append(DocumentationMarkup.DEFINITION_START)
        sb.append("enum <b>${enumDef.name}</b>")
        sb.append(DocumentationMarkup.DEFINITION_END)

        sb.append(DocumentationMarkup.CONTENT_START)
        sb.append("<b>Defined in:</b> <code>${enumDef.file.name}</code> (line ${enumDef.line + 1})<br/>")
        sb.append("<b>Values:</b> ${enumDef.values.joinToString(", ") { "<code>$it</code>" }}<br/><br/>")
        sb.append("<pre><code>")
        sb.append("enum ${enumDef.name} {\n")
        for (value in enumDef.values) {
            sb.append("  $value\n")
        }
        sb.append("}")
        sb.append("</code></pre>")
        sb.append(DocumentationMarkup.CONTENT_END)

        return sb.toString()
    }

    private fun buildModuleDoc(moduleDef: VeldProjectService.ModuleDef): String {
        val sb = StringBuilder()
        sb.append(DocumentationMarkup.DEFINITION_START)
        sb.append("module <b>${moduleDef.name}</b>")
        sb.append(DocumentationMarkup.DEFINITION_END)

        sb.append(DocumentationMarkup.CONTENT_START)
        sb.append("<b>Defined in:</b> <code>${moduleDef.file.name}</code> (line ${moduleDef.line + 1})<br/>")
        sb.append("<b>Actions:</b> ${moduleDef.actions.size}<br/><br/>")

        if (moduleDef.actions.isNotEmpty()) {
            sb.append("<table>")
            sb.append("<tr><th>Action</th><th>Method</th><th>Path</th><th>Input</th><th>Output</th></tr>")
            for (action in moduleDef.actions) {
                sb.append("<tr>")
                sb.append("<td><b>${action.name}</b></td>")
                sb.append("<td><code>${action.method ?: "-"}</code></td>")
                sb.append("<td><code>${action.path ?: "-"}</code></td>")
                sb.append("<td>${action.input ?: "-"}</td>")
                sb.append("<td>${action.output ?: "-"}</td>")
                sb.append("</tr>")
            }
            sb.append("</table>")
        }
        sb.append(DocumentationMarkup.CONTENT_END)

        return sb.toString()
    }

    private fun buildSimpleDoc(kind: String, name: String, description: String): String {
        val sb = StringBuilder()
        sb.append(DocumentationMarkup.DEFINITION_START)
        sb.append("$kind <b>$name</b>")
        sb.append(DocumentationMarkup.DEFINITION_END)

        if (description.isNotEmpty()) {
            sb.append(DocumentationMarkup.CONTENT_START)
            sb.append(description)
            sb.append(DocumentationMarkup.CONTENT_END)
        }

        return sb.toString()
    }
}

