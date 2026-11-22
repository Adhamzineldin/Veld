import sys
import os

# Make the generated package importable from backend/
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from flask import Flask
from generated.routes.users_routes import register_users_routes
from generated.routes.todos_routes import register_todos_routes
from services.users_service import UsersService
from services.todos_service import TodosService

app = Flask(__name__)


@app.after_request
def cors(response):
    """Allow requests from the Vite dev server."""
    response.headers["Access-Control-Allow-Origin"] = "*"
    response.headers["Access-Control-Allow-Methods"] = "GET,POST,PUT,DELETE,PATCH,OPTIONS"
    response.headers["Access-Control-Allow-Headers"] = "Content-Type"
    return response


@app.route("/", defaults={"path": ""}, methods=["OPTIONS"])
@app.route("/<path:path>", methods=["OPTIONS"])
def options_handler(path):
    return "", 204


register_users_routes(app, UsersService())
register_todos_routes(app, TodosService())

if __name__ == "__main__":
    port = int(os.environ.get("PORT", 5000))
    print(f"Server running on http://localhost:{port}")
    app.run(port=port, debug=True)
