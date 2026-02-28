package dev.veld.jetbrains

import com.intellij.extapi.psi.PsiFileBase
import com.intellij.openapi.fileTypes.FileType
import com.intellij.psi.FileViewProvider

/**
 * PSI file representation for Veld files
 */
class VeldPsiFile(viewProvider: FileViewProvider) : PsiFileBase(viewProvider, VeldLanguage) {

    override fun getFileType(): FileType = VeldFileType

    override fun toString(): String = "Veld File"
}

