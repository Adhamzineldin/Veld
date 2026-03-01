package dev.veld.jetbrains.psi

import com.intellij.extapi.psi.ASTWrapperPsiElement
import com.intellij.lang.ASTNode
import com.intellij.psi.PsiNameIdentifierOwner
import com.intellij.psi.PsiElement
import dev.veld.jetbrains.VeldTokenTypes

/**
 * PSI element for an enum declaration: `enum Role { admin user guest }`
 */
class VeldEnumDeclaration(node: ASTNode) : ASTWrapperPsiElement(node), PsiNameIdentifierOwner {

    override fun getName(): String? = getNameIdentifier()?.text

    override fun getNameIdentifier(): PsiElement? {
        val enumKw = node.findChildByType(VeldTokenTypes.ENUM_KEYWORD) ?: return null
        var next = enumKw.treeNext
        while (next != null && next.elementType == VeldTokenTypes.WHITE_SPACE) {
            next = next.treeNext
        }
        return if (next?.elementType == VeldTokenTypes.IDENTIFIER) next.psi else null
    }

    override fun setName(name: String): PsiElement = this

    /** Returns enum value names. */
    fun getValues(): List<String> {
        val values = mutableListOf<String>()
        var child = node.findChildByType(VeldTokenTypes.LBRACE)?.treeNext
        while (child != null) {
            if (child.elementType == VeldTokenTypes.RBRACE) break
            if (child.elementType == VeldTokenTypes.IDENTIFIER) {
                values.add(child.text)
            }
            child = child.treeNext
        }
        return values
    }

    override fun toString(): String = "VeldEnumDeclaration(${getName()})"
}
