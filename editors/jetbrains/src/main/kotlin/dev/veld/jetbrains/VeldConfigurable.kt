package dev.veld.jetbrains

import com.intellij.openapi.fileChooser.FileChooserDescriptorFactory
import com.intellij.openapi.options.BoundSearchableConfigurable
import com.intellij.openapi.ui.DialogPanel
import com.intellij.ui.dsl.builder.AlignX
import com.intellij.ui.dsl.builder.bindText
import com.intellij.ui.dsl.builder.panel

/** Settings | Tools | Veld. A change applies the next time the language server starts. */
class VeldConfigurable : BoundSearchableConfigurable("Veld", "dev.veld.jetbrains.settings") {
    override fun createPanel(): DialogPanel {
        val settings = VeldSettings.getInstance()
        return panel {
            row("Path to veld:") {
                textFieldWithBrowseButton(FileChooserDescriptorFactory.createSingleFileDescriptor().withTitle("Select the veld Executable"))
                    .align(AlignX.FILL)
                    .bindText(settings::executablePath)
                    .comment("Leave empty to find <code>veld</code> on PATH. Restart the language server after a change.")
            }
        }
    }
}
