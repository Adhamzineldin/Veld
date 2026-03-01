package dev.veld.jetbrains.psi

import com.intellij.extapi.psi.ASTWrapperPsiElement
import com.intellij.lang.ASTNode
import dev.veld.jetbrains.VeldTokenTypes

/**
 * PSI element for an import statement: `import @models/user`
 */
class VeldImportStatement(node: ASTNode) : ASTWrapperPsiElement(node) {

    /** Returns the import path text (e.g. "@models/user"), or null. */
    fun getImportPath(): String? {
        val pathNode = node.findChildByType(VeldTokenTypes.IMPORT_PATH)
            ?: node.findChildByType(VeldTokenTypes.STRING)
        return pathNode?.text?.removeSurrounding("\"")
    }

    override fun toString(): String = "VeldImportStatement(${getImportPath()})"
}
