import express from 'express'
import { authRouter } from '../../generated/routes/auth.routes'
import { AuthService } from './services/AuthService'

import { foodRouter } from '../../generated/routes/food.routes'
import {FoodService} from "./services/FoodService";


const app = express()
app.use(express.json())

// You create the router — Veld registers routes onto it.
// Express and all its types stay entirely in YOUR package.json.
authRouter(app, new AuthService())
foodRouter(app, new FoodService())

app.listen(3000, () => {
  console.log('Server running on http://localhost:3000')
})
