package dev.veld.jetbrains

import com.intellij.application.options.IndentOptionsEditor
import com.intellij.lang.Language
import com.intellij.psi.codeStyle.CodeStyleSettingsCustomizable
import com.intellij.psi.codeStyle.CommonCodeStyleSettings
import com.intellij.psi.codeStyle.LanguageCodeStyleSettingsProvider

/**
 * Language code style settings provider for Veld
 */
class VeldLanguageCodeStyleSettingsProvider : LanguageCodeStyleSettingsProvider() {

    override fun getLanguage(): Language = VeldLanguage

    override fun getCodeSample(settingsType: SettingsType): String = """
        model User {
            id: int
            email: string
            name: string
        }

        module users {
            action GetUser {
                method: GET
                path: /:id
                output: User
            }
        }
    """.trimIndent()

    override fun customizeSettings(consumer: CodeStyleSettingsCustomizable, settingsType: SettingsType) {
        when (settingsType) {
            SettingsType.SPACING_SETTINGS -> {
                consumer.showStandardOptions("SPACE_AROUND_ASSIGNMENT_OPERATORS")
            }
            SettingsType.INDENT_SETTINGS -> {
                consumer.showStandardOptions("INDENT_SIZE", "TAB_SIZE")
            }
            else -> {}
        }
    }

    override fun getIndentOptionsEditor(): IndentOptionsEditor? = IndentOptionsEditor()

    override fun getDefaultCommonSettings(): CommonCodeStyleSettings {
        val commonSettings = CommonCodeStyleSettings(language)
        val indentOptions = commonSettings.initIndentOptions()
        indentOptions.INDENT_SIZE = 2
        indentOptions.TAB_SIZE = 2
        indentOptions.USE_TAB_CHARACTER = false
        return commonSettings
    }
}

