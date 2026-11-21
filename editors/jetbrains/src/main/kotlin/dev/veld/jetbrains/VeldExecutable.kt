package dev.veld.jetbrains

import com.intellij.execution.configurations.PathEnvironmentVariableUtil
import java.io.File

/** Resolves the `veld` executable from the setting and builds the `veld lsp` command line. */
object VeldExecutable {
    const val DEFAULT_NAME = "veld"

    /** The configured command, or [DEFAULT_NAME] when the setting is blank. */
    fun command(configured: String): String = configured.trim().ifEmpty { DEFAULT_NAME }

    fun commandLine(configured: String): List<String> = listOf(command(configured), "lsp")

    /** The file the command resolves to, or null if it does not exist. */
    fun locate(configured: String): File? {
        val command = command(configured)
        val direct = File(command)
        if (direct.isAbsolute || command.contains('/') || command.contains('\\')) {
            return direct.takeIf { it.isFile }
        }
        return PathEnvironmentVariableUtil.findInPath(command)
    }
}
