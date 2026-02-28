package dev.veld.jetbrains

import com.intellij.codeInsight.completion.*
import com.intellij.codeInsight.lookup.LookupElementBuilder
import com.intellij.patterns.PlatformPatterns
import com.intellij.util.ProcessingContext

/**
 * Code completion contributor for Veld language
 */
class VeldCompletionContributor : CompletionContributor() {

    init {
        // Complete keywords
        extend(
            CompletionType.BASIC,
            PlatformPatterns.psiElement(),
            object : CompletionProvider<CompletionParameters>() {
                override fun addCompletions(
                    parameters: CompletionParameters,
                    context: ProcessingContext,
                    result: CompletionResultSet
                ) {
                    // Keywords
                    result.addElement(LookupElementBuilder.create("model").bold())
                    result.addElement(LookupElementBuilder.create("module").bold())
                    result.addElement(LookupElementBuilder.create("action").bold())
                    result.addElement(LookupElementBuilder.create("enum").bold())
                    result.addElement(LookupElementBuilder.create("import").bold())
                    result.addElement(LookupElementBuilder.create("extends").bold())

                    // Directives
                    result.addElement(LookupElementBuilder.create("method:"))
                    result.addElement(LookupElementBuilder.create("path:"))
                    result.addElement(LookupElementBuilder.create("input:"))
                    result.addElement(LookupElementBuilder.create("output:"))
                    result.addElement(LookupElementBuilder.create("description:"))
                    result.addElement(LookupElementBuilder.create("prefix:"))

                    // Built-in types
                    result.addElement(LookupElementBuilder.create("string").withTypeText("type"))
                    result.addElement(LookupElementBuilder.create("int").withTypeText("type"))
                    result.addElement(LookupElementBuilder.create("float").withTypeText("type"))
                    result.addElement(LookupElementBuilder.create("bool").withTypeText("type"))
                    result.addElement(LookupElementBuilder.create("date").withTypeText("type"))
                    result.addElement(LookupElementBuilder.create("datetime").withTypeText("type"))
                    result.addElement(LookupElementBuilder.create("uuid").withTypeText("type"))
                    result.addElement(LookupElementBuilder.create("bytes").withTypeText("type"))
                    result.addElement(LookupElementBuilder.create("json").withTypeText("type"))
                    result.addElement(LookupElementBuilder.create("any").withTypeText("type"))

                    // Generic types
                    result.addElement(LookupElementBuilder.create("List<>").withTypeText("generic"))
                    result.addElement(LookupElementBuilder.create("Map<,>").withTypeText("generic"))

                    // HTTP methods
                    result.addElement(LookupElementBuilder.create("GET").withTypeText("HTTP method"))
                    result.addElement(LookupElementBuilder.create("POST").withTypeText("HTTP method"))
                    result.addElement(LookupElementBuilder.create("PUT").withTypeText("HTTP method"))
                    result.addElement(LookupElementBuilder.create("DELETE").withTypeText("HTTP method"))
                    result.addElement(LookupElementBuilder.create("PATCH").withTypeText("HTTP method"))
                }
            }
        )
    }
}

