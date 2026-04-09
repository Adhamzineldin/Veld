package dev.veld.jetbrains.psi

import com.intellij.extapi.psi.ASTWrapperPsiElement
import com.intellij.lang.ASTNode
import com.intellij.psi.PsiNameIdentifierOwner
import com.intellij.psi.PsiElement
import dev.veld.jetbrains.VeldTokenTypes

/**
 * PSI element for a constants declaration: `constants GroupName { NAME: type = value ... }`
 */
class VeldConstantsDeclaration(node: ASTNode) : ASTWrapperPsiElement(node), PsiNameIdentifierOwner {

    override fun getName(): String? = getNameIdentifier()?.text

    override fun getNameIdentifier(): PsiElement? {
        val kw = node.findChildByType(VeldTokenTypes.CONSTANTS_KEYWORD) ?: return null
        var next = kw.treeNext
        while (next != null && next.elementType == VeldTokenTypes.WHITE_SPACE) {
            next = next.treeNext
        }
        return if (next?.elementType == VeldTokenTypes.IDENTIFIER) next.psi else null
    }

    override fun setName(name: String): PsiElement = this

    /** Returns constant field names. */
    fun getFieldNames(): List<String> {
        val names = mutableListOf<String>()
        var child = node.findChildByType(VeldTokenTypes.LBRACE)?.treeNext
        while (child != null) {
            if (child.elementType == VeldTokenTypes.RBRACE) break
            if (child.elementType == VeldTokenTypes.IDENTIFIER) {
                names.add(child.text)
            }
            child = child.treeNext
        }
        return names
    }

    override fun toString(): String = "VeldConstantsDeclaration(${getName()})"
}

