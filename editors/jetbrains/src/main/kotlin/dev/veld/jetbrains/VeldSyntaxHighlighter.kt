package dev.veld.jetbrains

import com.intellij.lexer.Lexer
import com.intellij.openapi.editor.DefaultLanguageHighlighterColors
import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.fileTypes.SyntaxHighlighterBase
import com.intellij.psi.tree.IElementType
import com.intellij.openapi.editor.colors.TextAttributesKey.createTextAttributesKey

/**
 * Syntax highlighter for Veld language
 */
class VeldSyntaxHighlighter : SyntaxHighlighterBase() {

    override fun getHighlightingLexer(): Lexer = VeldLexer()

    override fun getTokenHighlights(tokenType: IElementType): Array<TextAttributesKey> {
        return when (tokenType) {
            VeldTokenTypes.COMMENT -> arrayOf(COMMENT)
            VeldTokenTypes.STRING -> arrayOf(STRING)
            VeldTokenTypes.NUMBER -> arrayOf(NUMBER)

            VeldTokenTypes.MODEL_KEYWORD,
            VeldTokenTypes.MODULE_KEYWORD,
            VeldTokenTypes.ACTION_KEYWORD,
            VeldTokenTypes.ENUM_KEYWORD,
            VeldTokenTypes.IMPORT_KEYWORD,
            VeldTokenTypes.EXTENDS_KEYWORD -> arrayOf(KEYWORD)

            VeldTokenTypes.DIRECTIVE_KEYWORD -> arrayOf(DIRECTIVE)
            VeldTokenTypes.TYPE_KEYWORD -> arrayOf(TYPE)
            VeldTokenTypes.GENERIC_TYPE -> arrayOf(GENERIC)
            VeldTokenTypes.HTTP_METHOD -> arrayOf(HTTP_METHOD)

            VeldTokenTypes.LBRACE, VeldTokenTypes.RBRACE -> arrayOf(BRACES)
            VeldTokenTypes.LT, VeldTokenTypes.GT -> arrayOf(BRACKETS)
            VeldTokenTypes.COLON -> arrayOf(COLON)
            VeldTokenTypes.COMMA -> arrayOf(COMMA)
            VeldTokenTypes.AT -> arrayOf(AT)

            VeldTokenTypes.IDENTIFIER -> arrayOf(IDENTIFIER)
            VeldTokenTypes.BAD_CHARACTER -> arrayOf(BAD_CHARACTER)

            else -> emptyArray()
        }
    }

    companion object {
        val COMMENT = createTextAttributesKey("VELD_COMMENT", DefaultLanguageHighlighterColors.LINE_COMMENT)
        val STRING = createTextAttributesKey("VELD_STRING", DefaultLanguageHighlighterColors.STRING)
        val NUMBER = createTextAttributesKey("VELD_NUMBER", DefaultLanguageHighlighterColors.NUMBER)
        val KEYWORD = createTextAttributesKey("VELD_KEYWORD", DefaultLanguageHighlighterColors.KEYWORD)
        val DIRECTIVE = createTextAttributesKey("VELD_DIRECTIVE", DefaultLanguageHighlighterColors.INSTANCE_FIELD)
        val TYPE = createTextAttributesKey("VELD_TYPE", DefaultLanguageHighlighterColors.PREDEFINED_SYMBOL)
        val GENERIC = createTextAttributesKey("VELD_GENERIC", DefaultLanguageHighlighterColors.CLASS_NAME)
        val HTTP_METHOD = createTextAttributesKey("VELD_HTTP_METHOD", DefaultLanguageHighlighterColors.CONSTANT)
        val IDENTIFIER = createTextAttributesKey("VELD_IDENTIFIER", DefaultLanguageHighlighterColors.IDENTIFIER)
        val BRACES = createTextAttributesKey("VELD_BRACES", DefaultLanguageHighlighterColors.BRACES)
        val BRACKETS = createTextAttributesKey("VELD_BRACKETS", DefaultLanguageHighlighterColors.BRACKETS)
        val COLON = createTextAttributesKey("VELD_COLON", DefaultLanguageHighlighterColors.OPERATION_SIGN)
        val COMMA = createTextAttributesKey("VELD_COMMA", DefaultLanguageHighlighterColors.COMMA)
        val AT = createTextAttributesKey("VELD_AT", DefaultLanguageHighlighterColors.METADATA)
        val BAD_CHARACTER = createTextAttributesKey("VELD_BAD_CHARACTER", DefaultLanguageHighlighterColors.INVALID_STRING_ESCAPE)
    }
}

