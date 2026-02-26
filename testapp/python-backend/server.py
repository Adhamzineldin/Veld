"""
Python Flask backend — powered by Veld-generated routes.

Usage:
    cd testapp/python-backend
    pip install flask
    python server.py
"""
import sys, os

# Make the generated package importable as `testapp.generated.*`
ROOT = os.path.join(os.path.dirname(__file__), '..', '..')
sys.path.insert(0, os.path.abspath(ROOT))

from flask import Flask
from services.auth_service import AuthService
from services.food_service import FoodService

# Import Veld-generated route registrars (zero dependency — only flask request/jsonify)
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
from generated.routes.auth_routes import register_auth_routes
from generated.routes.food_routes import register_food_routes

app = Flask(__name__)

register_auth_routes(app, AuthService())
register_food_routes(app, FoodService())

if __name__ == '__main__':
    app.run(port=3001, debug=True)
