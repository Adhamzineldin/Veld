package dev.veld.jetbrains

import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.fileTypes.SyntaxHighlighter
import com.intellij.openapi.options.colors.AttributesDescriptor
import com.intellij.openapi.options.colors.ColorDescriptor
import com.intellij.openapi.options.colors.ColorSettingsPage
import javax.swing.Icon

/**
 * Color settings page for Veld language
 */
class VeldColorSettingsPage : ColorSettingsPage {

    override fun getAttributeDescriptors(): Array<AttributesDescriptor> = DESCRIPTORS

    override fun getColorDescriptors(): Array<ColorDescriptor> = ColorDescriptor.EMPTY_ARRAY

    override fun getDisplayName(): String = "Veld"

    override fun getIcon(): Icon? = null

    override fun getHighlighter(): SyntaxHighlighter = VeldSyntaxHighlighter()

    override fun getDemoText(): String = """
        // Veld contract example
        import "./models/common.veld"

        model User {
            id: int
            email: string
            name: string
            createdAt: datetime
        }

        enum Role {
            admin user guest
        }

        module users {
            description: "User management API"
            prefix: /api/users

            action ListUsers {
                method: GET
                path: /
                output: List<User>
            }

            action GetUser {
                method: GET
                path: /:id
                output: User
            }

            action CreateUser {
                method: POST
                path: /
                input: User
                output: User
            }
        }
    """.trimIndent()

    override fun getAdditionalHighlightingTagToDescriptorMap(): Map<String, TextAttributesKey>? = null

    companion object {
        private val DESCRIPTORS = arrayOf(
            AttributesDescriptor("Comment", VeldSyntaxHighlighter.COMMENT),
            AttributesDescriptor("String", VeldSyntaxHighlighter.STRING),
            AttributesDescriptor("Number", VeldSyntaxHighlighter.NUMBER),
            AttributesDescriptor("Keyword", VeldSyntaxHighlighter.KEYWORD),
            AttributesDescriptor("Directive", VeldSyntaxHighlighter.DIRECTIVE),
            AttributesDescriptor("Type", VeldSyntaxHighlighter.TYPE),
            AttributesDescriptor("Generic Type", VeldSyntaxHighlighter.GENERIC),
            AttributesDescriptor("HTTP Method", VeldSyntaxHighlighter.HTTP_METHOD),
            AttributesDescriptor("Identifier", VeldSyntaxHighlighter.IDENTIFIER),
            AttributesDescriptor("Braces", VeldSyntaxHighlighter.BRACES),
            AttributesDescriptor("Brackets", VeldSyntaxHighlighter.BRACKETS),
        )
    }
}

