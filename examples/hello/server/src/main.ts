// The hello server: implement the generated `HelloService` and mount its handler on Express.
// `@veld/api` is the generated package (see `veld setup`), not a relative path.
import express from 'express';
import { NotFound, createHelloHandler, toExpress } from '@veld/api';
import type { GreetRequest, HelloService } from '@veld/api';

/** The business logic. Throw a generated error to answer with its HTTP status. */
export const hello: HelloService = {
  async greet({ name }: GreetRequest) {
    if (name.toLowerCase() === 'nobody') {
      throw new NotFound({ resource: name });
    }
    return { message: `Hello, ${name}!` };
  },
};

const app = express();
app.use(toExpress(createHelloHandler(hello)));

const port = Number(process.env.PORT ?? 3000);
app.listen(port, '127.0.0.1', () => {
  console.log(`hello server listening on http://127.0.0.1:${port}`);
});
