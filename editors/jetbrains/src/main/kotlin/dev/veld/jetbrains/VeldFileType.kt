package dev.veld.jetbrains

import com.intellij.openapi.fileTypes.LanguageFileType
import com.intellij.openapi.util.IconLoader
import javax.swing.Icon

/**
 * Veld file type definition (.veld files)
 */
object VeldFileType : LanguageFileType(VeldLanguage) {

    override fun getName(): String = "Veld"

    override fun getDescription(): String = "Veld Contract File"

    override fun getDefaultExtension(): String = "veld"

    override fun getIcon(): Icon = IconLoader.getIcon("/icons/veld_16.png", VeldFileType::class.java)

    const val FILE_EXTENSION = "veld"
}

