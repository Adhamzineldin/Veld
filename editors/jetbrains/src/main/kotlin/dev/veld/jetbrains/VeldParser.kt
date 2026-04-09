package dev.veld.jetbrains

import com.intellij.lang.ASTNode
import com.intellij.lang.PsiBuilder
import com.intellij.lang.PsiParser
import com.intellij.psi.tree.IElementType

/**
 * Recursive descent parser for Veld language.
 * Builds a proper PSI tree with MODEL_DECLARATION, ENUM_DECLARATION,
 * MODULE_DECLARATION, ACTION_DECLARATION, FIELD_DECLARATION, and IMPORT_STATEMENT nodes.
 */
class VeldParser : PsiParser {

    override fun parse(root: IElementType, builder: PsiBuilder): ASTNode {
        val rootMarker = builder.mark()
        parseFile(builder)
        rootMarker.done(root)
        return builder.treeBuilt
    }

    private fun parseFile(b: PsiBuilder) {
        while (!b.eof()) {
            skipWhitespaceAndComments(b)
            if (b.eof()) break

            when (b.tokenType) {
                VeldTokenTypes.IMPORT_KEYWORD -> parseImport(b)
                VeldTokenTypes.MODEL_KEYWORD -> parseModel(b)
                VeldTokenTypes.ENUM_KEYWORD -> parseEnum(b)
                VeldTokenTypes.CONSTANTS_KEYWORD -> parseConstants(b)
                VeldTokenTypes.MODULE_KEYWORD -> parseModule(b)
                VeldTokenTypes.COMMENT -> b.advanceLexer()
                else -> {
                    // Error recovery: skip unexpected token
                    b.advanceLexer()
                }
            }
        }
    }

    private fun parseImport(b: PsiBuilder) {
        val marker = b.mark()
        b.advanceLexer() // consume IMPORT_KEYWORD
        skipWhitespace(b)

        // Consume import path or string
        if (b.tokenType == VeldTokenTypes.IMPORT_PATH || b.tokenType == VeldTokenTypes.STRING) {
            b.advanceLexer()
        }

        marker.done(VeldElementTypes.IMPORT_STATEMENT)
    }

    private fun parseModel(b: PsiBuilder) {
        val marker = b.mark()
        b.advanceLexer() // consume MODEL_KEYWORD
        skipWhitespace(b)

        // Model name
        if (b.tokenType == VeldTokenTypes.IDENTIFIER) {
            b.advanceLexer()
        }
        skipWhitespace(b)

        // Optional: extends ParentName
        if (b.tokenType == VeldTokenTypes.EXTENDS_KEYWORD) {
            b.advanceLexer() // consume extends
            skipWhitespace(b)
            if (b.tokenType == VeldTokenTypes.IDENTIFIER) {
                b.advanceLexer() // consume parent name
            }
            skipWhitespace(b)
        }

        // Expect { ... }
        if (b.tokenType == VeldTokenTypes.LBRACE) {
            b.advanceLexer() // consume {
            parseModelBody(b)
            if (b.tokenType == VeldTokenTypes.RBRACE) {
                b.advanceLexer() // consume }
            }
        }

        marker.done(VeldElementTypes.MODEL_DECLARATION)
    }

    private fun parseModelBody(b: PsiBuilder) {
        while (!b.eof() && b.tokenType != VeldTokenTypes.RBRACE) {
            skipWhitespaceAndComments(b)
            if (b.eof() || b.tokenType == VeldTokenTypes.RBRACE) break

            when (b.tokenType) {
                VeldTokenTypes.DIRECTIVE_KEYWORD -> parseDirective(b)
                VeldTokenTypes.IDENTIFIER -> parseField(b)
                else -> b.advanceLexer() // error recovery
            }
        }
    }

    private fun parseField(b: PsiBuilder) {
        val marker = b.mark()

        // Field name (IDENTIFIER)
        b.advanceLexer()
        skipWhitespace(b)

        // Optional ?
        if (b.tokenType == VeldTokenTypes.QUESTION) {
            b.advanceLexer()
            skipWhitespace(b)
        }

        // Colon
        if (b.tokenType == VeldTokenTypes.COLON) {
            b.advanceLexer()
            skipWhitespace(b)
        }

        // Consume type expression tokens until end of field
        consumeTypeExpression(b)

        marker.done(VeldElementTypes.FIELD_DECLARATION)
    }

    private fun consumeTypeExpression(b: PsiBuilder) {
        // Consume tokens that form a type expression, including:
        // IDENTIFIER, TYPE_KEYWORD, GENERIC_TYPE, LT, GT, COMMA, LBRACKET, RBRACKET, AT, LPAREN, RPAREN, STRING, NUMBER, IMPORT_PATH
        // Stop at: newline (WHITE_SPACE containing \n), RBRACE, LBRACE, or next field/directive
        var angleBracketDepth = 0
        var parenDepth = 0

        while (!b.eof()) {
            val tt = b.tokenType

            // Stop conditions
            if (tt == VeldTokenTypes.RBRACE || tt == VeldTokenTypes.LBRACE) break

            // White space with newline signals end of field (unless inside angle brackets)
            if (tt == VeldTokenTypes.WHITE_SPACE) {
                val text = b.tokenText ?: ""
                if (text.contains('\n') && angleBracketDepth == 0 && parenDepth == 0) break
                b.advanceLexer()
                continue
            }

            if (tt == VeldTokenTypes.COMMENT) break

            // Track nesting
            when (tt) {
                VeldTokenTypes.LT -> angleBracketDepth++
                VeldTokenTypes.GT -> {
                    angleBracketDepth--
                    if (angleBracketDepth < 0) {
                        angleBracketDepth = 0
                        break
                    }
                }
                VeldTokenTypes.LPAREN -> parenDepth++
                VeldTokenTypes.RPAREN -> {
                    parenDepth--
                    if (parenDepth < 0) {
                        parenDepth = 0
                        break
                    }
                }
                else -> {}
            }

            b.advanceLexer()
        }
    }

    private fun parseEnum(b: PsiBuilder) {
        val marker = b.mark()
        b.advanceLexer() // consume ENUM_KEYWORD
        skipWhitespace(b)

        // Enum name
        if (b.tokenType == VeldTokenTypes.IDENTIFIER) {
            b.advanceLexer()
        }
        skipWhitespace(b)

        // Expect { values }
        if (b.tokenType == VeldTokenTypes.LBRACE) {
            b.advanceLexer() // consume {
            // Consume enum values (identifiers inside braces)
            while (!b.eof() && b.tokenType != VeldTokenTypes.RBRACE) {
                if (b.tokenType == VeldTokenTypes.WHITE_SPACE || b.tokenType == VeldTokenTypes.COMMENT) {
                    b.advanceLexer()
                } else if (b.tokenType == VeldTokenTypes.IDENTIFIER) {
                    b.advanceLexer()
                } else {
                    b.advanceLexer() // error recovery
                }
            }
            if (b.tokenType == VeldTokenTypes.RBRACE) {
                b.advanceLexer()
            }
        }

        marker.done(VeldElementTypes.ENUM_DECLARATION)
    }

    private fun parseConstants(b: PsiBuilder) {
        val marker = b.mark()
        b.advanceLexer() // consume CONSTANTS_KEYWORD
        skipWhitespace(b)

        // Constants group name
        if (b.tokenType == VeldTokenTypes.IDENTIFIER) {
            b.advanceLexer()
        }
        skipWhitespace(b)

        // Expect { fields }
        if (b.tokenType == VeldTokenTypes.LBRACE) {
            b.advanceLexer() // consume {
            // Consume constant fields: NAME: type = value
            while (!b.eof() && b.tokenType != VeldTokenTypes.RBRACE) {
                if (b.tokenType == VeldTokenTypes.WHITE_SPACE || b.tokenType == VeldTokenTypes.COMMENT) {
                    b.advanceLexer()
                } else if (b.tokenType == VeldTokenTypes.DIRECTIVE_KEYWORD) {
                    // description: "..." inside constants
                    parseDirective(b)
                } else {
                    // Consume the entire constant field line (NAME: type = value)
                    val fieldMarker = b.mark()
                    // Consume tokens until newline or closing brace
                    while (!b.eof()) {
                        val tt = b.tokenType
                        if (tt == VeldTokenTypes.RBRACE || tt == VeldTokenTypes.LBRACE) break
                        if (tt == VeldTokenTypes.WHITE_SPACE) {
                            val text = b.tokenText ?: ""
                            if (text.contains('\n')) break
                            b.advanceLexer()
                            continue
                        }
                        if (tt == VeldTokenTypes.COMMENT) break
                        b.advanceLexer()
                    }
                    fieldMarker.done(VeldElementTypes.FIELD_DECLARATION)
                }
            }
            if (b.tokenType == VeldTokenTypes.RBRACE) {
                b.advanceLexer()
            }
        }

        marker.done(VeldElementTypes.CONSTANTS_DECLARATION)
    }

    private fun parseModule(b: PsiBuilder) {
        val marker = b.mark()
        b.advanceLexer() // consume MODULE_KEYWORD
        skipWhitespace(b)

        // Module name
        if (b.tokenType == VeldTokenTypes.IDENTIFIER) {
            b.advanceLexer()
        }
        skipWhitespace(b)

        // Expect { directives and actions }
        if (b.tokenType == VeldTokenTypes.LBRACE) {
            b.advanceLexer() // consume {
            parseModuleBody(b)
            if (b.tokenType == VeldTokenTypes.RBRACE) {
                b.advanceLexer()
            }
        }

        marker.done(VeldElementTypes.MODULE_DECLARATION)
    }

    private fun parseModuleBody(b: PsiBuilder) {
        while (!b.eof() && b.tokenType != VeldTokenTypes.RBRACE) {
            skipWhitespaceAndComments(b)
            if (b.eof() || b.tokenType == VeldTokenTypes.RBRACE) break

            when (b.tokenType) {
                VeldTokenTypes.ACTION_KEYWORD -> parseAction(b)
                VeldTokenTypes.DIRECTIVE_KEYWORD -> parseDirective(b)
                else -> b.advanceLexer() // error recovery
            }
        }
    }

    private fun parseAction(b: PsiBuilder) {
        val marker = b.mark()
        b.advanceLexer() // consume ACTION_KEYWORD
        skipWhitespace(b)

        // Action name
        if (b.tokenType == VeldTokenTypes.IDENTIFIER) {
            b.advanceLexer()
        }
        skipWhitespace(b)

        // Expect { directives }
        if (b.tokenType == VeldTokenTypes.LBRACE) {
            b.advanceLexer() // consume {
            parseActionBody(b)
            if (b.tokenType == VeldTokenTypes.RBRACE) {
                b.advanceLexer()
            }
        }

        marker.done(VeldElementTypes.ACTION_DECLARATION)
    }

    private fun parseActionBody(b: PsiBuilder) {
        while (!b.eof() && b.tokenType != VeldTokenTypes.RBRACE) {
            skipWhitespaceAndComments(b)
            if (b.eof() || b.tokenType == VeldTokenTypes.RBRACE) break

            when (b.tokenType) {
                VeldTokenTypes.DIRECTIVE_KEYWORD -> parseDirective(b)
                else -> b.advanceLexer() // error recovery
            }
        }
    }

    private fun parseDirective(b: PsiBuilder) {
        val marker = b.mark()
        b.advanceLexer() // consume DIRECTIVE_KEYWORD
        skipWhitespace(b)

        // Colon
        if (b.tokenType == VeldTokenTypes.COLON) {
            b.advanceLexer()
            skipWhitespace(b)
        }

        // Consume directive value tokens until newline or closing brace
        while (!b.eof()) {
            val tt = b.tokenType
            if (tt == VeldTokenTypes.RBRACE || tt == VeldTokenTypes.LBRACE) break
            if (tt == VeldTokenTypes.WHITE_SPACE) {
                val text = b.tokenText ?: ""
                if (text.contains('\n')) break
                b.advanceLexer()
                continue
            }
            if (tt == VeldTokenTypes.COMMENT) break
            b.advanceLexer()
        }

        marker.done(VeldElementTypes.DIRECTIVE)
    }

    private fun skipWhitespace(b: PsiBuilder) {
        while (!b.eof() && b.tokenType == VeldTokenTypes.WHITE_SPACE) {
            b.advanceLexer()
        }
    }

    private fun skipWhitespaceAndComments(b: PsiBuilder) {
        while (!b.eof() && (b.tokenType == VeldTokenTypes.WHITE_SPACE || b.tokenType == VeldTokenTypes.COMMENT)) {
            b.advanceLexer()
        }
    }
}
