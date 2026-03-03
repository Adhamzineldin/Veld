package dev.veld.jetbrains

import com.intellij.ide.actions.CreateFileFromTemplateAction
import com.intellij.ide.actions.CreateFileFromTemplateDialog
import com.intellij.openapi.project.Project
import com.intellij.openapi.util.IconLoader
import com.intellij.psi.PsiDirectory

/**
 * "New -> Veld File" action in the Project view context menu.
 * Provides sub-options for creating Model, Module, and Enum files.
 */
class VeldCreateFileAction : CreateFileFromTemplateAction(
    "Veld File",
    "Create a new Veld contract file",
    IconLoader.getIcon("/icons/veld_16.png", VeldCreateFileAction::class.java)
) {

    override fun buildDialog(project: Project, directory: PsiDirectory, builder: CreateFileFromTemplateDialog.Builder) {
        val icon = IconLoader.getIcon("/icons/veld_16.png", VeldCreateFileAction::class.java)
        builder
            .setTitle("New Veld File")
            .addKind("App Entry Point", icon, "Veld App")
            .addKind("Model", icon, "Veld Model")
            .addKind("Module", icon, "Veld Module")
            .addKind("Enum", icon, "Veld Enum")
            .addKind("Config (veld.config.json)", icon, "Veld Config")
    }

    override fun getActionName(directory: PsiDirectory, newName: String, templateName: String): String =
        "Create Veld File: $newName"
}
