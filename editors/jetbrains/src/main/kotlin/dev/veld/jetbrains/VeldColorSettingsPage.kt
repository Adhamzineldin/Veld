package dev.veld.jetbrains

import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.fileTypes.SyntaxHighlighter
import com.intellij.openapi.options.colors.AttributesDescriptor
import com.intellij.openapi.options.colors.ColorDescriptor
import com.intellij.openapi.options.colors.ColorSettingsPage
import javax.swing.Icon

/**
 * Color settings page for Veld language.
 * Accessible via Settings → Editor → Color Scheme → Veld.
 */
class VeldColorSettingsPage : ColorSettingsPage {

    override fun getAttributeDescriptors(): Array<AttributesDescriptor> = DESCRIPTORS

    override fun getColorDescriptors(): Array<ColorDescriptor> = ColorDescriptor.EMPTY_ARRAY

    override fun getDisplayName(): String = "Veld"

    override fun getIcon(): Icon? = null

    override fun getHighlighter(): SyntaxHighlighter = VeldSyntaxHighlighter()

    override fun getDemoText(): String = """
        // Veld contract — color settings preview
        import <importPath>@models/auth</importPath>

        <keyword>model</keyword> <modelDecl>CreateUserRequest</modelDecl> {
            <field>email</field>:    <type>string</type>
            <field>name</field>:     <type>string</type>
            <field>age</field>?:     <type>int</type>
            <field>tags</field>:     <type>string</type>[]
            <field>role</field>:     <modelRef>Role</modelRef> <annotation>@default(user)</annotation>
            <field>metadata</field>: Map<<type>string</type>, <type>string</type>>
        }

        <keyword>model</keyword> <modelDecl>User</modelDecl> <keyword>extends</keyword> <modelRef>CreateUserRequest</modelRef> {
            <field>id</field>:        <type>uuid</type>
            <field>createdAt</field>: <type>datetime</type>
        }

        <keyword>enum</keyword> <enumDecl>Role</enumDecl> {
            <enumValue>admin</enumValue>
            <enumValue>user</enumValue>
            <enumValue>guest</enumValue>
        }

        <keyword>module</keyword> <moduleDecl>Users</moduleDecl> {
            <directive>description</directive>: "User management API"
            <directive>prefix</directive>: <path>/api/v1/users</path>

            <keyword>action</keyword> <actionName>ListUsers</actionName> {
                <directive>method</directive>: <httpMethod>GET</httpMethod>
                <directive>path</directive>:   <path>/</path>
                <directive>output</directive>: List<<modelRef>User</modelRef>>
            }

            <keyword>action</keyword> <actionName>GetUser</actionName> {
                <directive>method</directive>: <httpMethod>GET</httpMethod>
                <directive>path</directive>:   <path>/</path><pathParam>:id</pathParam>
                <directive>output</directive>: <modelRef>User</modelRef>
            }

            <keyword>action</keyword> <actionName>CreateUser</actionName> {
                <directive>method</directive>:    <httpMethod>POST</httpMethod>
                <directive>path</directive>:      <path>/</path>
                <directive>input</directive>:     <modelRef>CreateUserRequest</modelRef>
                <directive>output</directive>:    <modelRef>User</modelRef>
                <directive>middleware</directive>: <modelRef>AuthGuard</modelRef>
            }

            <keyword>action</keyword> <actionName>DeleteUser</actionName> {
                <directive>method</directive>: <httpMethod>DELETE</httpMethod>
                <directive>path</directive>:   <path>/</path><pathParam>:id</pathParam>
            }
        }
    """.trimIndent()

    override fun getAdditionalHighlightingTagToDescriptorMap(): Map<String, TextAttributesKey> = mapOf(
        // Lexer-based
        "keyword"    to VeldSyntaxHighlighter.KEYWORD,
        "directive"  to VeldSyntaxHighlighter.DIRECTIVE,
        "type"       to VeldSyntaxHighlighter.TYPE,
        "generic"    to VeldSyntaxHighlighter.GENERIC,
        "httpMethod" to VeldSyntaxHighlighter.HTTP_METHOD,
        "importPath" to VeldSyntaxHighlighter.IMPORT_PATH,
        "path"       to VeldSyntaxHighlighter.PATH,
        // Annotator-based semantic highlights
        "modelDecl"  to VeldAnnotator.MODEL_DECLARATION,
        "enumDecl"   to VeldAnnotator.ENUM_DECLARATION,
        "moduleDecl" to VeldAnnotator.MODULE_DECLARATION,
        "actionName" to VeldAnnotator.ACTION_NAME,
        "field"      to VeldAnnotator.FIELD_NAME,
        "enumValue"  to VeldAnnotator.ENUM_VALUE,
        "modelRef"   to VeldAnnotator.MODEL_REFERENCE,
        "enumRef"    to VeldAnnotator.ENUM_REFERENCE,
        "annotation" to VeldAnnotator.ANNOTATION,
        "pathParam"  to VeldAnnotator.PATH_PARAM,
    )

    companion object {
        private val DESCRIPTORS = arrayOf(
            // ── Lexer highlights ─────────────────────────────────────────────
            AttributesDescriptor("Comment",           VeldSyntaxHighlighter.COMMENT),
            AttributesDescriptor("String",            VeldSyntaxHighlighter.STRING),
            AttributesDescriptor("Number",            VeldSyntaxHighlighter.NUMBER),
            AttributesDescriptor("Keyword",           VeldSyntaxHighlighter.KEYWORD),
            AttributesDescriptor("Directive",         VeldSyntaxHighlighter.DIRECTIVE),
            AttributesDescriptor("Built-in type",     VeldSyntaxHighlighter.TYPE),
            AttributesDescriptor("Generic type",      VeldSyntaxHighlighter.GENERIC),
            AttributesDescriptor("HTTP method",       VeldSyntaxHighlighter.HTTP_METHOD),
            AttributesDescriptor("Identifier",        VeldSyntaxHighlighter.IDENTIFIER),
            AttributesDescriptor("Braces",            VeldSyntaxHighlighter.BRACES),
            AttributesDescriptor("Brackets",          VeldSyntaxHighlighter.BRACKETS),
            AttributesDescriptor("Colon",             VeldSyntaxHighlighter.COLON),
            AttributesDescriptor("Import path",       VeldSyntaxHighlighter.IMPORT_PATH),
            AttributesDescriptor("URL path",          VeldSyntaxHighlighter.PATH),
            // ── Semantic highlights (annotator) ──────────────────────────────
            AttributesDescriptor("Model declaration", VeldAnnotator.MODEL_DECLARATION),
            AttributesDescriptor("Enum declaration",  VeldAnnotator.ENUM_DECLARATION),
            AttributesDescriptor("Module declaration",VeldAnnotator.MODULE_DECLARATION),
            AttributesDescriptor("Action name",       VeldAnnotator.ACTION_NAME),
            AttributesDescriptor("Field name",        VeldAnnotator.FIELD_NAME),
            AttributesDescriptor("Enum value",        VeldAnnotator.ENUM_VALUE),
            AttributesDescriptor("Model reference",   VeldAnnotator.MODEL_REFERENCE),
            AttributesDescriptor("Enum reference",    VeldAnnotator.ENUM_REFERENCE),
            AttributesDescriptor("Module reference",  VeldAnnotator.MODULE_REFERENCE),
            AttributesDescriptor("Annotation",        VeldAnnotator.ANNOTATION),
            AttributesDescriptor("Path parameter",    VeldAnnotator.PATH_PARAM),
        )
    }
}
