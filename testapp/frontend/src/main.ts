// Set the API base URL before any calls.
// In a real app this comes from an environment variable or build config.
process.env.VELD_API_URL = 'http://localhost:3000'

import { api } from '../../generated/client/api'
import type { User } from '../../generated/types/auth'
import type { Food, FoodList } from '../../generated/types/food'


async function main() {
  // ── Public endpoints (no middleware) ──────────────────────────────────────

  // GET all foods — no auth required
  const foods: FoodList = await api.food.GetAllFoods()
  console.log('Foods:', foods)

  // POST add a food item — no auth required
  const newFood: Food = await api.food.AddFood({
    name: 'Pizza',
    price: 12,
    tags: ['italian', 'hot'],
    type: 'meat',
  })
  console.log('Added food:', newFood)

  // ── Auth endpoints (RateLimit middleware on login) ────────────────────────

  // Login — protected by RateLimit middleware on the server
  const user: User = await api.Auth.Login({
    email: 'user@example.com',
    password: 'secret',
  })
  console.log('Logged in:', user)

  // Register — no middleware on this endpoint
  const newUser: User = await api.Auth.Register({
    email: 'new@example.com',
    password: 'pass123',
    name: 'Alice',
  })
  console.log('Registered:', newUser)

  // ── Protected endpoints (AuthGuard middleware) ───────────────────────────
  // These will return 401 unless the server gets an Authorization header.
  // In a real app you'd store the token from Login and attach it to requests.

  try {
    const me: User = await api.Auth.Me()
    console.log('Current user:', me)
  } catch (err) {
    console.log('Me() rejected (expected — AuthGuard blocks without token):', (err as Error).message)
  }

  try {
    const result = await api.Auth.Logout()
    console.log('Logged out:', result)
  } catch (err) {
    console.log('Logout() rejected (expected — AuthGuard blocks without token):', (err as Error).message)
  }
}

main().catch(console.error)
