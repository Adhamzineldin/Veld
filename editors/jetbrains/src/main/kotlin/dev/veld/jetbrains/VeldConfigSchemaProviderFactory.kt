package dev.veld.jetbrains

import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.jetbrains.jsonSchema.extension.JsonSchemaFileProvider
import com.jetbrains.jsonSchema.extension.JsonSchemaProviderFactory
import com.jetbrains.jsonSchema.extension.SchemaType

/**
 * Provides JSON schema for veld.config.json files.
 * Enables auto-completion, validation, and documentation for all config keys.
 */
class VeldConfigSchemaProviderFactory : JsonSchemaProviderFactory {
    override fun getProviders(project: Project): List<JsonSchemaFileProvider> {
        return listOf(VeldConfigSchemaProvider())
    }
}

private class VeldConfigSchemaProvider : JsonSchemaFileProvider {
    override fun isAvailable(file: VirtualFile): Boolean {
        return file.name == "veld.config.json"
    }

    override fun getName(): String = "Veld Config"

    override fun getSchemaFile(): VirtualFile? {
        return JsonSchemaProviderFactory.getResourceFile(
            VeldConfigSchemaProvider::class.java,
            "/schemas/veld-config.schema.json"
        )
    }

    override fun getSchemaType(): SchemaType = SchemaType.embeddedSchema
}

