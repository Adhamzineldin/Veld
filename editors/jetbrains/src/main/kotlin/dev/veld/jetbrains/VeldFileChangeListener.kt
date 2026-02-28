package dev.veld.jetbrains

import com.intellij.openapi.project.ProjectManager
import com.intellij.openapi.vfs.VirtualFileListener
import com.intellij.openapi.vfs.VirtualFileEvent

/**
 * Listener for .veld file changes. Reindexes changed files in VeldProjectService.
 */
class VeldFileChangeListener : VirtualFileListener {

    override fun contentsChanged(event: VirtualFileEvent) {
        if (event.file.extension != "veld") return
        for (project in ProjectManager.getInstance().openProjects) {
            if (project.isDisposed) continue
            val service = VeldProjectService.getInstance(project)
            service.reindexFile(event.file)
        }
    }
}
