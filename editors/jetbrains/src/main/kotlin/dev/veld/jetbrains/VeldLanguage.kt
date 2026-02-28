package dev.veld.jetbrains

import com.intellij.lang.Language

/**
 * Veld language definition
 */
object VeldLanguage : Language("Veld") {
    override fun getDisplayName(): String = "Veld"

    override fun isCaseSensitive(): Boolean = true
}

