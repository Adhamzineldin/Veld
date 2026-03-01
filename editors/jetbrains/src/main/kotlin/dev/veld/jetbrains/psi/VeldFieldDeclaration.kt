package dev.veld.jetbrains.psi

import com.intellij.extapi.psi.ASTWrapperPsiElement
import com.intellij.lang.ASTNode
import com.intellij.psi.PsiNameIdentifierOwner
import com.intellij.psi.PsiElement
import dev.veld.jetbrains.VeldTokenTypes

/**
 * PSI element for a field declaration: `name: string` or `email?: string`
 */
class VeldFieldDeclaration(node: ASTNode) : ASTWrapperPsiElement(node), PsiNameIdentifierOwner {

    override fun getName(): String? = getNameIdentifier()?.text

    override fun getNameIdentifier(): PsiElement? {
        val first = node.firstChildNode ?: return null
        // Skip whitespace to get the field name identifier
        var current = first
        while (current.elementType == VeldTokenTypes.WHITE_SPACE) {
            current = current.treeNext ?: return null
        }
        return if (current.elementType == VeldTokenTypes.IDENTIFIER) current.psi else null
    }

    override fun setName(name: String): PsiElement = this

    /** Returns the type expression text (everything after the colon). */
    fun getFieldType(): String? {
        val colon = node.findChildByType(VeldTokenTypes.COLON) ?: return null
        val parts = mutableListOf<String>()
        var next = colon.treeNext
        while (next != null) {
            if (next.elementType != VeldTokenTypes.WHITE_SPACE) {
                parts.add(next.text)
            }
            next = next.treeNext
        }
        return if (parts.isNotEmpty()) parts.joinToString("") else null
    }

    /** Returns true if the field is optional (has a ? before the colon). */
    fun isOptional(): Boolean = node.findChildByType(VeldTokenTypes.QUESTION) != null

    override fun toString(): String = "VeldFieldDeclaration(${getName()})"
}
