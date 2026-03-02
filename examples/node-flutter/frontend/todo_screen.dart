import 'package:flutter/material.dart';
import '../generated/client/api_client.dart';

// Connects to the Veld-generated Dart client.
// Run `veld generate` from the veld/ directory first, then wire this
// screen into your Flutter app's router.
final _client = VeldApi(baseUrl: 'http://localhost:3000');

class TodoScreen extends StatefulWidget {
  const TodoScreen({super.key});

  @override
  State<TodoScreen> createState() => _TodoScreenState();
}

class _TodoScreenState extends State<TodoScreen> {
  List<Todo> _todos = [];
  bool _loading = true;
  String? _error;
  final _titleController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _loadTodos();
  }

  Future<void> _loadTodos() async {
    try {
      final todos = await _client.listTodos();
      setState(() { _todos = todos; _loading = false; });
    } on VeldApiError catch (e) {
      setState(() { _error = 'API error ${e.status}'; _loading = false; });
    }
  }

  Future<void> _addTodo() async {
    final title = _titleController.text.trim();
    if (title.isEmpty) return;
    try {
      final todo = await _client.createTodo(
        CreateTodoInput(title: title, userId: '1'),
      );
      setState(() { _todos = [..._todos, todo]; });
      _titleController.clear();
    } on VeldApiError catch (e) {
      setState(() { _error = 'Failed to create todo: ${e.status}'; });
    }
  }

  Future<void> _toggleTodo(Todo todo) async {
    try {
      final updated = await _client.updateTodo(
        todo.id,
        UpdateTodoInput(completed: !todo.completed),
      );
      setState(() {
        _todos = _todos.map((t) => t.id == updated.id ? updated : t).toList();
      });
    } on VeldApiError catch (e) {
      setState(() { _error = 'Failed to update todo: ${e.status}'; });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Todos')),
      body: _loading
          ? const Center(child: CircularProgressIndicator())
          : Column(
              children: [
                if (_error != null)
                  Padding(
                    padding: const EdgeInsets.all(8),
                    child: Text(_error!, style: const TextStyle(color: Colors.red)),
                  ),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                  child: Row(
                    children: [
                      Expanded(
                        child: TextField(
                          controller: _titleController,
                          decoration: const InputDecoration(hintText: 'New todo…'),
                        ),
                      ),
                      const SizedBox(width: 8),
                      ElevatedButton(onPressed: _addTodo, child: const Text('Add')),
                    ],
                  ),
                ),
                Expanded(
                  child: ListView.builder(
                    itemCount: _todos.length,
                    itemBuilder: (_, i) {
                      final todo = _todos[i];
                      return CheckboxListTile(
                        title: Text(todo.title),
                        value: todo.completed,
                        onChanged: (_) => _toggleTodo(todo),
                      );
                    },
                  ),
                ),
              ],
            ),
    );
  }
}
