import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', '..'))

from testapp.generated.interfaces.i_food_service import IfoodService


class FoodService(IfoodService):
    """YOUR FILE — edit freely, Veld never overwrites it."""

    def __init__(self):
        self._foods = [
            {'id': '1', 'name': 'Pizza',  'price': 12, 'tags': ['italian', 'hot'], 'type': 'meat'},
            {'id': '2', 'name': 'Burger', 'price': 8,  'tags': ['american', 'fast-food'], 'type': 'burger'},
            {'id': '3', 'name': 'Salad',  'price': 7,  'tags': ['healthy', 'cold'], 'type': 'salatat'},
        ]

    def get_all_foods(self, user_id):
        return {'items': self._foods, 'total': len(self._foods)}

    def add_food(self, input):
        food = {'id': str(len(self._foods) + 1), **input}
        self._foods.append(food)
        return food
