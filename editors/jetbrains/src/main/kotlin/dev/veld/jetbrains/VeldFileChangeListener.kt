package dev.veld.jetbrains

import com.intellij.openapi.vfs.VirtualFileListener
import com.intellij.openapi.vfs.VirtualFileEvent

/**
 * Listener for .veld file changes to trigger validation
 */
class VeldFileChangeListener : VirtualFileListener {

    override fun contentsChanged(event: VirtualFileEvent) {
        if (event.file.extension == "veld") {
            // Trigger validation - handled by ExternalAnnotator
        }
    }
}

