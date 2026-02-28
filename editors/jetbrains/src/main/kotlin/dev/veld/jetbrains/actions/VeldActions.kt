package dev.veld.jetbrains.actions

import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.execution.process.OSProcessHandler
import com.intellij.execution.process.ProcessAdapter
import com.intellij.execution.process.ProcessEvent
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.ProgressManager
import com.intellij.openapi.progress.Task
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.Messages
import com.intellij.openapi.util.Key
import com.intellij.openapi.wm.ToolWindowManager

/**
 * Base class for Veld CLI actions
 */
abstract class VeldCliAction(private val commandName: String) : AnAction() {

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return

        ProgressManager.getInstance().run(object : Task.Backgroundable(project, "Running Veld $commandName...") {
            override fun run(indicator: ProgressIndicator) {
                runVeldCommand(project, getVeldArgs())
            }
        })
    }

    abstract fun getVeldArgs(): List<String>

    private fun runVeldCommand(project: Project, args: List<String>) {
        val basePath = project.basePath ?: return

        try {
            val commandLine = GeneralCommandLine()
                .withExePath("veld")
                .withParameters(args)
                .withWorkDirectory(basePath)

            val handler = OSProcessHandler(commandLine)
            val output = StringBuilder()
            val errorOutput = StringBuilder()

            handler.addProcessListener(object : ProcessAdapter() {
                override fun onTextAvailable(event: ProcessEvent, outputType: Key<*>) {
                    val text = event.text
                    output.append(text)
                    if (outputType.toString().contains("STDERR")) {
                        errorOutput.append(text)
                    }
                }

                override fun processTerminated(event: ProcessEvent) {
                    ApplicationManager.getApplication().invokeLater {
                        if (event.exitCode == 0) {
                            Messages.showInfoMessage(
                                project,
                                "Veld $commandName completed successfully!\n\n${output.toString().take(500)}",
                                "Veld Success"
                            )
                        } else {
                            Messages.showErrorDialog(
                                project,
                                "Veld $commandName failed:\n\n${errorOutput.toString().take(500)}",
                                "Veld Error"
                            )
                        }
                    }
                }
            })

            handler.startNotify()
            handler.waitFor()

        } catch (e: Exception) {
            ApplicationManager.getApplication().invokeLater {
                Messages.showErrorDialog(
                    project,
                    "Failed to execute veld command:\n${e.message}\n\nMake sure 'veld' is installed and on your PATH.",
                    "Veld Error"
                )
            }
        }
    }
}

/**
 * Action to validate Veld contracts
 */
class VeldValidateAction : VeldCliAction("Validate") {
    override fun getVeldArgs(): List<String> = listOf("validate")
}

/**
 * Action to generate code from Veld contracts
 */
class VeldGenerateAction : VeldCliAction("Generate") {
    override fun getVeldArgs(): List<String> = listOf("generate")
}

/**
 * Action to run dry-run generation
 */
class VeldGenerateDryRunAction : VeldCliAction("Generate (Dry Run)") {
    override fun getVeldArgs(): List<String> = listOf("generate", "--dry-run")
}

