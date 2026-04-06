package dev.veld.jetbrains

import com.intellij.lexer.LexerBase
import com.intellij.psi.tree.IElementType

/**
 * Lexer for Veld language.
 * Handles all Veld tokens including path literals (/foo/:id) and import paths (@models/auth).
 */
class VeldLexer : LexerBase() {
    private var buffer: CharSequence = ""
    private var startOffset: Int = 0
    private var endOffset: Int = 0
    private var currentOffset: Int = 0
    private var currentToken: IElementType? = null

    // Track state for context-sensitive lexing
    private var lastSignificantToken: IElementType? = null
    // Track if we are expecting a path literal (after path: or prefix:)
    private var expectPathLiteral: Boolean = false

    override fun start(buffer: CharSequence, startOffset: Int, endOffset: Int, initialState: Int) {
        this.buffer = buffer
        this.startOffset = startOffset
        this.endOffset = endOffset
        this.currentOffset = startOffset
        this.lastSignificantToken = null
        this.expectPathLiteral = false
        advance()
    }

    override fun getState(): Int = if (expectPathLiteral) 1 else 0
    override fun getTokenType(): IElementType? = currentToken
    override fun getTokenStart(): Int = startOffset
    override fun getTokenEnd(): Int = currentOffset
    override fun getBufferSequence(): CharSequence = buffer
    override fun getBufferEnd(): Int = endOffset

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
                    // Reset path expectation on newline
                    if (buffer[currentOffset] == '\n') {
                        expectPathLiteral = false
                    }
                    currentOffset++
                }
                currentToken = VeldTokenTypes.WHITE_SPACE
            }

            // Line comment
            char == '/' && peek(1) == '/' -> {
                currentOffset += 2
                while (currentOffset < endOffset && buffer[currentOffset] != '\n') {
                    currentOffset++
                }
                currentToken = VeldTokenTypes.COMMENT
                expectPathLiteral = false
            }

            // Block comment
            char == '/' && peek(1) == '*' -> {
                currentOffset += 2
                while (currentOffset < endOffset - 1) {
                    if (buffer[currentOffset] == '*' && buffer[currentOffset + 1] == '/') {
                        currentOffset += 2
                        break
                    }
                    currentOffset++
                }
                // Handle unterminated block comment
                if (currentOffset >= endOffset - 1 && !(currentOffset >= 2 && buffer[currentOffset - 1] == '/' && buffer[currentOffset - 2] == '*')) {
                    currentOffset = endOffset
                }
                currentToken = VeldTokenTypes.COMMENT
                expectPathLiteral = false
            }

            // String literal
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
                lastSignificantToken = currentToken
                expectPathLiteral = false
            }

            // @ sign - could be start of import path like @models/auth
            // Recognized after: import keyword, colon (type position), extends keyword
            char == '@' -> {
                if (lastSignificantToken == VeldTokenTypes.IMPORT_KEYWORD ||
                    lastSignificantToken == VeldTokenTypes.COLON ||
                    lastSignificantToken == VeldTokenTypes.EXTENDS_KEYWORD) {
                    // Consume whole import path: @alias/name
                    currentOffset++ // skip @
                    while (currentOffset < endOffset &&
                        (buffer[currentOffset].isLetterOrDigit() || buffer[currentOffset] == '_' ||
                                buffer[currentOffset] == '/' || buffer[currentOffset] == '-')) {
                        currentOffset++
                    }
                    currentToken = VeldTokenTypes.IMPORT_PATH
                } else {
                    currentOffset++
                    currentToken = VeldTokenTypes.AT
                }
                lastSignificantToken = currentToken
                expectPathLiteral = false
            }

            // Path literal: starts with / when expecting a path (after path: or prefix:)
            char == '/' && expectPathLiteral -> {
                // Consume the entire path: /foo/bar/:id
                while (currentOffset < endOffset &&
                    !buffer[currentOffset].isWhitespace() &&
                    buffer[currentOffset] != '{' && buffer[currentOffset] != '}') {
                    currentOffset++
                }
                currentToken = VeldTokenTypes.PATH_LITERAL
                lastSignificantToken = currentToken
                expectPathLiteral = false
            }

            // Path literal fallback: / after colon when buffer scan confirms path/prefix context
            char == '/' && isInPathContext() -> {
                while (currentOffset < endOffset &&
                    !buffer[currentOffset].isWhitespace() &&
                    buffer[currentOffset] != '{' && buffer[currentOffset] != '}') {
                    currentOffset++
                }
                currentToken = VeldTokenTypes.PATH_LITERAL
                lastSignificantToken = currentToken
                expectPathLiteral = false
            }

            // Forward slash (standalone, not in path context)
            char == '/' -> {
                currentOffset++
                currentToken = VeldTokenTypes.SLASH
                lastSignificantToken = currentToken
            }

            // Identifiers and keywords
            char.isLetter() || char == '_' -> {
                val wordStart = currentOffset
                while (currentOffset < endOffset &&
                    (buffer[currentOffset].isLetterOrDigit() || buffer[currentOffset] == '_')) {
                    currentOffset++
                }
                val word = buffer.subSequence(wordStart, currentOffset).toString()
                currentToken = classifyWord(word)
                lastSignificantToken = currentToken
                // Track if this is a path/prefix directive for path literal detection
                if (currentToken == VeldTokenTypes.DIRECTIVE_KEYWORD && (word == "path" || word == "prefix")) {
                    // Will expect a path literal after the upcoming colon
                    expectPathLiteral = false // not yet, wait for colon
                    lastSignificantToken = VeldTokenTypes.DIRECTIVE_KEYWORD
                }
            }

            // Numbers
            char.isDigit() -> {
                while (currentOffset < endOffset &&
                    (buffer[currentOffset].isDigit() || buffer[currentOffset] == '.')) {
                    currentOffset++
                }
                currentToken = VeldTokenTypes.NUMBER
                lastSignificantToken = currentToken
                expectPathLiteral = false
            }

            // Colon
            char == ':' -> {
                currentOffset++
                currentToken = VeldTokenTypes.COLON
                // If the last significant token was a path/prefix directive, now expect a path literal
                if (lastSignificantToken == VeldTokenTypes.DIRECTIVE_KEYWORD) {
                    // Check if the directive before this colon was path or prefix
                    if (isDirectivePathOrPrefix()) {
                        expectPathLiteral = true
                    }
                }
                lastSignificantToken = currentToken
            }

            // Symbols
            char == '{' -> { currentOffset++; currentToken = VeldTokenTypes.LBRACE; lastSignificantToken = currentToken; expectPathLiteral = false }
            char == '}' -> { currentOffset++; currentToken = VeldTokenTypes.RBRACE; lastSignificantToken = currentToken; expectPathLiteral = false }
            char == '<' -> { currentOffset++; currentToken = VeldTokenTypes.LT; lastSignificantToken = currentToken; expectPathLiteral = false }
            char == '>' -> { currentOffset++; currentToken = VeldTokenTypes.GT; lastSignificantToken = currentToken; expectPathLiteral = false }
            char == ',' -> { currentOffset++; currentToken = VeldTokenTypes.COMMA; lastSignificantToken = currentToken; expectPathLiteral = false }
            char == '.' -> { currentOffset++; currentToken = VeldTokenTypes.DOT; lastSignificantToken = currentToken; expectPathLiteral = false }
            char == '?' -> { currentOffset++; currentToken = VeldTokenTypes.QUESTION; lastSignificantToken = currentToken; expectPathLiteral = false }
            char == '(' -> { currentOffset++; currentToken = VeldTokenTypes.LPAREN; lastSignificantToken = currentToken; expectPathLiteral = false }
            char == ')' -> { currentOffset++; currentToken = VeldTokenTypes.RPAREN; lastSignificantToken = currentToken; expectPathLiteral = false }
            char == '[' -> { currentOffset++; currentToken = VeldTokenTypes.LBRACKET; lastSignificantToken = currentToken; expectPathLiteral = false }
            char == ']' -> { currentOffset++; currentToken = VeldTokenTypes.RBRACKET; lastSignificantToken = currentToken; expectPathLiteral = false }
            char == '*' -> { currentOffset++; currentToken = VeldTokenTypes.IDENTIFIER; lastSignificantToken = currentToken; expectPathLiteral = false }

            // Anything else
            else -> {
                currentOffset++
                currentToken = VeldTokenTypes.BAD_CHARACTER
            }
        }
    }

    private fun peek(offset: Int): Char? {
        val idx = currentOffset + offset
        return if (idx < endOffset) buffer[idx] else null
    }

    /**
     * Check if the directive keyword before the current colon was "path" or "prefix".
     * Scans backwards from the colon position.
     */
    private fun isDirectivePathOrPrefix(): Boolean {
        var i = startOffset - 1 // startOffset is the colon position
        // skip whitespace between directive and colon
        while (i >= 0 && buffer[i] == ' ') i--
        // read the word backwards
        val wordEnd = i + 1
        while (i >= 0 && (buffer[i].isLetterOrDigit() || buffer[i] == '_')) i--
        if (i + 1 >= wordEnd) return false
        val word = buffer.subSequence(i + 1, wordEnd).toString()
        return word == "path" || word == "prefix"
    }

    /**
     * Fallback: detect if we are in a context where `/` starts a path literal.
     * Scans backwards through the buffer from the current position.
     */
    private fun isInPathContext(): Boolean {
        var i = startOffset - 1
        // skip whitespace (but not newlines - path must be on same line as directive)
        while (i >= 0 && buffer[i] == ' ') i--
        // we should be at ':'
        if (i < 0 || buffer[i] != ':') return false
        i-- // skip ':'
        // skip whitespace
        while (i >= 0 && buffer[i] == ' ') i--
        // read the word backwards
        val wordEnd = i + 1
        while (i >= 0 && (buffer[i].isLetterOrDigit() || buffer[i] == '_')) i--
        if (i + 1 >= wordEnd) return false
        val word = buffer.subSequence(i + 1, wordEnd).toString()
        return word == "path" || word == "prefix"
    }

    private fun classifyWord(word: String): IElementType {
        return when (word) {
            "model" -> VeldTokenTypes.MODEL_KEYWORD
            "module" -> VeldTokenTypes.MODULE_KEYWORD
            "action" -> VeldTokenTypes.ACTION_KEYWORD
            "enum" -> VeldTokenTypes.ENUM_KEYWORD
            "import" -> VeldTokenTypes.IMPORT_KEYWORD
            "from" -> VeldTokenTypes.IMPORT_KEYWORD
            "extends" -> VeldTokenTypes.EXTENDS_KEYWORD
            "method", "path", "input", "output", "description", "prefix", "default",
            "query", "middleware", "stream", "errors" ->
                VeldTokenTypes.DIRECTIVE_KEYWORD
            "string", "int", "float", "decimal", "bool", "date", "datetime", "uuid", "bytes", "json", "any" ->
                VeldTokenTypes.TYPE_KEYWORD
            "List", "Map" -> VeldTokenTypes.GENERIC_TYPE
            "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS" -> VeldTokenTypes.HTTP_METHOD
            else -> VeldTokenTypes.IDENTIFIER
        }
    }
}
