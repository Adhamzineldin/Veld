package dev.veld.jetbrains

import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.openapi.vfs.VfsUtil
import java.io.File

/**
 * Central project-level service that indexes all .veld files in the workspace.
 * Provides model/enum/module lookup, import resolution, and cross-file references.
 * Registered in plugin.xml as a projectService.
 */
class VeldProjectService(private val project: Project) {

    data class FieldDef(val name: String, val type: String, val line: Int)
    data class ModelDef(val name: String, val file: VirtualFile, val line: Int, val fields: List<FieldDef>)
    data class EnumDef(val name: String, val file: VirtualFile, val line: Int, val values: List<String>)
    data class ActionDef(
        val name: String, val line: Int,
        val method: String?, val path: String?,
        val input: String?, val output: String?
    )
    data class ModuleDef(val name: String, val file: VirtualFile, val line: Int, val actions: List<ActionDef>)
    data class ImportDef(val raw: String, val alias: String, val name: String, val line: Int)

    data class VeldFileIndex(
        val file: VirtualFile,
        val models: List<ModelDef>,
        val enums: List<EnumDef>,
        val modules: List<ModuleDef>,
        val imports: List<ImportDef>
    )

    private val fileIndices = mutableMapOf<String, VeldFileIndex>()

    // ── Public API ────────────────────────────────────

    fun getIndex(file: VirtualFile): VeldFileIndex? {
        val key = file.path
        if (!fileIndices.containsKey(key)) {
            reindexFile(file)
        }
        return fileIndices[key]
    }

    fun reindexFile(file: VirtualFile) {
        if (file.extension != "veld") return
        val content = try { String(file.contentsToByteArray(), Charsets.UTF_8) } catch (e: Exception) { return }
        fileIndices[file.path] = parseVeldFile(file, content)
    }

    fun reindexFile(file: VirtualFile, content: String) {
        if (file.extension != "veld") return
        fileIndices[file.path] = parseVeldFile(file, content)
    }

    /**
     * Find the veld project root (directory containing veld.config.json or app.veld).
     * Also checks for veld/ subfolder — returns it as the root if found.
     */
    fun findProjectRoot(file: VirtualFile): VirtualFile? {
        var dir = file.parent
        for (i in 0 until 15) {
            if (dir == null) break
            if (dir.findChild("veld.config.json") != null) return dir
            // Check veld/ subfolder — return it as root (that's where .veld files live)
            val veldSubDir = dir.findChild("veld")
            if (veldSubDir != null && veldSubDir.isDirectory && veldSubDir.findChild("veld.config.json") != null) {
                return veldSubDir
            }
            if (dir.findChild("app.veld") != null) return dir
            dir = dir.parent
        }
        // Fallback: project base path
        val basePath = project.basePath ?: return null
        return VfsUtil.findFileByIoFile(File(basePath), true)
    }

    /** Default alias → folder mappings (matches Go config.DefaultAliases). */
    private val defaultAliases = mapOf(
        "models" to "models", "modules" to "modules", "types" to "types",
        "enums" to "enums", "schemas" to "schemas", "services" to "services",
        "lib" to "lib", "common" to "common", "shared" to "shared"
    )

    /** Read aliases from veld.config.json — falls back to alias=folder convention. */
    private fun readAliases(root: VirtualFile): Map<String, String> {
        val configFile = root.findChild("veld.config.json")
            ?: root.findChild("veld")?.findChild("veld.config.json")
            ?: return defaultAliases
        return try {
            val content = String(configFile.contentsToByteArray(), Charsets.UTF_8)
            val aliases = defaultAliases.toMutableMap()
            // Simple regex-based extraction of "aliases": { "key": "value", ... }
            val aliasBlock = Regex(""""aliases"\s*:\s*\{([^}]*)\}""").find(content)
            if (aliasBlock != null) {
                val pairs = Regex(""""(\w+)"\s*:\s*"([^"]+)"""").findAll(aliasBlock.groupValues[1])
                for (pair in pairs) {
                    aliases[pair.groupValues[1]] = pair.groupValues[2]
                }
            }
            aliases
        } catch (_: Exception) {
            defaultAliases
        }
    }

    /** Resolve an alias name to its folder VirtualFile using config aliases. */
    fun resolveAliasDir(alias: String, root: VirtualFile): VirtualFile? {
        val aliases = readAliases(root)
        val folder = aliases[alias] ?: alias
        return root.findFileByRelativePath(folder)
    }

    /**
     * Resolve an import path to a VirtualFile.
     * Supports: @alias/name, /path/name, and quoted relative paths.
     */
    fun resolveImport(importPath: String, fromFile: VirtualFile): VirtualFile? {
        // Relative quoted imports (e.g. "../models/foo.model.veld") must be resolved directly.
        // The alias regex below would incorrectly match "/models/foo" inside the path,
        // routing them through alias resolution instead of relative resolution.
        if (importPath.startsWith(".")) {
            return fromFile.parent?.findFileByRelativePath(importPath)
        }

        // @alias/name or /alias/name
        val aliasMatch = Regex("""[@/](\w+)/(\w+)""").find(importPath)
        if (aliasMatch != null) {
            val alias = aliasMatch.groupValues[1]
            val name = aliasMatch.groupValues[2]
            val root = findProjectRoot(fromFile) ?: return null
            val dir = resolveAliasDir(alias, root) ?: return null
            // Try exact match first, then compound extensions
            val candidates = listOf(
                "$name.veld",
                "$name.model.veld",
                "$name.module.veld",
                "$name.enum.veld",
                "$name.types.veld",
                "$name.schema.veld"
            )
            for (candidate in candidates) {
                val found = dir.findChild(candidate)
                if (found != null) return found
            }
            return null
        }
        // Quoted relative path (already resolved to a file name at parse time)
        if (importPath.endsWith(".veld") || !importPath.startsWith("@")) {
            val parentDir = fromFile.parent ?: return null
            return parentDir.findFileByRelativePath(importPath)
        }
        return null
    }

    /**
     * Get all models visible from a file (local + imported).
     */
    fun getVisibleModels(file: VirtualFile): List<ModelDef> {
        val index = getIndex(file) ?: return emptyList()
        val result = mutableListOf<ModelDef>()
        result.addAll(index.models)

        for (imp in index.imports) {
            val resolved = resolveImport(imp.raw, file) ?: continue
            val importedIndex = getIndex(resolved) ?: continue
            result.addAll(importedIndex.models)
        }
        return result
    }

    /**
     * Get all enums visible from a file (local + imported).
     */
    fun getVisibleEnums(file: VirtualFile): List<EnumDef> {
        val index = getIndex(file) ?: return emptyList()
        val result = mutableListOf<EnumDef>()
        result.addAll(index.enums)

        for (imp in index.imports) {
            val resolved = resolveImport(imp.raw, file) ?: continue
            val importedIndex = getIndex(resolved) ?: continue
            result.addAll(importedIndex.enums)
        }
        return result
    }

    /**
     * Get all modules visible from a file (local + imported).
     */
    fun getVisibleModules(file: VirtualFile): List<ModuleDef> {
        val index = getIndex(file) ?: return emptyList()
        val result = mutableListOf<ModuleDef>()
        result.addAll(index.modules)

        for (imp in index.imports) {
            val resolved = resolveImport(imp.raw, file) ?: continue
            val importedIndex = getIndex(resolved) ?: continue
            result.addAll(importedIndex.modules)
        }
        return result
    }

    /**
     * Find the definition of a type name (model or enum) visible from a file.
     * Returns the file + line where it is defined.
     */
    fun findDefinition(typeName: String, fromFile: VirtualFile): Pair<VirtualFile, Int>? {
        val models = getVisibleModels(fromFile)
        models.find { it.name == typeName }?.let { return Pair(it.file, it.line) }

        val enums = getVisibleEnums(fromFile)
        enums.find { it.name == typeName }?.let { return Pair(it.file, it.line) }

        val modules = getVisibleModules(fromFile)
        modules.find { it.name == typeName }?.let { return Pair(it.file, it.line) }

        return null
    }

    /**
     * Find all .veld files that reference a given type name.
     */
    fun findReferences(typeName: String, rootFile: VirtualFile): List<Pair<VirtualFile, Int>> {
        val results = mutableListOf<Pair<VirtualFile, Int>>()
        val root = findProjectRoot(rootFile) ?: return results
        collectVeldFiles(root).forEach { vf ->
            val content = try { String(vf.contentsToByteArray(), Charsets.UTF_8) } catch (e: Exception) { return@forEach }
            val lines = content.split("\n")
            for ((i, line) in lines.withIndex()) {
                if (Regex("""\b${Regex.escape(typeName)}\b""").containsMatchIn(line)) {
                    results.add(Pair(vf, i))
                }
            }
        }
        return results
    }

    /**
     * Collect all .veld files under a directory recursively.
     */
    fun collectVeldFiles(dir: VirtualFile): List<VirtualFile> {
        val result = mutableListOf<VirtualFile>()
        VfsUtil.iterateChildrenRecursively(dir, { true }) { file ->
            if (file.extension == "veld" && !file.isDirectory) {
                result.add(file)
            }
            true
        }
        return result
    }

    // ── Parsing ───────────────────────────────────────

    private fun parseVeldFile(file: VirtualFile, content: String): VeldFileIndex {
        val lines = content.split("\n")
        val models = mutableListOf<ModelDef>()
        val enums = mutableListOf<EnumDef>()
        val modules = mutableListOf<ModuleDef>()
        val imports = mutableListOf<ImportDef>()

        var i = 0
        while (i < lines.size) {
            val trimmed = lines[i].trim()

            // Parse import — supports all syntaxes:
            //   import @models/user
            //   import @models/*
            //   import /models/user
            //   import /models/*
            //   from @models import *
            //   import "./path/file.veld"
            if (trimmed.startsWith("import") || trimmed.startsWith("from")) {
                val singleMatch = Regex("""import\s+[@/](\w+)/(\w+)""").find(trimmed)
                val wildcardMatch = Regex("""import\s+[@/](\w+)/\*""").find(trimmed)
                val fromWildcard = Regex("""from\s+[@/](\w+)\s+import\s+\*""").find(trimmed)
                val fromNamed = Regex("""from\s+[@/](\w+)\s+import\s+(.+)""").find(trimmed)
                val quotedMatch = Regex("""import\s+"([^"]+)"""").find(trimmed)

                if (singleMatch != null && wildcardMatch == null) {
                    val alias = singleMatch.groupValues[1]
                    val name = singleMatch.groupValues[2]
                    imports.add(ImportDef("@$alias/$name", alias, name, i))
                } else if (wildcardMatch != null) {
                    // Wildcard: resolve all .veld files in the alias dir
                    val alias = wildcardMatch.groupValues[1]
                    val root = findProjectRoot(file)
                    val dir = if (root != null) resolveAliasDir(alias, root) else null
                    if (dir != null && dir.isDirectory) {
                        for (child in dir.children) {
                            if (child.extension == "veld") {
                                val name = child.nameWithoutExtension
                                imports.add(ImportDef("@$alias/$name", alias, name, i))
                            }
                        }
                    }
                } else if (fromWildcard != null) {
                    // from @models import * — same as wildcard
                    val alias = fromWildcard.groupValues[1]
                    val root = findProjectRoot(file)
                    val dir = if (root != null) resolveAliasDir(alias, root) else null
                    if (dir != null && dir.isDirectory) {
                        for (child in dir.children) {
                            if (child.extension == "veld") {
                                val name = child.nameWithoutExtension
                                imports.add(ImportDef("@$alias/$name", alias, name, i))
                            }
                        }
                    }
                } else if (fromNamed != null && fromWildcard == null) {
                    // from @models import User, Role — named imports
                    val alias = fromNamed.groupValues[1]
                    val nameList = fromNamed.groupValues[2]
                    val names = nameList.split(",").map { it.trim() }.filter { it.isNotEmpty() && it != "*" }
                    for (name in names) {
                        imports.add(ImportDef("@$alias/$name", alias, name, i))
                    }
                } else if (quotedMatch != null) {
                    // Quoted path: import "./models/user.veld"
                    val relPath = quotedMatch.groupValues[1]
                    val parentDir = file.parent
                    if (parentDir != null) {
                        val resolved = parentDir.findFileByRelativePath(relPath)
                        if (resolved != null && resolved.extension == "veld") {
                            val name = resolved.nameWithoutExtension
                            imports.add(ImportDef(relPath, "", name, i))
                        }
                    }
                }
                i++
                continue
            }

            // Parse model
            if (trimmed.startsWith("model ")) {
                val match = Regex("""model\s+([A-Za-z_]\w*)""").find(trimmed)
                if (match != null) {
                    val modelName = match.groupValues[1]
                    val fields = mutableListOf<FieldDef>()
                    val modelLine = i
                    i++
                    while (i < lines.size && lines[i].trim() != "}") {
                        val fieldLine = lines[i].trim()
                        val fieldMatch = Regex("""^([a-z_]\w*)\??:\s*(.+?)(?:\s*//.*)?$""").find(fieldLine)
                        if (fieldMatch != null) {
                            // Strip @annotations (e.g. @default(user)) so type is clean for display
                            val rawType = fieldMatch.groupValues[2].trim()
                            val cleanType = rawType.replace(Regex("""@\w+(?:\([^)]*\))?"""), "").trim()
                            fields.add(FieldDef(fieldMatch.groupValues[1], cleanType, i))
                        }
                        i++
                    }
                    models.add(ModelDef(modelName, file, modelLine, fields))
                    i++
                    continue
                }
            }

            // Parse enum
            if (trimmed.startsWith("enum ")) {
                val match = Regex("""enum\s+([A-Za-z_]\w*)""").find(trimmed)
                if (match != null) {
                    val enumName = match.groupValues[1]
                    val values = mutableListOf<String>()
                    val enumLine = i
                    i++
                    while (i < lines.size && lines[i].trim() != "}") {
                        val valueLine = lines[i].trim()
                        if (valueLine.isNotEmpty() && !valueLine.startsWith("//")) {
                            // Enum values can be space-separated or one per line
                            valueLine.split(Regex("""\s+""")).forEach { v ->
                                if (v.isNotEmpty()) values.add(v)
                            }
                        }
                        i++
                    }
                    enums.add(EnumDef(enumName, file, enumLine, values))
                    i++
                    continue
                }
            }

            // Parse module
            if (trimmed.startsWith("module ")) {
                val match = Regex("""module\s+([A-Za-z_]\w*)""").find(trimmed)
                if (match != null) {
                    val moduleName = match.groupValues[1]
                    val actions = mutableListOf<ActionDef>()
                    val moduleLine = i
                    i++

                    var braceDepth = 1
                    var currentActionName: String? = null
                    var currentActionLine = 0
                    var currentMethod: String? = null
                    var currentPath: String? = null
                    var currentInput: String? = null
                    var currentOutput: String? = null

                    while (i < lines.size && braceDepth > 0) {
                        val actionLine = lines[i].trim()

                        if (actionLine == "}") {
                            braceDepth--
                            if (braceDepth == 1 && currentActionName != null) {
                                actions.add(ActionDef(currentActionName, currentActionLine, currentMethod, currentPath, currentInput, currentOutput))
                                currentActionName = null
                                currentMethod = null; currentPath = null; currentInput = null; currentOutput = null
                            }
                        } else if (actionLine.contains("{")) {
                            braceDepth++
                            val actionMatch = Regex("""action\s+([A-Za-z_]\w*)""").find(actionLine)
                            if (actionMatch != null) {
                                currentActionName = actionMatch.groupValues[1]
                                currentActionLine = i
                            }
                        } else if (currentActionName != null) {
                            val dirMatch = Regex("""^\s*(\w+):\s*(.+)""").find(actionLine)
                            if (dirMatch != null) {
                                when (dirMatch.groupValues[1]) {
                                    "method" -> currentMethod = dirMatch.groupValues[2].trim()
                                    "path" -> currentPath = dirMatch.groupValues[2].trim()
                                    "input" -> currentInput = dirMatch.groupValues[2].trim()
                                    "output" -> currentOutput = dirMatch.groupValues[2].trim()
                                }
                            }
                        }
                        i++
                    }
                    // Flush last action if module ended abruptly
                    if (currentActionName != null) {
                        actions.add(ActionDef(currentActionName, currentActionLine, currentMethod, currentPath, currentInput, currentOutput))
                    }
                    modules.add(ModuleDef(moduleName, file, moduleLine, actions))
                    continue
                }
            }

            i++
        }

        return VeldFileIndex(file, models, enums, modules, imports)
    }

    companion object {
        fun getInstance(project: Project): VeldProjectService {
            return project.getService(VeldProjectService::class.java)
        }
    }
}


