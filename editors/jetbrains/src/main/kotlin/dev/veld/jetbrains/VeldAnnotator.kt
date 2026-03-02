package dev.veld.jetbrains

import com.intellij.lang.annotation.AnnotationHolder
import com.intellij.lang.annotation.Annotator
import com.intellij.lang.annotation.HighlightSeverity
import com.intellij.openapi.editor.DefaultLanguageHighlighterColors
import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.util.TextRange
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFile

/**
 * Real-time in-editor annotator for Veld files.
 *
 * Provides:
 *   • Semantic highlighting — distinct colors for every declaration kind, reference kind,
 *     field names, enum values, HTTP methods, path params, @annotations.
 *   • Validation — undefined types, bad imports, invalid HTTP methods, brace mismatches.
 *
 * The annotator operates on the PsiFile level (whole-file pass) so it can correctly
 * resolve cross-line state (brace depth, enum body, model body).
 */
class VeldAnnotator : Annotator {

    companion object {
        // ── Declaration colors ───────────────────────────────────────────────
        val MODEL_DECLARATION = TextAttributesKey.createTextAttributesKey(
            "VELD_MODEL_DECLARATION", DefaultLanguageHighlighterColors.CLASS_NAME
        )
        val ENUM_DECLARATION = TextAttributesKey.createTextAttributesKey(
            "VELD_ENUM_DECLARATION", DefaultLanguageHighlighterColors.INTERFACE_NAME
        )
        val MODULE_DECLARATION = TextAttributesKey.createTextAttributesKey(
            "VELD_MODULE_DECLARATION", DefaultLanguageHighlighterColors.CLASS_NAME
        )
        val ACTION_NAME = TextAttributesKey.createTextAttributesKey(
            "VELD_ACTION_NAME", DefaultLanguageHighlighterColors.FUNCTION_DECLARATION
        )

        // ── Reference colors ─────────────────────────────────────────────────
        val MODEL_REFERENCE = TextAttributesKey.createTextAttributesKey(
            "VELD_MODEL_REFERENCE", DefaultLanguageHighlighterColors.CLASS_REFERENCE
        )
        val ENUM_REFERENCE = TextAttributesKey.createTextAttributesKey(
            "VELD_ENUM_REFERENCE", DefaultLanguageHighlighterColors.INTERFACE_NAME
        )
        val MODULE_REFERENCE = TextAttributesKey.createTextAttributesKey(
            "VELD_MODULE_REFERENCE", DefaultLanguageHighlighterColors.CLASS_REFERENCE
        )

        // ── Member colors ────────────────────────────────────────────────────
        val FIELD_NAME = TextAttributesKey.createTextAttributesKey(
            "VELD_FIELD_NAME", DefaultLanguageHighlighterColors.INSTANCE_FIELD
        )
        val ENUM_VALUE = TextAttributesKey.createTextAttributesKey(
            "VELD_ENUM_VALUE", DefaultLanguageHighlighterColors.CONSTANT
        )

        // ── Annotation color ─────────────────────────────────────────────────
        val ANNOTATION = TextAttributesKey.createTextAttributesKey(
            "VELD_ANNOTATION", DefaultLanguageHighlighterColors.METADATA
        )

        // ── Path parameter color ─────────────────────────────────────────────
        val PATH_PARAM = TextAttributesKey.createTextAttributesKey(
            "VELD_PATH_PARAM", DefaultLanguageHighlighterColors.PARAMETER
        )
    }

    // Simple context tracker so we highlight enum values vs. model fields correctly.
    private enum class BlockKind { NONE, MODEL, ENUM, MODULE, ACTION }

    override fun annotate(element: PsiElement, holder: AnnotationHolder) {
        if (element !is PsiFile) return
        if (element.virtualFile?.extension != "veld") return

        val project = element.project
        val virtualFile = element.virtualFile ?: return
        val service = VeldProjectService.getInstance(project)
        val content = element.text
        val lines = content.split("\n")

        // Keep the index fresh so visible-type lookups are accurate.
        service.reindexFile(virtualFile, content)

        val visibleModels = service.getVisibleModels(virtualFile).map { it.name }.toSet()
        val visibleEnums  = service.getVisibleEnums(virtualFile).map  { it.name }.toSet()
        val allTypes      = visibleModels + visibleEnums +
                            VeldLanguageSpec.BUILTIN_TYPES + VeldLanguageSpec.SPECIAL_TYPES

        // ── Line-by-line pass ────────────────────────────────────────────────
        var offset     = 0
        var braceDepth = 0
        var blockStack = ArrayDeque<BlockKind>()  // tracks nested block kinds

        for (i in lines.indices) {
            val line        = lines[i]
            val trimmed     = line.trim()
            val lineStart   = offset
            val indentLen   = line.length - line.trimStart().length

            val openBraces  = trimmed.count { it == '{' }
            val closeBraces = trimmed.count { it == '}' }

            // Current block context (before brace adjustments this line)
            val currentBlock = blockStack.lastOrNull() ?: BlockKind.NONE

            // ── Semantic highlighting ────────────────────────────────────────

            when {
                // ── model declaration ────────────────────────────────────────
                trimmed.startsWith("model ") -> {
                    val m = Regex("""^model\s+([A-Za-z_]\w*)""").find(trimmed)
                    if (m != null) {
                        highlightWord(m.groupValues[1], line, lineStart, content.length, holder, MODEL_DECLARATION)
                        // extends parent
                        val ex = Regex("""extends\s+([A-Za-z_]\w*)""").find(trimmed)
                        if (ex != null) {
                            highlightWord(
                                ex.groupValues[1], line, lineStart, content.length, holder, MODEL_REFERENCE,
                                searchFrom = line.indexOf("extends")
                            )
                        }
                    }
                    if (openBraces > closeBraces) blockStack.addLast(BlockKind.MODEL)
                }

                // ── enum declaration ─────────────────────────────────────────
                trimmed.startsWith("enum ") -> {
                    val m = Regex("""^enum\s+([A-Za-z_]\w*)""").find(trimmed)
                    if (m != null) {
                        highlightWord(m.groupValues[1], line, lineStart, content.length, holder, ENUM_DECLARATION)
                    }
                    if (openBraces > closeBraces) {
                        blockStack.addLast(BlockKind.ENUM)
                        // Single-line enum: enum Role { admin user guest }
                        val inlineValues = Regex("""\{([^}]*)\}""").find(trimmed)?.groupValues?.get(1)
                        if (inlineValues != null) {
                            highlightEnumValues(inlineValues, line, lineStart, content.length,
                                line.indexOf('{'), holder)
                            blockStack.removeLast()  // immediately closed
                        }
                    }
                }

                // ── module declaration ────────────────────────────────────────
                trimmed.startsWith("module ") -> {
                    val m = Regex("""^module\s+([A-Za-z_]\w*)""").find(trimmed)
                    if (m != null) {
                        highlightWord(m.groupValues[1], line, lineStart, content.length, holder, MODULE_DECLARATION)
                    }
                    if (openBraces > closeBraces) blockStack.addLast(BlockKind.MODULE)
                }

                // ── action declaration ────────────────────────────────────────
                trimmed.startsWith("action ") -> {
                    val m = Regex("""^action\s+([A-Za-z_]\w*)""").find(trimmed)
                    if (m != null) {
                        highlightWord(m.groupValues[1], line, lineStart, content.length, holder, ACTION_NAME)
                    }
                    if (openBraces > closeBraces) blockStack.addLast(BlockKind.ACTION)
                }

                // ── import / from-import ──────────────────────────────────
                trimmed.startsWith("import") || trimmed.startsWith("from") -> {
                    validateImport(trimmed, line, lineStart, content.length, virtualFile, service, holder)
                }

                // ── top-level prefix: /api/v1 ────────────────────────────────
                trimmed.startsWith("prefix:") && blockStack.isEmpty() -> {
                    // Valid top-level prefix directive — no special handling needed
                }

                // ── inside enum body: enum values ────────────────────────────
                currentBlock == BlockKind.ENUM && trimmed != "{" && trimmed != "}" &&
                        trimmed.isNotEmpty() && !trimmed.startsWith("//") -> {
                    highlightEnumValues(trimmed, line, lineStart, content.length, indentLen, holder)
                }

                // ── inside model/action body: directives and fields ───────────
                currentBlock == BlockKind.MODEL || currentBlock == BlockKind.ACTION ||
                currentBlock == BlockKind.MODULE -> {
                    handleDirectiveOrField(
                        trimmed, line, lineStart, content.length,
                        visibleModels, visibleEnums, allTypes, holder
                    )
                }

                // ── closing braces ────────────────────────────────────────────
                trimmed == "}" -> { /* handled below */ }
            }

            // ── Brace depth tracking ─────────────────────────────────────────
            braceDepth += openBraces - closeBraces
            repeat(closeBraces) {
                if (blockStack.isNotEmpty()) blockStack.removeLast()
            }

            offset += line.length + 1  // +1 for \n
        }

        // ── Unclosed brace validation ────────────────────────────────────────
        if (braceDepth > 0) {
            val endOffset = content.length
            holder.newAnnotation(
                HighlightSeverity.ERROR,
                "Unclosed brace: $braceDepth opening brace(s) without matching closing brace(s)"
            ).range(TextRange((endOffset - (lines.lastOrNull()?.length ?: 0)).coerceAtLeast(0), endOffset))
                .create()
        } else if (braceDepth < 0) {
            holder.newAnnotation(
                HighlightSeverity.ERROR,
                "Extra closing brace(s): ${-braceDepth} closing brace(s) without matching opening brace(s)"
            ).range(TextRange(0, 1.coerceAtMost(content.length))).create()
        }
    }

    // ── Directive / field handling ───────────────────────────────────────────

    private fun handleDirectiveOrField(
        trimmed: String, line: String, lineStart: Int, contentLen: Int,
        models: Set<String>, enums: Set<String>, allTypes: Set<String>,
        holder: AnnotationHolder
    ) {
        if (trimmed.startsWith("//") || trimmed == "{" || trimmed == "}" || trimmed.isEmpty()) return

        // ── method directive ──────────────────────────────────────────────────
        if (trimmed.startsWith("method:")) {
            val m = Regex("""method:\s*(\w+)""").find(trimmed)
            if (m != null) {
                val method = m.groupValues[1]
                if (!VeldLanguageSpec.isHttpMethod(method)) {
                    val start = lineStart + line.indexOf(method)
                    val end   = start + method.length
                    if (start >= 0 && end <= contentLen) {
                        holder.newAnnotation(
                            HighlightSeverity.ERROR,
                            "Invalid HTTP method '$method'. Valid: ${VeldLanguageSpec.HTTP_METHODS.joinToString(", ")}"
                        ).range(TextRange(start, end)).create()
                    }
                }
            }
            return
        }

        // ── path directive: highlight :param segments ─────────────────────────
        if (trimmed.startsWith("path:") || trimmed.startsWith("prefix:")) {
            val colonIdx = trimmed.indexOf(':')
            val pathVal  = trimmed.substring(colonIdx + 1).trim()
            highlightPathParams(pathVal, line, lineStart, contentLen, holder)
            return
        }

        // ── input / output / query directives — validate as type references ──
        if (trimmed.startsWith("input:") || trimmed.startsWith("output:") ||
            trimmed.startsWith("query:")
        ) {
            val colonIdx = trimmed.indexOf(':')
            if (colonIdx >= 0) {
                val typeExpr = trimmed.substring(colonIdx + 1).trim()
                highlightTypeReferences(typeExpr, line, lineStart, contentLen, models, enums, holder)
                validateTypeExpression(typeExpr, line, lineStart, contentLen, allTypes, holder)
            }
            return
        }

        // ── middleware directive — just a label name, not a type reference ───
        if (trimmed.startsWith("middleware:")) return

        // ── stream directive — type reference ────────────────────────────────
        if (trimmed.startsWith("stream:")) {
            val colonIdx = trimmed.indexOf(':')
            if (colonIdx >= 0) {
                val typeExpr = trimmed.substring(colonIdx + 1).trim()
                highlightTypeReferences(typeExpr, line, lineStart, contentLen, models, enums, holder)
                validateTypeExpression(typeExpr, line, lineStart, contentLen, allTypes, holder)
            }
            return
        }

        // ── errors directive — list of error names, not type references ──────
        if (trimmed.startsWith("errors:")) return

        // ── description / prefix directives: no special highlighting ─────────
        if (trimmed.startsWith("description:")) return

        // ── field definition inside a model ──────────────────────────────────
        val fieldMatch = Regex("""^([a-z_]\w*)(\??):\s*(.+?)(?:\s*//.*)?$""").find(trimmed)
        if (fieldMatch != null) {
            val fieldName    = fieldMatch.groupValues[1]
            val rawTypeExpr  = fieldMatch.groupValues[3].trim()

            // Highlight field name
            highlightWord(fieldName, line, lineStart, contentLen, holder, FIELD_NAME)

            // Strip @annotations (e.g. @default(user)) before type processing
            val typeExpr = rawTypeExpr.replace(Regex("""@\w+\([^)]*\)"""), "").trim()

            // Highlight type references
            highlightTypeReferences(typeExpr, line, lineStart, contentLen, models, enums, holder)

            // Validate types
            validateTypeExpression(typeExpr, line, lineStart, contentLen, allTypes, holder)

            // Highlight @annotation(...) clauses on this field line
            highlightAnnotations(rawTypeExpr, line, lineStart, contentLen, holder)
        }
    }

    // ── Enum value highlighting ──────────────────────────────────────────────

    private fun highlightEnumValues(
        body: String, fullLine: String, lineStart: Int, contentLen: Int,
        searchFromCol: Int, holder: AnnotationHolder
    ) {
        for (m in Regex("""\b([a-zA-Z_]\w*)\b""").findAll(body)) {
            val start = lineStart + fullLine.indexOf(m.value, searchFromCol)
            val end   = start + m.value.length
            if (start >= lineStart && end <= contentLen && start < end) {
                highlightRange(holder, TextRange(start, end), ENUM_VALUE)
            }
        }
    }

    // ── @annotation(...) highlighting and validation ─────────────────────────

    private fun highlightAnnotations(
        typeExprRaw: String, line: String, lineStart: Int, contentLen: Int,
        holder: AnnotationHolder
    ) {
        if (!typeExprRaw.contains('@')) return
        val colonIdx = line.indexOf(':')
        for (m in Regex("""@\w+(?:\([^)]*\))?""").findAll(typeExprRaw)) {
            val searchFrom = if (colonIdx >= 0) colonIdx else 0
            val start = lineStart + line.indexOf(m.value, searchFrom)
            val end   = start + m.value.length
            if (start >= lineStart && end <= contentLen && start < end) {
                highlightRange(holder, TextRange(start, end), ANNOTATION)
                // Validate annotation name
                val annName = m.value.substringAfter('@').substringBefore('(')
                if (annName !in VeldLanguageSpec.KNOWN_ANNOTATIONS) {
                    holder.newAnnotation(
                        HighlightSeverity.WARNING,
                        "Unknown annotation '@$annName'. Known: ${VeldLanguageSpec.KNOWN_ANNOTATIONS.joinToString(", ") { "@$it" }}"
                    ).range(TextRange(start, end)).create()
                }
            }
        }
    }

    // ── Path :param highlighting ─────────────────────────────────────────────

    private fun highlightPathParams(
        pathVal: String, line: String, lineStart: Int, contentLen: Int,
        holder: AnnotationHolder
    ) {
        for (m in Regex(""":([\w]+)""").findAll(pathVal)) {
            val searchFrom = line.indexOf(':').let { if (it >= 0) it else 0 }
            val colonStart = lineStart + line.indexOf(":" + m.groupValues[1], searchFrom)
            val end        = colonStart + m.groupValues[1].length + 1
            if (colonStart >= lineStart && end <= contentLen && colonStart < end) {
                highlightRange(holder, TextRange(colonStart, end), PATH_PARAM)
            }
        }
    }

    // ── Type reference highlighting ──────────────────────────────────────────

    private fun highlightTypeReferences(
        typeExpr: String, line: String, lineStart: Int, contentLen: Int,
        models: Set<String>, enums: Set<String>, holder: AnnotationHolder
    ) {
        val colonIdx = line.indexOf(':')
        val searchFrom = if (colonIdx >= 0) colonIdx else 0

        for (m in Regex("""[A-Za-z_]\w*""").findAll(typeExpr)) {
            val typeName = m.value
            val start    = lineStart + line.indexOf(typeName, searchFrom)
            val end      = start + typeName.length
            if (start < lineStart || end > contentLen || start >= end) continue

            val key = when {
                models.contains(typeName) -> MODEL_REFERENCE
                enums.contains(typeName)  -> ENUM_REFERENCE
                VeldLanguageSpec.isSpecialType(typeName) ->
                    TextAttributesKey.createTextAttributesKey("VELD_GENERIC",
                        DefaultLanguageHighlighterColors.CLASS_NAME)
                else -> null
            }
            if (key != null) highlightRange(holder, TextRange(start, end), key)
        }
    }

    // ── Import validation ────────────────────────────────────────────────────

    private fun validateImport(
        trimmed: String, line: String, lineStart: Int, contentLen: Int,
        virtualFile: com.intellij.openapi.vfs.VirtualFile,
        service: VeldProjectService,
        holder: AnnotationHolder
    ) {
        // Accept all valid import formats:
        //   import @models/user    import @models/*    import @models
        //   import /models/user    import /models/*    import /models
        //   import models/user     import models/*     import models
        //   from @models import *  from /models import user  from models import *
        //   import "legacy/path.veld"
        val validPatterns = listOf(
            Regex("""import\s+@\w+(/[\w*]+)?"""),           // import @alias[/name|/*]
            Regex("""import\s+/\w+(/[\w*]+)?"""),           // import /path[/name|/*]
            Regex("""import\s+\w+(/[\w*]+)?"""),            // import alias[/name|/*]
            Regex("""import\s+"[^"]+""""),                   // import "legacy"
            Regex("""from\s+@?\w+\s+import\s+(\*|\w+)"""), // from @?alias import *|name
            Regex("""from\s+/\w+\s+import\s+(\*|\w+)""")   // from /path import *|name
        )
        if (validPatterns.any { it.matches(trimmed) }) {
            // Valid syntax — optionally validate resolved file exists
            val singleFile = Regex("""import\s+(@\w+/\w+)""").find(trimmed)
            if (singleFile != null) {
                val path = singleFile.groupValues[1]
                val resolved = service.resolveImport(path, virtualFile)
                if (resolved == null || !resolved.exists()) {
                    val atIdx = line.indexOf('@')
                    val start = lineStart + atIdx
                    val end   = lineStart + line.trimEnd().length
                    if (atIdx >= 0 && start < end && end <= contentLen) {
                        holder.newAnnotation(HighlightSeverity.WARNING,
                            "Cannot resolve import '$path'. File may not exist yet.")
                            .range(TextRange(start, end)).create()
                    }
                }
            }
        }
    }

    // ── Type validation ──────────────────────────────────────────────────────

    private fun validateTypeExpression(
        typeExpr: String, line: String, lineStart: Int, contentLen: Int,
        allTypes: Set<String>, holder: AnnotationHolder
    ) {
        // Strip @annotations before validation so @default(user) doesn't cause false errors
        val clean = typeExpr.replace(Regex("""@\w+\([^)]*\)"""), "").trim()

        val colonIdx   = line.indexOf(':')
        val searchFrom = if (colonIdx >= 0) colonIdx else 0

        for (m in Regex("""[A-Za-z_]\w*""").findAll(clean)) {
            val typeName = m.value
            if (typeName in allTypes) continue
            // Lowercase builtins are already in allTypes, but double-check
            if (VeldLanguageSpec.isBuiltinType(typeName)) continue
            // Generic containers (List, Map) are in SPECIAL_TYPES → allTypes
            if (VeldLanguageSpec.isSpecialType(typeName)) continue

            val start = lineStart + line.indexOf(typeName, searchFrom)
            val end   = start + typeName.length
            if (start >= lineStart && end <= contentLen && start < end) {
                holder.newAnnotation(HighlightSeverity.ERROR,
                    "Type '$typeName' is not defined. Did you forget to import?")
                    .range(TextRange(start, end)).create()
            }
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    /** Highlight a word in the raw line at lineStart, searching from searchFrom column. */
    private fun highlightWord(
        word: String, line: String, lineStart: Int, contentLen: Int,
        holder: AnnotationHolder, key: TextAttributesKey,
        searchFrom: Int = 0
    ) {
        val idx   = line.indexOf(word, searchFrom)
        if (idx < 0) return
        val start = lineStart + idx
        val end   = start + word.length
        if (start >= 0 && end <= contentLen && start < end) {
            highlightRange(holder, TextRange(start, end), key)
        }
    }

    private fun highlightRange(holder: AnnotationHolder, range: TextRange, key: TextAttributesKey) {
        holder.newSilentAnnotation(HighlightSeverity.INFORMATION)
            .range(range)
            .textAttributes(key)
            .create()
    }
}
