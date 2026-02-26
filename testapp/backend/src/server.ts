import express from 'express'
import { authRouter } from '../../generated/routes/auth.routes'
import { AuthService } from './services/AuthService'

const app = express()
app.use(express.json())

// You create the router — Veld registers routes onto it.
// Express and all its types stay entirely in YOUR package.json.
authRouter(app, new AuthService())

app.listen(3000, () => {
  console.log('Server running on http://localhost:3000')
})
