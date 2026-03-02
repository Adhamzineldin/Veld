import uuid
from generated.interfaces.i_todos_service import ITodosService

_store = [
    {"id": "1", "title": "Buy groceries", "completed": False, "userId": "1"},
    {"id": "2", "title": "Read docs",     "completed": True,  "userId": "2"},
]


class TodosService(ITodosService):
    def list_todos(self):
        return list(_store)

    def get_todo(self, id: str):
        todo = next((t for t in _store if t["id"] == id), None)
        if todo is None:
            raise LookupError(f"Todo {id} not found")
        return todo

    def create_todo(self, input):
        todo = {"id": str(uuid.uuid4()), "completed": False, **input}
        _store.append(todo)
        return todo

    def update_todo(self, id: str, input):
        todo = next((t for t in _store if t["id"] == id), None)
        if todo is None:
            raise LookupError(f"Todo {id} not found")
        if input.get("title") is not None:
            todo["title"] = input["title"]
        if input.get("completed") is not None:
            todo["completed"] = input["completed"]
        return todo

    def delete_todo(self, id: str):
        idx = next((i for i, t in enumerate(_store) if t["id"] == id), None)
        if idx is None:
            raise LookupError(f"Todo {id} not found")
        _store.pop(idx)
