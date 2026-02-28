package dev.veld.jetbrains

import com.intellij.extapi.psi.ASTWrapperPsiElement
import com.intellij.lang.ASTNode

/**
 * Basic PSI element wrapper for Veld
 */
class VeldPsiElement(node: ASTNode) : ASTWrapperPsiElement(node)

