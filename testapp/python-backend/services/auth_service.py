import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', '..'))

from testapp.generated.interfaces.i_auth_service import IAuthService


class AuthService(IAuthService):

    def Login(self, input):
        return {'id': '1', 'email': input['email'], 'name': 'Demo User'}

    def Register(self, input):
        return {'id': '2', 'email': input['email'], 'name': input['name']}

    def Me(self, user_id):
        return {'id': user_id or '1', 'email': 'user@example.com', 'name': 'Demo User'}

    def Logout(self, user_id):
        return {'success': True}
