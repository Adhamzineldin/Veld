package dev.veld.jetbrains

import com.intellij.openapi.project.ProjectManager
import com.intellij.openapi.vfs.newvfs.BulkFileListener
import com.intellij.openapi.vfs.newvfs.events.VFileContentChangeEvent
import com.intellij.openapi.vfs.newvfs.events.VFileEvent

/**
 * Listener for .veld file changes. Reindexes changed files in VeldProjectService.
 *
 * Uses [BulkFileListener] (instead of the legacy VirtualFileListener) so the
 * plugin can be loaded and unloaded dynamically without an IDE restart.
 */
class VeldFileChangeListener : BulkFileListener {

    override fun after(events: MutableList<out VFileEvent>) {
        val veldEvents = events.filterIsInstance<VFileContentChangeEvent>()
            .filter { it.file.extension == "veld" }
        if (veldEvents.isEmpty()) return

        for (project in ProjectManager.getInstance().openProjects) {
            if (project.isDisposed) continue
            val service = VeldProjectService.getInstance(project)
            for (event in veldEvents) {
                service.reindexFile(event.file)
            }
        }
    }
}
