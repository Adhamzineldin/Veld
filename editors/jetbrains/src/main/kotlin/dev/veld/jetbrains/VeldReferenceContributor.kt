package dev.veld.jetbrains

import com.intellij.openapi.util.TextRange
import com.intellij.patterns.PlatformPatterns
import com.intellij.psi.*
import com.intellij.psi.util.PsiTreeUtil
import com.intellij.util.ProcessingContext
import dev.veld.jetbrains.psi.VeldEnumDeclaration
import dev.veld.jetbrains.psi.VeldModelDeclaration

/**
 * Reference contributor for Veld language.
 * Enables go-to-definition (Ctrl+Click / Ctrl+B) and find-references for:
 *   - Import paths: @models/auth  -> navigate to the .veld file
 *   - Type names:   User          -> navigate to model/enum/module declaration
 *
 * Handles IMPORT_PATH tokens in any position (import, type, extends).
 */
class VeldReferenceContributor : PsiReferenceContributor() {

    override fun registerReferenceProviders(registrar: PsiReferenceRegistrar) {
        registrar.registerReferenceProvider(
            PlatformPatterns.psiElement(),
            object : PsiReferenceProvider() {
                override fun getReferencesByElement(
                    element: PsiElement,
                    context: ProcessingContext
                ): Array<PsiReference> {
                    val file = element.containingFile?.virtualFile ?: return PsiReference.EMPTY_ARRAY
                    val project = element.project
                    val service = VeldProjectService.getInstance(project)
                    val text = element.text ?: return PsiReference.EMPTY_ARRAY
                    val nodeType = element.node?.elementType

                    // Import path token: @models/auth -> navigate to the file
                    // Works in any position: after import, after colon (type), after extends
                    if (nodeType == VeldTokenTypes.IMPORT_PATH) {
                        val refs = mutableListOf<PsiReference>()
                        // File reference
                        refs.add(VeldImportReference(element, TextRange(0, text.length), text, service))
                        // Also try to resolve to a specific type within the file
                        val typeName = text.substringAfterLast('/').replaceFirstChar { it.uppercase() }
                        if (service.findDefinition(typeName, file) != null) {
                            refs.add(VeldTypeReference(element, TextRange(0, text.length), typeName, service))
                        }
                        return refs.toTypedArray()
                    }

                    // IDENTIFIER: PascalCase -> type reference (model, enum, module)
                    if (nodeType == VeldTokenTypes.IDENTIFIER &&
                        text.isNotEmpty() && text[0].isUpperCase() &&
                        !VeldLanguageSpec.isKeyword(text) &&
                        !VeldLanguageSpec.isBuiltinType(text) &&
                        !VeldLanguageSpec.isSpecialType(text) &&
                        !VeldLanguageSpec.isHttpMethod(text)
                    ) {
                        // First try PSI-based resolution within the same file
                        val psiFile = element.containingFile
                        if (psiFile != null) {
                            val modelDecl = PsiTreeUtil.findChildrenOfType(psiFile, VeldModelDeclaration::class.java)
                                .firstOrNull { it.name == text }
                            if (modelDecl != null) {
                                return arrayOf(VeldPsiElementReference(element, TextRange(0, text.length), modelDecl))
                            }
                            val enumDecl = PsiTreeUtil.findChildrenOfType(psiFile, VeldEnumDeclaration::class.java)
                                .firstOrNull { it.name == text }
                            if (enumDecl != null) {
                                return arrayOf(VeldPsiElementReference(element, TextRange(0, text.length), enumDecl))
                            }
                        }

                        // Fall back to cross-file resolution via VeldProjectService
                        if (service.findDefinition(text, file) != null) {
                            return arrayOf(
                                VeldTypeReference(element, TextRange(0, text.length), text, service)
                            )
                        }
                    }

                    return PsiReference.EMPTY_ARRAY
                }
            }
        )
    }
}

/**
 * Reference to an imported file via @alias/name path.
 * Resolves to the PsiFile for the target .veld file.
 */
class VeldImportReference(
    element: PsiElement,
    range: TextRange,
    private val importPath: String,
    private val service: VeldProjectService
) : PsiReferenceBase<PsiElement>(element, range, true) {

    override fun resolve(): PsiElement? {
        val file = element.containingFile?.virtualFile ?: return null
        val resolved = service.resolveImport(importPath, file) ?: return null
        return PsiManager.getInstance(element.project).findFile(resolved)
    }

    override fun getVariants(): Array<Any> = emptyArray()
}

/**
 * Reference to a model/enum/module type name.
 * Resolves to the declaration identifier across files using PsiTreeUtil.nextLeaf
 * for reliable sequential leaf traversal in both flat and nested PSI trees.
 */
class VeldTypeReference(
    element: PsiElement,
    range: TextRange,
    private val typeName: String,
    private val service: VeldProjectService
) : PsiReferenceBase<PsiElement>(element, range, true) {

    override fun resolve(): PsiElement? {
        val fromFile = element.containingFile?.virtualFile ?: return null
        val (defFile, defLine) = service.findDefinition(typeName, fromFile) ?: return null
        val psiManager = PsiManager.getInstance(element.project)
        val psiFile = psiManager.findFile(defFile) ?: return null

        // Try PSI-based resolution first
        val modelDecl = PsiTreeUtil.findChildrenOfType(psiFile, VeldModelDeclaration::class.java)
            .firstOrNull { it.name == typeName }
        if (modelDecl != null) return modelDecl.nameIdentifier ?: modelDecl

        val enumDecl = PsiTreeUtil.findChildrenOfType(psiFile, VeldEnumDeclaration::class.java)
            .firstOrNull { it.name == typeName }
        if (enumDecl != null) return enumDecl.nameIdentifier ?: enumDecl

        // Fallback: line-based navigation
        val document = PsiDocumentManager.getInstance(element.project)
            .getDocument(psiFile) ?: return psiFile
        if (defLine < 0 || defLine >= document.lineCount) return psiFile

        val lineStart = document.getLineStartOffset(defLine)
        val lineEnd = document.getLineEndOffset(defLine)

        var leaf: PsiElement? = psiFile.findElementAt(lineStart)
        while (leaf != null && leaf.textOffset <= lineEnd) {
            if (leaf.text == typeName && leaf.textOffset >= lineStart) {
                return leaf
            }
            leaf = PsiTreeUtil.nextLeaf(leaf)
        }

        return psiFile.findElementAt(lineStart) ?: psiFile
    }

    override fun getVariants(): Array<Any> = emptyArray()
}

/**
 * Direct PSI element reference for same-file navigation.
 */
class VeldPsiElementReference(
    element: PsiElement,
    range: TextRange,
    private val target: PsiElement
) : PsiReferenceBase<PsiElement>(element, range, true) {

    override fun resolve(): PsiElement = target

    override fun getVariants(): Array<Any> = emptyArray()
}
