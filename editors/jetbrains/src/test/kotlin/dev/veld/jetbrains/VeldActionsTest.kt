package dev.veld.jetbrains

import com.intellij.ide.fileTemplates.FileTemplateManager
import com.intellij.openapi.actionSystem.ActionManager
import com.intellij.testFramework.fixtures.BasePlatformTestCase
import java.io.File
import java.nio.file.Files

class VeldActionsTest : BasePlatformTestCase() {
    fun testEveryCliActionIsRegisteredUnderToolsVeld() {
        val manager = ActionManager.getInstance()
        for (command in VeldCommands.ALL) {
            val action = manager.getAction(command.actionId)
            assertNotNull("action ${command.actionId} is not registered", action)
            assertInstanceOf(action, VeldCliAction::class.java)
        }
        assertNotNull(manager.getAction(VeldCommands.EXPORT_OPENAPI_ACTION_ID))
        assertNotNull(manager.getAction("dev.veld.ToolsMenu"))
    }

    fun testNewVeldFileActionIsRegistered() {
        assertInstanceOf(ActionManager.getInstance().getAction("dev.veld.newFile"), VeldNewFileAction::class.java)
    }

    fun testActionsRunThePlannedCliNames() {
        assertEquals(listOf("check"), VeldCommands.VALIDATE.args)
        assertEquals(listOf("generate"), VeldCommands.GENERATE.args)
        assertEquals(listOf("generate", "--dry-run"), VeldCommands.GENERATE_DRY_RUN.args)
        assertEquals(listOf("generate", "--check"), VeldCommands.GENERATE_CHECK.args)
        assertEquals(listOf("generate", "--watch"), VeldCommands.WATCH.args)
        assertTrue(VeldCommands.WATCH.longRunning)
        assertEquals(listOf("export", "openapi", "-o", "api.json"), VeldCommands.exportOpenApi("api.json"))
        assertEquals(listOf("--no-color", "check"), VeldCommands.withNoColor(listOf("check")))
    }

    fun testFileTemplatesExistForEveryNewFileKind() {
        val manager = FileTemplateManager.getInstance(project)
        for ((label, name) in VeldNewFileAction.TEMPLATES) {
            val template = manager.getInternalTemplate(name)
            assertNotNull("template `$name` ($label) is missing", template)
            assertTrue("template `$name` is empty", template.text.isNotBlank())
        }
        val names = VeldNewFileAction.TEMPLATES.map { it.second }.toSet()
        assertTrue(names.containsAll(listOf("Veld Model", "Veld Enum", "Veld Error", "Veld Service", "Veld Workspace Config")))
    }

    fun testTemplatesUseV2Syntax() {
        val manager = FileTemplateManager.getInstance(project)
        for (name in listOf("Veld Model", "Veld Enum", "Veld Error", "Veld Service")) {
            val text = manager.getInternalTemplate(name).text
            assertFalse("v1 keyword in $name", Regex("^[ \t]*(module|action)[ \t]", RegexOption.MULTILINE).containsMatchIn(text))
        }
        assertTrue(manager.getInternalTemplate("Veld Workspace Config").text.contains("[services."))
    }

    fun testTomlSchemaIsBundledAndMappedToVeldToml() {
        assertNotNull(VeldTomlSchemaProvider::class.java.getResource(VeldTomlSchemaProvider.SCHEMA_RESOURCE))
        assertEquals("veld.toml", VeldTomlSchemaProvider.FILE_NAME)
    }

    fun testProjectDirIsTheNearestFolderWithVeldToml() {
        val root = Files.createTempDirectory("veld-dir").toFile()
        try {
            val nested = File(root, "contracts/orders").apply { mkdirs() }
            File(root, "veld.toml").writeText("[project]\nname = \"x\"\n")
            assertEquals(root, VeldProjectDir.find(nested, null))
            assertEquals(root, VeldProjectDir.find(File(nested, "a.veld"), null))
            val other = Files.createTempDirectory("veld-none").toFile()
            assertEquals(other, VeldProjectDir.find(null, other))
            other.delete()
        } finally {
            root.deleteRecursively()
        }
    }
}
