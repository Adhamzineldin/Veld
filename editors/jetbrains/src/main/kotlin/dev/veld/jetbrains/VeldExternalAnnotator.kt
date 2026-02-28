package dev.veld.jetbrains

import com.intellij.lang.annotation.AnnotationHolder
import com.intellij.lang.annotation.ExternalAnnotator
import com.intellij.lang.annotation.HighlightSeverity
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.util.TextRange
import com.intellij.psi.PsiFile
import java.io.BufferedReader
import java.io.InputStreamReader

/**
 * External annotator that runs `veld validate` and displays errors
 */
class VeldExternalAnnotator : ExternalAnnotator<PsiFile, List<VeldValidationError>>() {

    override fun collectInformation(file: PsiFile): PsiFile = file

    override fun doAnnotate(collectedInfo: PsiFile): List<VeldValidationError>? {
        val project = collectedInfo.project
        val basePath = project.basePath ?: return null

        return try {
            val process = ProcessBuilder("veld", "validate")
                .directory(java.io.File(basePath))
                .redirectErrorStream(true)
                .start()

            val reader = BufferedReader(InputStreamReader(process.inputStream))
            val errors = mutableListOf<VeldValidationError>()

            // Parse veld validate output: "error: file.veld:10: message"
            val errorRegex = Regex("""([^\s:]+\.veld):(\d+):\s+(.+)""")

            reader.lineSequence().forEach { line ->
                val cleanLine = line.replace(Regex("\u001B\\[[0-9;]*m"), "") // Remove ANSI codes
                errorRegex.find(cleanLine)?.let { match ->
                    val (fileName, lineNum, message) = match.destructured
                    errors.add(VeldValidationError(fileName, lineNum.toInt(), message))
                }
            }

            process.waitFor()
            errors

        } catch (e: Exception) {
            null // veld not found or other error
        }
    }

    override fun apply(file: PsiFile, annotationResult: List<VeldValidationError>?, holder: AnnotationHolder) {
        annotationResult?.forEach { error ->
            if (error.fileName.endsWith(file.name)) {
                val lineStartOffset = getLineStartOffset(file.text, error.line)
                if (lineStartOffset >= 0) {
                    val lineEndOffset = file.text.indexOf('\n', lineStartOffset).let {
                        if (it < 0) file.text.length else it
                    }

                    holder.newAnnotation(HighlightSeverity.ERROR, error.message)
                        .range(TextRange(lineStartOffset, lineEndOffset))
                        .create()
                }
            }
        }
    }

    private fun getLineStartOffset(text: String, line: Int): Int {
        var currentLine = 1
        var offset = 0

        while (currentLine < line && offset < text.length) {
            if (text[offset] == '\n') {
                currentLine++
            }
            offset++
        }

        return if (currentLine == line) offset else -1
    }
}

data class VeldValidationError(
    val fileName: String,
    val line: Int,
    val message: String
)

