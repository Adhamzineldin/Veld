import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', '..'))

from testapp.generated.interfaces.i_food_service import IfoodService


class FoodService(IfoodService):

    def __init__(self):
        self._foods = [
            {'id': '1', 'name': 'Pizza',  'price': 12, 'tags': ['italian', 'hot']},
            {'id': '2', 'name': 'Burger', 'price': 8,  'tags': ['american', 'fast-food']},
            {'id': '3', 'name': 'Salad',  'price': 7,  'tags': ['healthy', 'cold']},
        ]

    def GetAllFoods(self, user_id):
        return {'items': self._foods, 'total': len(self._foods)}

    def AddFood(self, input):
        food = {'id': str(len(self._foods) + 1), **input}
        self._foods.append(food)
        return food
