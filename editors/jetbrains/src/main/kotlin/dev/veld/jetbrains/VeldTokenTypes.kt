package dev.veld.jetbrains

import com.intellij.psi.tree.IElementType
import com.intellij.psi.TokenType

/**
 * Token types for Veld language
 */
object VeldTokenTypes {
    @JvmField val WHITE_SPACE = TokenType.WHITE_SPACE
    @JvmField val BAD_CHARACTER = TokenType.BAD_CHARACTER

    @JvmField val COMMENT = IElementType("COMMENT", VeldLanguage)
    @JvmField val STRING = IElementType("STRING", VeldLanguage)
    @JvmField val NUMBER = IElementType("NUMBER", VeldLanguage)
    @JvmField val IDENTIFIER = IElementType("IDENTIFIER", VeldLanguage)

    // Keywords
    @JvmField val MODEL_KEYWORD = IElementType("MODEL", VeldLanguage)
    @JvmField val MODULE_KEYWORD = IElementType("MODULE", VeldLanguage)
    @JvmField val ACTION_KEYWORD = IElementType("ACTION", VeldLanguage)
    @JvmField val ENUM_KEYWORD = IElementType("ENUM", VeldLanguage)
    @JvmField val IMPORT_KEYWORD = IElementType("IMPORT", VeldLanguage)
    @JvmField val EXTENDS_KEYWORD = IElementType("EXTENDS", VeldLanguage)
    @JvmField val DIRECTIVE_KEYWORD = IElementType("DIRECTIVE", VeldLanguage)
    @JvmField val TYPE_KEYWORD = IElementType("TYPE", VeldLanguage)
    @JvmField val GENERIC_TYPE = IElementType("GENERIC_TYPE", VeldLanguage)
    @JvmField val HTTP_METHOD = IElementType("HTTP_METHOD", VeldLanguage)

    // Symbols
    @JvmField val LBRACE = IElementType("LBRACE", VeldLanguage)
    @JvmField val RBRACE = IElementType("RBRACE", VeldLanguage)
    @JvmField val LT = IElementType("LT", VeldLanguage)
    @JvmField val GT = IElementType("GT", VeldLanguage)
    @JvmField val COLON = IElementType("COLON", VeldLanguage)
    @JvmField val COMMA = IElementType("COMMA", VeldLanguage)
    @JvmField val AT = IElementType("AT", VeldLanguage)
}

