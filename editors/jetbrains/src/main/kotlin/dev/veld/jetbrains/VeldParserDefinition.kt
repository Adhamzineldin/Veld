package dev.veld.jetbrains

import com.intellij.lang.ASTNode
import com.intellij.lang.ParserDefinition
import com.intellij.lang.PsiParser
import com.intellij.lexer.Lexer
import com.intellij.openapi.project.Project
import com.intellij.psi.FileViewProvider
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFile
import com.intellij.psi.tree.IFileElementType
import com.intellij.psi.tree.TokenSet

/**
 * Parser definition for Veld language
 */
class VeldParserDefinition : ParserDefinition {

    override fun createLexer(project: Project?): Lexer = VeldLexer()

    override fun createParser(project: Project?): PsiParser = VeldParser()

    override fun getFileNodeType(): IFileElementType = FILE

    override fun getCommentTokens(): TokenSet = COMMENTS

    override fun getStringLiteralElements(): TokenSet = STRINGS

    override fun createElement(node: ASTNode): PsiElement = VeldPsiElement(node)

    override fun createFile(viewProvider: FileViewProvider): PsiFile =
        VeldPsiFile(viewProvider)

    companion object {
        val FILE = IFileElementType(VeldLanguage)
        val COMMENTS = TokenSet.create(VeldTokenTypes.COMMENT)
        val STRINGS = TokenSet.create(VeldTokenTypes.STRING)
    }
}

