package dev.veld.jetbrains

import com.intellij.psi.tree.IElementType

/**
 * Composite element types for the Veld PSI tree.
 * These represent parsed constructs (as opposed to token types which are lexer-level).
 */
object VeldElementTypes {
    @JvmField val IMPORT_STATEMENT = IElementType("IMPORT_STATEMENT", VeldLanguage)
    @JvmField val MODEL_DECLARATION = IElementType("MODEL_DECLARATION", VeldLanguage)
    @JvmField val ENUM_DECLARATION = IElementType("ENUM_DECLARATION", VeldLanguage)
    @JvmField val CONSTANTS_DECLARATION = IElementType("CONSTANTS_DECLARATION", VeldLanguage)
    @JvmField val MODULE_DECLARATION = IElementType("MODULE_DECLARATION", VeldLanguage)
    @JvmField val ACTION_DECLARATION = IElementType("ACTION_DECLARATION", VeldLanguage)
    @JvmField val FIELD_DECLARATION = IElementType("FIELD_DECLARATION", VeldLanguage)
    @JvmField val DIRECTIVE = IElementType("DIRECTIVE_ELEMENT", VeldLanguage)
    @JvmField val ENUM_BODY = IElementType("ENUM_BODY", VeldLanguage)
}
