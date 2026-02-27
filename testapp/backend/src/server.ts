import express from 'express'
import { authRouter } from '../../generated/routes/auth.routes'
import type { Middleware as AuthMiddleware } from '../../generated/routes/auth.routes'
import { AuthService } from './services/AuthService'

import { foodRouter } from '../../generated/routes/food.routes'
import { FoodService } from './services/FoodService'

const app = express()
app.use(express.json())

// ── Middleware implementations ──────────────────────────────────────────────
// These are the functions your contract declares. Veld generates a typed
// Middleware interface so TypeScript tells you exactly what to supply.

const authMiddleware: AuthMiddleware = {
  RateLimit: (req, res, next) => {
    // TODO: plug in your rate-limiting logic (e.g. express-rate-limit)
    console.log(`[RateLimit] ${req.method} ${req.path}`)
    next()
  },
  AuthGuard: (req, res, next) => {
    // TODO: verify JWT / session token and attach req.user
    const token = req.headers.authorization
    if (!token) {
      res.status(401).json({ error: 'Unauthorized' })
      return
    }
    // Stub: accept any token for demo purposes
    ;(req as any).user = { id: '1' }
    next()
  },
}

// ── Register routes ─────────────────────────────────────────────────────────
// Veld-generated routers wire up every endpoint declared in the contract.
// Modules that use middleware require the third argument.
authRouter(app, new AuthService(), authMiddleware)
foodRouter(app, new FoodService())

app.listen(3000, () => {
  console.log('Server running on http://localhost:3000')
})
