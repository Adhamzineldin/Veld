package dev.veld.jetbrains.psi

import com.intellij.extapi.psi.ASTWrapperPsiElement
import com.intellij.lang.ASTNode
import com.intellij.psi.PsiNameIdentifierOwner
import com.intellij.psi.PsiElement
import com.intellij.psi.util.PsiTreeUtil
import dev.veld.jetbrains.VeldElementTypes
import dev.veld.jetbrains.VeldTokenTypes

/**
 * PSI element for a model declaration: `model User extends Base { ... }`
 */
class VeldModelDeclaration(node: ASTNode) : ASTWrapperPsiElement(node), PsiNameIdentifierOwner {

    override fun getName(): String? = getNameIdentifier()?.text

    override fun getNameIdentifier(): PsiElement? {
        val modelKw = node.findChildByType(VeldTokenTypes.MODEL_KEYWORD) ?: return null
        // The identifier immediately after MODEL_KEYWORD
        var next = modelKw.treeNext
        while (next != null && next.elementType == VeldTokenTypes.WHITE_SPACE) {
            next = next.treeNext
        }
        return if (next?.elementType == VeldTokenTypes.IDENTIFIER) next.psi else null
    }

    override fun setName(name: String): PsiElement = this

    /** Returns the name of the parent type (from `extends`), or null. */
    fun getExtendsName(): String? {
        val extendsKw = node.findChildByType(VeldTokenTypes.EXTENDS_KEYWORD) ?: return null
        var next = extendsKw.treeNext
        while (next != null && next.elementType == VeldTokenTypes.WHITE_SPACE) {
            next = next.treeNext
        }
        return if (next?.elementType == VeldTokenTypes.IDENTIFIER) next.text else null
    }

    /** Returns all field declarations inside this model. */
    fun getFields(): Array<VeldFieldDeclaration> =
        PsiTreeUtil.getChildrenOfType(this, VeldFieldDeclaration::class.java) ?: emptyArray()

    override fun toString(): String = "VeldModelDeclaration(${getName()})"
}
