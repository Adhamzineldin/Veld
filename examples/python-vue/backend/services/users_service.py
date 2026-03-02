import uuid
from generated.interfaces.i_users_service import IUsersService

_store = [
    {"id": "1", "name": "Alice", "email": "alice@example.com"},
    {"id": "2", "name": "Bob",   "email": "bob@example.com"},
]


class UsersService(IUsersService):
    def list_users(self):
        return list(_store)

    def get_user(self, id: str):
        user = next((u for u in _store if u["id"] == id), None)
        if user is None:
            raise LookupError(f"User {id} not found")
        return user

    def create_user(self, input):
        user = {"id": str(uuid.uuid4()), **input}
        _store.append(user)
        return user

    def delete_user(self, id: str):
        idx = next((i for i, u in enumerate(_store) if u["id"] == id), None)
        if idx is None:
            raise LookupError(f"User {id} not found")
        _store.pop(idx)
