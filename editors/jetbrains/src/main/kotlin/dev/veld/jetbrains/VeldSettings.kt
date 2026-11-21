package dev.veld.jetbrains

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage

/** The application-wide setting for the `veld` executable. Blank means "look it up on PATH". */
@Service(Service.Level.APP)
@State(name = "VeldSettings", storages = [Storage("veld.xml")])
class VeldSettings : PersistentStateComponent<VeldSettings.State> {
    class State {
        var executablePath: String = ""
    }

    private var state = State()

    override fun getState(): State = state

    override fun loadState(state: State) {
        this.state = state
    }

    var executablePath: String
        get() = state.executablePath.trim()
        set(value) {
            state.executablePath = value.trim()
        }

    companion object {
        fun getInstance(): VeldSettings = ApplicationManager.getApplication().getService(VeldSettings::class.java)
    }
}
