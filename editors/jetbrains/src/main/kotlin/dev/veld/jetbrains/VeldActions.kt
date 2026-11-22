package dev.veld.jetbrains

import com.intellij.ide.actions.CreateFileFromTemplateAction
import com.intellij.ide.actions.CreateFileFromTemplateDialog
import com.intellij.openapi.actionSystem.ActionUpdateThread
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.actionSystem.CommonDataKeys
import com.intellij.openapi.fileChooser.FileChooserFactory
import com.intellij.openapi.fileChooser.FileSaverDescriptor
import com.intellij.openapi.project.DumbAware
import com.intellij.openapi.project.DumbAwareAction
import com.intellij.openapi.project.Project
import com.intellij.psi.PsiDirectory
import java.io.File

/** An action under Tools | Veld that runs one CLI command in a run console. */
abstract class VeldCliAction(private val command: VeldCommand) : DumbAwareAction(command.title) {
    override fun getActionUpdateThread(): ActionUpdateThread = ActionUpdateThread.BGT

    override fun update(e: AnActionEvent) {
        e.presentation.isEnabledAndVisible = e.project != null
    }

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        VeldRunner.run(project, command.title, command.args, e.getData(CommonDataKeys.VIRTUAL_FILE))
    }
}

class VeldValidateAction : VeldCliAction(VeldCommands.VALIDATE)
class VeldGenerateAction : VeldCliAction(VeldCommands.GENERATE)
class VeldGenerateDryRunAction : VeldCliAction(VeldCommands.GENERATE_DRY_RUN)
class VeldGenerateCheckAction : VeldCliAction(VeldCommands.GENERATE_CHECK)
class VeldSetupAction : VeldCliAction(VeldCommands.SETUP)
class VeldWatchAction : VeldCliAction(VeldCommands.WATCH)
class VeldDoctorAction : VeldCliAction(VeldCommands.DOCTOR)

/** Tools | Veld | Export OpenAPI...: asks for a file, then runs `veld export openapi -o FILE`. */
class VeldExportOpenApiAction : DumbAwareAction("Export OpenAPI...") {
    override fun getActionUpdateThread(): ActionUpdateThread = ActionUpdateThread.BGT

    override fun update(e: AnActionEvent) {
        e.presentation.isEnabledAndVisible = e.project != null
    }

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return
        val descriptor = FileSaverDescriptor("Export OpenAPI", "Where to write the OpenAPI document", "json")
        val base = project.basePath?.let { File(it) }?.let { com.intellij.openapi.vfs.LocalFileSystem.getInstance().findFileByIoFile(it) }
        val saved = FileChooserFactory.getInstance().createSaveFileDialog(descriptor, project).save(base, "openapi.json") ?: return
        VeldRunner.run(project, "Export OpenAPI", VeldCommands.exportOpenApi(saved.file.path), e.getData(CommonDataKeys.VIRTUAL_FILE))
    }
}

/** File | New | Veld File: model, enum, error, service, or a `veld.toml`. */
class VeldNewFileAction : CreateFileFromTemplateAction("Veld File", "Create a new Veld contract or config file", VeldIcons.FILE), DumbAware {
    override fun buildDialog(project: Project, directory: PsiDirectory, builder: CreateFileFromTemplateDialog.Builder) {
        builder.setTitle("New Veld File")
        for ((label, template) in TEMPLATES) {
            builder.addKind(label, VeldIcons.FILE, template)
        }
    }

    override fun getActionName(directory: PsiDirectory?, newName: String, templateName: String?): String = "Veld File"

    companion object {
        /** Dialog label to file template name (`fileTemplates/internal/<name>.<ext>.ft`). */
        val TEMPLATES: List<Pair<String, String>> = listOf(
            "Model" to "Veld Model",
            "Enum" to "Veld Enum",
            "Error" to "Veld Error",
            "Service" to "Veld Service",
            "veld.toml (single service)" to "Veld Config",
            "veld.toml (workspace)" to "Veld Workspace Config",
        )
    }
}
