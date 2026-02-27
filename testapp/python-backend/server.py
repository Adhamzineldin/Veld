"""
Python Flask backend — powered by Veld-generated routes.

Usage:
    cd testapp/python-backend
    pip install flask
    python server.py
"""
import sys, os
from functools import wraps

# Make the generated package importable as `testapp.generated.*`
ROOT = os.path.join(os.path.dirname(__file__), '..', '..')
sys.path.insert(0, os.path.abspath(ROOT))

from flask import Flask, request, jsonify
from services.auth_service import AuthService
from services.food_service import FoodService

# Import Veld-generated route registrars (zero dependency — only flask request/jsonify)
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))
from generated.routes.auth_routes import register_auth_routes
from generated.routes.food_routes import register_food_routes

app = Flask(__name__)


# ── Middleware implementations ───────────────────────────────────────────────
# Veld-generated routes expect a dict of middleware functions.
# Each middleware wraps a view function (Flask decorator pattern).

def rate_limit(fn):
    """Simple rate-limit stub — replace with a real implementation."""
    @wraps(fn)
    def wrapper(*args, **kwargs):
        # TODO: plug in actual rate-limiting (e.g. Flask-Limiter)
        print(f"[RateLimit] {request.method} {request.path}")
        return fn(*args, **kwargs)
    return wrapper


def auth_guard(fn):
    """Verify the Authorization header before allowing access."""
    @wraps(fn)
    def wrapper(*args, **kwargs):
        token = request.headers.get('Authorization')
        if not token:
            return jsonify({'error': 'Unauthorized'}), 401
        # TODO: decode JWT / look up session — stub accepts any token
        return fn(*args, **kwargs)
    return wrapper


middleware = {
    'RateLimit': rate_limit,
    'AuthGuard': auth_guard,
}


# ── Register routes ──────────────────────────────────────────────────────────
register_auth_routes(app, AuthService(), middleware)
register_food_routes(app, FoodService())

if __name__ == '__main__':
    app.run(port=3001, debug=True)
