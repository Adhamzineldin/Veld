package dev.veld.jetbrains

import com.intellij.execution.ExecutionException
import com.intellij.execution.RunContentExecutor
import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.execution.process.KillableProcessHandler
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import java.io.File
import java.nio.charset.StandardCharsets

/** Runs `veld` in a run console (the Run tool window), with a rerun and a stop button. */
object VeldRunner {
    fun run(project: Project, title: String, args: List<String>, context: VirtualFile? = null) {
        FileDocumentManager.getInstance().saveAllDocuments()
        val configured = VeldSettings.getInstance().executablePath
        if (VeldExecutable.locate(configured) == null) {
            VeldNotifications.executableMissing(project, configured)
            return
        }
        val workDir = VeldProjectDir.find(context?.let { File(it.path) }, project.basePath?.let { File(it) })
        val command = GeneralCommandLine(VeldExecutable.command(configured))
            .withParameters(VeldCommands.withNoColor(args))
            .withCharset(StandardCharsets.UTF_8)
        if (workDir != null) command.withWorkDirectory(workDir)
        try {
            val handler = KillableProcessHandler(command)
            RunContentExecutor(project, handler)
                .withTitle("Veld: $title")
                .withRerun { run(project, title, args, context) }
                .withStop({ handler.destroyProcess() }, { !handler.isProcessTerminated })
                .run()
        } catch (e: ExecutionException) {
            VeldNotifications.executableMissing(project, configured)
        }
    }
}
