package dev.veld.jetbrains

import com.intellij.openapi.fileTypes.LanguageFileType
import javax.swing.Icon

/** `.veld` contract files. Highlighting comes from the bundled TextMate grammar. */
object VeldFileType : LanguageFileType(VeldLanguage) {
    override fun getName(): String = "Veld"
    override fun getDescription(): String = "Veld contract"
    override fun getDefaultExtension(): String = "veld"
    override fun getIcon(): Icon = VeldIcons.FILE
}
