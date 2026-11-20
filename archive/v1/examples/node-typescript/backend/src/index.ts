import express from 'express';
import { registerUsersRoutes } from '../../generated/routes/users.routes';
import { registerTodosRoutes } from '../../generated/routes/todos.routes';
import { UsersService } from './services/UsersService';
import { TodosService } from './services/TodosService';

const app = express();
app.use(express.json());

registerUsersRoutes(app, new UsersService());
registerTodosRoutes(app, new TodosService());

const PORT = process.env.PORT ?? 3000;
app.listen(PORT, () => {
  console.log(`Server running at http://localhost:${PORT}`);
});
