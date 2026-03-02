package com.example.app.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import veld.generated.client.Todo
import veld.generated.client.CreateTodoInput
import veld.generated.client.VeldApi
import veld.generated.client.VeldApiError

// Demonstrates using the Veld-generated VeldApi Kotlin client from an
// Android ViewModel. The generated client lives in generated/client/ApiClient.kt.
class TodoViewModel : ViewModel() {

    private val _todos = MutableStateFlow<List<Todo>>(emptyList())
    val todos: StateFlow<List<Todo>> = _todos

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error

    init {
        VeldApi.baseUrl = "http://10.0.2.2:5000"   // emulator → host loopback
        loadTodos()
    }

    fun loadTodos() {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                _todos.value = VeldApi.Todos.listTodos()
                _error.value = null
            } catch (e: VeldApiError) {
                _error.value = "HTTP ${e.status}: ${e.body}"
            }
        }
    }

    fun addTodo(title: String, userId: String) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val created = VeldApi.Todos.createTodo(CreateTodoInput(title, userId))
                _todos.value = _todos.value + created
            } catch (e: VeldApiError) {
                _error.value = "HTTP ${e.status}: ${e.body}"
            }
        }
    }

    fun deleteTodo(id: String) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                VeldApi.Todos.deleteTodo(id)
                _todos.value = _todos.value.filter { it.id != id }
            } catch (e: VeldApiError) {
                _error.value = "HTTP ${e.status}: ${e.body}"
            }
        }
    }
}
