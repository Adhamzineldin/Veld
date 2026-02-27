import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', '..'))

from testapp.generated.interfaces.i_auth_service import IAuthService


class AuthService(IAuthService):
    """YOUR FILE — edit freely, Veld never overwrites it."""

    def login(self, input):
        # TODO: verify credentials against your database
        return {'id': '1', 'email': input['email'], 'name': 'Demo User'}

    def register(self, input):
        # TODO: hash password and persist to database
        return {'id': '2', 'email': input['email'], 'name': input['name']}

    def me(self, user_id):
        # TODO: fetch user from database by user_id
        return {'id': user_id or '1', 'email': 'user@example.com', 'name': 'Demo User'}

    def logout(self, user_id):
        # TODO: invalidate session or token
        return {'success': True}
