import express from "express";
import { registerUsersRoutes } from "../../generated/routes/users.routes";
import { registerTodosRoutes } from "../../generated/routes/todos.routes";
import { UsersService } from "./services/UsersService";
import { TodosService } from "./services/TodosService";

const app = express();
app.use(express.json());

// Allow requests from the Vite dev server
app.use((_req, res, next) => {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "GET,POST,PUT,DELETE,PATCH");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type");
  next();
});

const router = express.Router();

registerUsersRoutes(router, new UsersService());
registerTodosRoutes(router, new TodosService());

app.use(router);

const PORT = process.env.PORT ?? 3000;
app.listen(PORT, () => {
  console.log(`Server running on http://localhost:${PORT}`);
});
