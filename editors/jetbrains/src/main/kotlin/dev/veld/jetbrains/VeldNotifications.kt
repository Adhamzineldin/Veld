package dev.veld.jetbrains

import com.intellij.notification.NotificationAction
import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.options.ShowSettingsUtil
import com.intellij.openapi.project.Project

object VeldNotifications {
    private const val GROUP = "Veld"

    fun executableMissing(project: Project?, configured: String) {
        val what = if (configured.isBlank()) "`veld` was not found on PATH" else "`$configured` does not exist"
        NotificationGroupManager.getInstance()
            .getNotificationGroup(GROUP)
            .createNotification(
                "Veld language server not started",
                "$what. Install Veld or set the path to the executable.",
                NotificationType.ERROR,
            )
            .addAction(
                NotificationAction.createSimple("Open settings") {
                    ShowSettingsUtil.getInstance().showSettingsDialog(project, VeldConfigurable::class.java)
                },
            )
            .notify(project)
    }
}
