package dev.veld.jetbrains

import com.intellij.lang.Language

/** The Veld language. Parsing and analysis live in `veld lsp`, so there is no PSI here. */
object VeldLanguage : Language("Veld")
