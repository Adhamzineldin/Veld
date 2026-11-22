package dev.veld.jetbrains

import com.intellij.openapi.fileTypes.FileTypeManager
import com.intellij.testFramework.fixtures.BasePlatformTestCase

class VeldPluginTest : BasePlatformTestCase() {
    fun testVeldFileTypeIsRegisteredForVeldFiles() {
        val type = FileTypeManager.getInstance().getFileTypeByFileName("hello.veld")
        assertSame(VeldFileType, type)
        assertNotNull(type.icon)
    }

    fun testBlankSettingResolvesToVeldOnPath() {
        VeldSettings.getInstance().executablePath = ""
        val provider = VeldLanguageServerFactory().createConnectionProvider(project) as VeldConnectionProvider
        assertEquals(listOf("veld", "lsp"), provider.commandLine)
    }

    fun testConfiguredPathIsTheServerCommand() {
        val settings = VeldSettings.getInstance()
        val before = settings.executablePath
        try {
            settings.executablePath = "  /opt/veld/bin/veld  "
            val provider = VeldLanguageServerFactory().createConnectionProvider(project) as VeldConnectionProvider
            assertEquals(listOf("/opt/veld/bin/veld", "lsp"), provider.commandLine)
        } finally {
            settings.executablePath = before
        }
    }

    fun testMissingExecutableIsReportedNotFound() {
        assertNull(VeldExecutable.locate("/definitely/not/here/veld"))
    }
}
