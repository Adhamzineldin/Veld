// TodoView.swift — SwiftUI view demonstrating the Veld-generated VeldApi client.
// Requires: generated/client/APIClient.swift linked into your Xcode target.

import SwiftUI

@MainActor
class TodoViewModel: ObservableObject {
    @Published var todos: [Todo] = []
    @Published var errorMessage: String?

    func loadTodos() async {
        do {
            todos = try await VeldApi.Todos.listTodos()
        } catch let err as VeldApiError {
            errorMessage = "Error \(err.status): \(err.body)"
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func addTodo(title: String, userId: String) async {
        do {
            let input = CreateTodoInput(title: title, userId: userId)
            let created = try await VeldApi.Todos.createTodo(input: input)
            todos.append(created)
        } catch let err as VeldApiError {
            errorMessage = "Error \(err.status): \(err.body)"
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    func toggleTodo(_ todo: Todo) async {
        do {
            let input = UpdateTodoInput(completed: !todo.completed)
            let updated = try await VeldApi.Todos.updateTodo(id: todo.id, input: input)
            if let idx = todos.firstIndex(where: { $0.id == updated.id }) {
                todos[idx] = updated
            }
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

struct TodoView: View {
    @StateObject private var vm = TodoViewModel()
    @State private var newTitle = ""

    var body: some View {
        NavigationStack {
            List {
                ForEach(vm.todos, id: \.id) { todo in
                    HStack {
                        Image(systemName: todo.completed ? "checkmark.circle.fill" : "circle")
                            .foregroundColor(todo.completed ? .green : .secondary)
                        Text(todo.title)
                            .strikethrough(todo.completed)
                    }
                    .onTapGesture {
                        Task { await vm.toggleTodo(todo) }
                    }
                }
            }
            .navigationTitle("Todos")
            .toolbar {
                ToolbarItem(placement: .bottomBar) {
                    HStack {
                        TextField("New todo", text: $newTitle)
                            .textFieldStyle(.roundedBorder)
                        Button("Add") {
                            let title = newTitle
                            newTitle = ""
                            Task { await vm.addTodo(title: title, userId: "demo-user") }
                        }
                        .disabled(newTitle.isEmpty)
                    }
                    .padding(.horizontal)
                }
            }
            .alert("Error", isPresented: Binding(
                get: { vm.errorMessage != nil },
                set: { if !$0 { vm.errorMessage = nil } }
            )) {
                Button("OK", role: .cancel) { vm.errorMessage = nil }
            } message: {
                Text(vm.errorMessage ?? "")
            }
        }
        .task { await vm.loadTodos() }
        .onAppear {
            // Point at your running Axum server.
            VeldApi.baseURL = "http://localhost:3000"
        }
    }
}

#Preview {
    TodoView()
}
