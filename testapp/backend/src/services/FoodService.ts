import type { Food, FoodList, CreateFoodInput } from '../../../generated/types/food'
import type { IfoodService } from '../../../generated/interfaces/IfoodService'

export class FoodService implements IfoodService {

  private foods: Food[] = [
    { id: '1', name: 'Pizza',  price: 12, tags: ['italian', 'hot'] },
    { id: '2', name: 'Burger', price: 8,  tags: ['american', 'fast-food'] },
    { id: '3', name: 'Salad',  price: 7,  tags: ['healthy', 'cold'] },
  ]

  async GetAllFoods(): Promise<FoodList> {
    return { items: this.foods, total: this.foods.length }
  }

  async AddFood(input: CreateFoodInput): Promise<Food> {
    const food: Food = { id: String(this.foods.length + 1), ...input }
    this.foods.push(food)
    return food
  }
}
