package dev.veld.jetbrains.psi

import com.intellij.extapi.psi.ASTWrapperPsiElement
import com.intellij.lang.ASTNode
import com.intellij.psi.PsiNameIdentifierOwner
import com.intellij.psi.PsiElement
import dev.veld.jetbrains.VeldTokenTypes

/**
 * PSI element for an action declaration: `action CreateUser { ... }`
 */
class VeldActionDeclaration(node: ASTNode) : ASTWrapperPsiElement(node), PsiNameIdentifierOwner {

    override fun getName(): String? = getNameIdentifier()?.text

    override fun getNameIdentifier(): PsiElement? {
        val actionKw = node.findChildByType(VeldTokenTypes.ACTION_KEYWORD) ?: return null
        var next = actionKw.treeNext
        while (next != null && next.elementType == VeldTokenTypes.WHITE_SPACE) {
            next = next.treeNext
        }
        return if (next?.elementType == VeldTokenTypes.IDENTIFIER) next.psi else null
    }

    override fun setName(name: String): PsiElement = this

    /** Returns the HTTP method (GET, POST, etc.) from a method directive. */
    fun getMethod(): String? = getDirectiveValue("method")

    /** Returns the path value from a path directive. */
    fun getPath(): String? = getDirectiveValue("path")

    /** Returns the input type from an input directive. */
    fun getInput(): String? = getDirectiveValue("input")

    /** Returns the output type from an output directive. */
    fun getOutput(): String? = getDirectiveValue("output")

    private fun getDirectiveValue(name: String): String? {
        var child = node.firstChildNode
        while (child != null) {
            if (child.elementType == VeldTokenTypes.DIRECTIVE_KEYWORD && child.text == name) {
                // Skip whitespace and colon to get the value
                var next = child.treeNext
                while (next != null && (next.elementType == VeldTokenTypes.WHITE_SPACE || next.elementType == VeldTokenTypes.COLON)) {
                    next = next.treeNext
                }
                if (next != null && next.elementType != VeldTokenTypes.WHITE_SPACE) {
                    return next.text
                }
            }
            child = child.treeNext
        }
        return null
    }

    override fun toString(): String = "VeldActionDeclaration(${getName()})"
}
