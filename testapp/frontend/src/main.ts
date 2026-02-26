// Set the API base URL before any calls.
// In a real app this comes from an environment variable or build config.
process.env.VELD_API_URL = 'http://localhost:3000'

import { api } from '../../generated/client/api'
import type { User } from '../../generated/types/auth'

async function main() {
  // Login — { email, password } typed as LoginInput
  const user: User = await api.Auth.Login({
    email: 'user@example.com',
    password: 'secret'
  })
  console.log('Logged in:', user)

  // Register — { email, password, name } typed as RegisterInput
  const newUser: User = await api.Auth.Register({
    email: 'new@example.com',
    password: 'pass123',
    name: 'Alice'
  })
  console.log('Registered:', newUser)

  // GET — no body, returns User
  const me: User = await api.Auth.Me()
  console.log('Current user:', me)

  // POST without body — returns { success: boolean }
  const result = await api.Auth.Logout()
  console.log('Logged out:', result)
}

main().catch(console.error)
