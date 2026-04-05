import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from flask import Flask
from generated.routes.transactions_routes import register_transactions_routes
from src.services.transactions_service import TransactionsService

app = Flask(__name__)

@app.after_request
def cors(response):
    response.headers["Access-Control-Allow-Origin"] = "*"
    response.headers["Access-Control-Allow-Methods"] = "GET,POST,PUT,DELETE,PATCH,OPTIONS"
    response.headers["Access-Control-Allow-Headers"] = "Content-Type,Authorization"
    return response

@app.route("/", defaults={"path": ""}, methods=["OPTIONS"])
@app.route("/<path:path>", methods=["OPTIONS"])
def options_handler(path):
    return "", 204

register_transactions_routes(app, TransactionsService())

if __name__ == "__main__":
    port = int(os.environ.get("PORT", 3003))
    print(f"[transaction-service] http://localhost:{port}")
    app.run(port=port, debug=True)
