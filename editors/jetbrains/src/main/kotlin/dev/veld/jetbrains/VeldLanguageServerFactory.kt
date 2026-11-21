package dev.veld.jetbrains

import com.intellij.openapi.project.Project
import com.redhat.devtools.lsp4ij.LanguageServerFactory
import com.redhat.devtools.lsp4ij.server.ProcessStreamConnectionProvider
import com.redhat.devtools.lsp4ij.server.StreamConnectionProvider

/** Starts `veld lsp` over stdio. Every language feature comes from that server. */
class VeldLanguageServerFactory : LanguageServerFactory {
    override fun createConnectionProvider(project: Project): StreamConnectionProvider =
        VeldConnectionProvider(project, VeldSettings.getInstance().executablePath)
}

class VeldConnectionProvider(
    private val project: Project?,
    private val configured: String,
) : ProcessStreamConnectionProvider(VeldExecutable.commandLine(configured), project?.basePath) {

    /** The command that will be run, for tests and diagnostics. */
    val commandLine: List<String> get() = VeldExecutable.commandLine(configured)

    override fun start() {
        if (VeldExecutable.locate(configured) == null) {
            VeldNotifications.executableMissing(project, configured)
            throw IllegalStateException("The veld executable was not found")
        }
        super.start()
    }
}
