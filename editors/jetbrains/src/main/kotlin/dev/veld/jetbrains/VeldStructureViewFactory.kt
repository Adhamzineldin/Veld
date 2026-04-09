package dev.veld.jetbrains

import com.intellij.ide.structureView.*
import com.intellij.ide.util.treeView.smartTree.TreeElement
import com.intellij.lang.PsiStructureViewFactory
import com.intellij.navigation.ItemPresentation
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.util.IconLoader
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFile
import com.intellij.psi.util.PsiTreeUtil
import dev.veld.jetbrains.psi.*
import javax.swing.Icon

/**
 * Structure view factory for Veld files.
 * Shows model/enum/module tree with fields/actions in the Structure tool window.
 */
class VeldStructureViewFactory : PsiStructureViewFactory {

    override fun getStructureViewBuilder(psiFile: PsiFile): StructureViewBuilder? {
        if (psiFile !is VeldPsiFile) return null
        return object : TreeBasedStructureViewBuilder() {
            override fun createStructureViewModel(editor: Editor?): StructureViewModel {
                return VeldStructureViewModel(psiFile, editor)
            }
        }
    }
}

private class VeldStructureViewModel(
    file: VeldPsiFile,
    editor: Editor?
) : StructureViewModelBase(file, editor, VeldFileElement(file)),
    StructureViewModel.ElementInfoProvider {

    override fun isAlwaysShowsPlus(element: StructureViewTreeElement): Boolean = false
    override fun isAlwaysLeaf(element: StructureViewTreeElement): Boolean = false
}

private val VELD_ICON: Icon by lazy {
    IconLoader.getIcon("/icons/veld_16.png", VeldStructureViewFactory::class.java)
}

private class VeldFileElement(private val file: VeldPsiFile) : StructureViewTreeElement {

    override fun getValue(): Any = file
    override fun navigate(requestFocus: Boolean) = file.navigate(requestFocus)
    override fun canNavigate(): Boolean = file.canNavigate()
    override fun canNavigateToSource(): Boolean = file.canNavigateToSource()

    override fun getPresentation(): ItemPresentation = object : ItemPresentation {
        override fun getPresentableText(): String = file.name
        override fun getLocationString(): String? = null
        override fun getIcon(unused: Boolean): Icon = VELD_ICON
    }

    override fun getChildren(): Array<TreeElement> {
        val children = mutableListOf<TreeElement>()

        // Imports
        PsiTreeUtil.getChildrenOfType(file, VeldImportStatement::class.java)?.forEach {
            children.add(VeldImportElement(it))
        }
        // Models
        PsiTreeUtil.getChildrenOfType(file, VeldModelDeclaration::class.java)?.forEach {
            children.add(VeldModelElement(it))
        }
        // Enums
        PsiTreeUtil.getChildrenOfType(file, VeldEnumDeclaration::class.java)?.forEach {
            children.add(VeldEnumElement(it))
        }
        // Constants
        PsiTreeUtil.getChildrenOfType(file, VeldConstantsDeclaration::class.java)?.forEach {
            children.add(VeldConstantsElement(it))
        }
        // Modules
        PsiTreeUtil.getChildrenOfType(file, VeldModuleDeclaration::class.java)?.forEach {
            children.add(VeldModuleElement(it))
        }

        return children.toTypedArray()
    }
}

private class VeldImportElement(private val element: VeldImportStatement) : StructureViewTreeElement {
    override fun getValue(): Any = element
    override fun navigate(requestFocus: Boolean) = element.navigate(requestFocus)
    override fun canNavigate(): Boolean = element.canNavigate()
    override fun canNavigateToSource(): Boolean = element.canNavigateToSource()

    override fun getPresentation(): ItemPresentation = object : ItemPresentation {
        override fun getPresentableText(): String = "import ${element.getImportPath() ?: "?"}"
        override fun getLocationString(): String? = null
        override fun getIcon(unused: Boolean): Icon? = null
    }

    override fun getChildren(): Array<TreeElement> = TreeElement.EMPTY_ARRAY
}

private class VeldModelElement(private val element: VeldModelDeclaration) : StructureViewTreeElement {
    override fun getValue(): Any = element
    override fun navigate(requestFocus: Boolean) = element.navigate(requestFocus)
    override fun canNavigate(): Boolean = element.canNavigate()
    override fun canNavigateToSource(): Boolean = element.canNavigateToSource()

    override fun getPresentation(): ItemPresentation = object : ItemPresentation {
        override fun getPresentableText(): String {
            val name = element.name ?: "?"
            val ext = element.getExtendsName()
            return if (ext != null) "model $name extends $ext" else "model $name"
        }
        override fun getLocationString(): String? = null
        override fun getIcon(unused: Boolean): Icon? = null
    }

    override fun getChildren(): Array<TreeElement> {
        return element.getFields().map { VeldFieldElement(it) }.toTypedArray()
    }
}

private class VeldFieldElement(private val element: VeldFieldDeclaration) : StructureViewTreeElement {
    override fun getValue(): Any = element
    override fun navigate(requestFocus: Boolean) = element.navigate(requestFocus)
    override fun canNavigate(): Boolean = element.canNavigate()
    override fun canNavigateToSource(): Boolean = element.canNavigateToSource()

    override fun getPresentation(): ItemPresentation = object : ItemPresentation {
        override fun getPresentableText(): String {
            val name = element.name ?: "?"
            val opt = if (element.isOptional()) "?" else ""
            val type = element.getFieldType() ?: "?"
            return "$name$opt: $type"
        }
        override fun getLocationString(): String? = null
        override fun getIcon(unused: Boolean): Icon? = null
    }

    override fun getChildren(): Array<TreeElement> = TreeElement.EMPTY_ARRAY
}

private class VeldEnumElement(private val element: VeldEnumDeclaration) : StructureViewTreeElement {
    override fun getValue(): Any = element
    override fun navigate(requestFocus: Boolean) = element.navigate(requestFocus)
    override fun canNavigate(): Boolean = element.canNavigate()
    override fun canNavigateToSource(): Boolean = element.canNavigateToSource()

    override fun getPresentation(): ItemPresentation = object : ItemPresentation {
        override fun getPresentableText(): String {
            val name = element.name ?: "?"
            val vals = element.getValues()
            return if (vals.isNotEmpty()) "enum $name { ${vals.joinToString(" ")} }" else "enum $name"
        }
        override fun getLocationString(): String? = null
        override fun getIcon(unused: Boolean): Icon? = null
    }

    override fun getChildren(): Array<TreeElement> = TreeElement.EMPTY_ARRAY
}

private class VeldModuleElement(private val element: VeldModuleDeclaration) : StructureViewTreeElement {
    override fun getValue(): Any = element
    override fun navigate(requestFocus: Boolean) = element.navigate(requestFocus)
    override fun canNavigate(): Boolean = element.canNavigate()
    override fun canNavigateToSource(): Boolean = element.canNavigateToSource()

    override fun getPresentation(): ItemPresentation = object : ItemPresentation {
        override fun getPresentableText(): String = "module ${element.name ?: "?"}"
        override fun getLocationString(): String? = null
        override fun getIcon(unused: Boolean): Icon? = null
    }

    override fun getChildren(): Array<TreeElement> {
        return element.getActions().map { VeldActionElement(it) }.toTypedArray()
    }
}

private class VeldActionElement(private val element: VeldActionDeclaration) : StructureViewTreeElement {
    override fun getValue(): Any = element
    override fun navigate(requestFocus: Boolean) = element.navigate(requestFocus)
    override fun canNavigate(): Boolean = element.canNavigate()
    override fun canNavigateToSource(): Boolean = element.canNavigateToSource()

    override fun getPresentation(): ItemPresentation = object : ItemPresentation {
        override fun getPresentableText(): String {
            val name = element.name ?: "?"
            val method = element.getMethod() ?: ""
            val path = element.getPath() ?: ""
            return if (method.isNotEmpty()) "action $name ($method $path)" else "action $name"
        }
        override fun getLocationString(): String? = null
        override fun getIcon(unused: Boolean): Icon? = null
    }

    override fun getChildren(): Array<TreeElement> = TreeElement.EMPTY_ARRAY
}

private class VeldConstantsElement(private val element: VeldConstantsDeclaration) : StructureViewTreeElement {
    override fun getValue(): Any = element
    override fun navigate(requestFocus: Boolean) = element.navigate(requestFocus)
    override fun canNavigate(): Boolean = element.canNavigate()
    override fun canNavigateToSource(): Boolean = element.canNavigateToSource()

    override fun getPresentation(): ItemPresentation = object : ItemPresentation {
        override fun getPresentableText(): String {
            val name = element.name ?: "?"
            val fields = element.getFieldNames()
            return if (fields.isNotEmpty()) "constants $name { ${fields.joinToString(", ")} }" else "constants $name"
        }
        override fun getLocationString(): String? = null
        override fun getIcon(unused: Boolean): Icon? = null
    }

    override fun getChildren(): Array<TreeElement> = TreeElement.EMPTY_ARRAY
}

