package dev.veld.jetbrains

import com.intellij.application.options.CodeStyleAbstractConfigurable
import com.intellij.application.options.CodeStyleAbstractPanel
import com.intellij.application.options.TabbedLanguageCodeStylePanel
import com.intellij.psi.codeStyle.CodeStyleConfigurable
import com.intellij.psi.codeStyle.CodeStyleSettings
import com.intellij.psi.codeStyle.CodeStyleSettingsProvider
import com.intellij.psi.codeStyle.CustomCodeStyleSettings

/**
 * Code style settings provider for Veld
 */
class VeldCodeStyleSettingsProvider : CodeStyleSettingsProvider() {
    override fun createConfigurable(
        settings: CodeStyleSettings,
        modelSettings: CodeStyleSettings
    ): CodeStyleConfigurable {
        return object : CodeStyleAbstractConfigurable(settings, modelSettings, "Veld") {
            override fun createPanel(settings: CodeStyleSettings): CodeStyleAbstractPanel {
                return VeldCodeStyleMainPanel(currentSettings, settings)
            }
        }
    }

    override fun getConfigurableDisplayName(): String = "Veld"

    private class VeldCodeStyleMainPanel(
        currentSettings: CodeStyleSettings,
        settings: CodeStyleSettings
    ) : TabbedLanguageCodeStylePanel(VeldLanguage, currentSettings, settings)
}

