package dev.veld.jetbrains

import java.io.File

/** One `veld` CLI invocation offered as an action under Tools | Veld. */
data class VeldCommand(
    /** The action id in plugin.xml. */
    val actionId: String,
    /** The menu text. */
    val title: String,
    /** The arguments after the executable. */
    val args: List<String>,
    /** A long-running command (watch): the console keeps running until the stop button is used. */
    val longRunning: Boolean = false,
)

/** The CLI names planned for Veld 2: `check` includes lint, `generate --check` is the CI staleness check. */
object VeldCommands {
    val VALIDATE = VeldCommand("dev.veld.validate", "Validate Contracts", listOf("check"))
    val GENERATE = VeldCommand("dev.veld.generate", "Generate", listOf("generate"))
    val GENERATE_DRY_RUN = VeldCommand("dev.veld.generateDryRun", "Generate (Dry Run)", listOf("generate", "--dry-run"))
    val GENERATE_CHECK = VeldCommand("dev.veld.generateCheck", "Check Generated Code Is Up To Date", listOf("generate", "--check"))
    val SETUP = VeldCommand("dev.veld.setup", "Setup (Wire Targets Into Apps)", listOf("setup"))
    val WATCH = VeldCommand("dev.veld.watch", "Watch (Generate On Change)", listOf("generate", "--watch"), longRunning = true)
    val DOCTOR = VeldCommand("dev.veld.doctor", "Doctor", listOf("doctor"))

    /** `veld export openapi -o FILE --force` is built by [exportOpenApi] because it needs a target file. */
    const val EXPORT_OPENAPI_ACTION_ID = "dev.veld.exportOpenApi"

    val ALL: List<VeldCommand> = listOf(VALIDATE, GENERATE, GENERATE_DRY_RUN, GENERATE_CHECK, SETUP, WATCH, DOCTOR)

    fun exportOpenApi(file: String): List<String> = listOf("export", "openapi", "-o", file, "--force")

    /** The full argument list, with colour off so the console shows plain text. */
    fun withNoColor(args: List<String>): List<String> = listOf("--no-color") + args
}

/** Finds the directory to run `veld` in. `veld` itself also searches upward for `veld.toml`. */
object VeldProjectDir {
    /** The nearest directory from [start] upward that holds a `veld.toml`, else [fallback]. */
    fun find(start: File?, fallback: File?): File? {
        var dir: File? = start?.let { if (it.isDirectory) it else it.parentFile }
        while (dir != null) {
            if (File(dir, "veld.toml").isFile) return dir
            dir = dir.parentFile
        }
        return fallback
    }
}
