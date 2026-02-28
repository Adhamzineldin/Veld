package dev.veld.jetbrains

import com.intellij.lexer.LexerBase
import com.intellij.psi.tree.IElementType

/**
 * Simple regex-based lexer for Veld language
 */
class VeldLexer : LexerBase() {
    private var buffer: CharSequence = ""
    private var startOffset: Int = 0
    private var endOffset: Int = 0
    private var currentOffset: Int = 0
    private var currentToken: IElementType? = null

    override fun start(buffer: CharSequence, startOffset: Int, endOffset: Int, initialState: Int) {
        this.buffer = buffer
        this.startOffset = startOffset
        this.endOffset = endOffset
        this.currentOffset = startOffset
        advance()
    }

    override fun getState(): Int = 0

    override fun getTokenType(): IElementType? = currentToken

    override fun getTokenStart(): Int = startOffset

    override fun getTokenEnd(): Int = currentOffset

    override fun advance() {
        startOffset = currentOffset

        if (currentOffset >= endOffset) {
            currentToken = null
            return
        }

        val char = buffer[currentOffset]

        when {
            // Whitespace
            char.isWhitespace() -> {
                while (currentOffset < endOffset && buffer[currentOffset].isWhitespace()) {
                    currentOffset++
                }
                currentToken = VeldTokenTypes.WHITE_SPACE
            }

            // Comments
            char == '/' && currentOffset + 1 < endOffset && buffer[currentOffset + 1] == '/' -> {
                currentOffset += 2
                while (currentOffset < endOffset && buffer[currentOffset] != '\n') {
                    currentOffset++
                }
                currentToken = VeldTokenTypes.COMMENT
            }

            // Strings
            char == '"' -> {
                currentOffset++
                while (currentOffset < endOffset) {
                    if (buffer[currentOffset] == '\\' && currentOffset + 1 < endOffset) {
                        currentOffset += 2
                    } else if (buffer[currentOffset] == '"') {
                        currentOffset++
                        break
                    } else {
                        currentOffset++
                    }
                }
                currentToken = VeldTokenTypes.STRING
            }

            // Identifiers and Keywords
            char.isLetter() || char == '_' -> {
                val wordStart = currentOffset
                while (currentOffset < endOffset &&
                       (buffer[currentOffset].isLetterOrDigit() || buffer[currentOffset] == '_')) {
                    currentOffset++
                }
                val word = buffer.subSequence(wordStart, currentOffset).toString()
                currentToken = when (word) {
                    "model" -> VeldTokenTypes.MODEL_KEYWORD
                    "module" -> VeldTokenTypes.MODULE_KEYWORD
                    "action" -> VeldTokenTypes.ACTION_KEYWORD
                    "enum" -> VeldTokenTypes.ENUM_KEYWORD
                    "import" -> VeldTokenTypes.IMPORT_KEYWORD
                    "extends" -> VeldTokenTypes.EXTENDS_KEYWORD
                    "method", "path", "input", "output", "description", "prefix" ->
                        VeldTokenTypes.DIRECTIVE_KEYWORD
                    "string", "int", "float", "bool", "date", "datetime", "uuid", "bytes", "json", "any" ->
                        VeldTokenTypes.TYPE_KEYWORD
                    "List", "Map" -> VeldTokenTypes.GENERIC_TYPE
                    "GET", "POST", "PUT", "DELETE", "PATCH" -> VeldTokenTypes.HTTP_METHOD
                    else -> VeldTokenTypes.IDENTIFIER
                }
            }

            // Numbers
            char.isDigit() -> {
                while (currentOffset < endOffset && buffer[currentOffset].isDigitOrPoint()) {
                    currentOffset++
                }
                currentToken = VeldTokenTypes.NUMBER
            }

            // Symbols
            char == '{' -> {
                currentOffset++
                currentToken = VeldTokenTypes.LBRACE
            }
            char == '}' -> {
                currentOffset++
                currentToken = VeldTokenTypes.RBRACE
            }
            char == '<' -> {
                currentOffset++
                currentToken = VeldTokenTypes.LT
            }
            char == '>' -> {
                currentOffset++
                currentToken = VeldTokenTypes.GT
            }
            char == ':' -> {
                currentOffset++
                currentToken = VeldTokenTypes.COLON
            }
            char == ',' -> {
                currentOffset++
                currentToken = VeldTokenTypes.COMMA
            }
            char == '@' -> {
                currentOffset++
                currentToken = VeldTokenTypes.AT
            }

            // Default
            else -> {
                currentOffset++
                currentToken = VeldTokenTypes.BAD_CHARACTER
            }
        }
    }

    override fun getBufferSequence(): CharSequence = buffer

    override fun getBufferEnd(): Int = endOffset

    private fun Char.isDigitOrPoint(): Boolean = this.isDigit() || this == '.'
}

