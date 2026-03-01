package dev.veld.jetbrains.psi

import com.intellij.extapi.psi.ASTWrapperPsiElement
import com.intellij.lang.ASTNode
import com.intellij.psi.PsiNameIdentifierOwner
import com.intellij.psi.PsiElement
import com.intellij.psi.util.PsiTreeUtil
import dev.veld.jetbrains.VeldTokenTypes

/**
 * PSI element for a module declaration: `module Users { ... }`
 */
class VeldModuleDeclaration(node: ASTNode) : ASTWrapperPsiElement(node), PsiNameIdentifierOwner {

    override fun getName(): String? = getNameIdentifier()?.text

    override fun getNameIdentifier(): PsiElement? {
        val moduleKw = node.findChildByType(VeldTokenTypes.MODULE_KEYWORD) ?: return null
        var next = moduleKw.treeNext
        while (next != null && next.elementType == VeldTokenTypes.WHITE_SPACE) {
            next = next.treeNext
        }
        return if (next?.elementType == VeldTokenTypes.IDENTIFIER) next.psi else null
    }

    override fun setName(name: String): PsiElement = this

    /** Returns all action declarations inside this module. */
    fun getActions(): Array<VeldActionDeclaration> =
        PsiTreeUtil.getChildrenOfType(this, VeldActionDeclaration::class.java) ?: emptyArray()

    override fun toString(): String = "VeldModuleDeclaration(${getName()})"
}
