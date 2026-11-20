# hello

One service, one endpoint, a Node server and a TypeScript client.

```
contracts/hello.veld        the contract
veld.toml                   the project: two targets
server/src/generated/       generated Node server (committed)
web/src/generated/          generated TypeScript client (committed)
```

## Commands

```bash
veld check                  # compile the contracts
veld generate               # write server/src/generated and web/src/generated
veld generate --dry-run     # nothing is written; shows what would change
veld generate web           # generate only the target named "web"
```

## Using the generated code

Server: implement the generated `HelloService` interface and mount the handler with an adapter.

```ts
import { createHelloHandler, toExpress, NotFound } from './server/src/generated';
import express from 'express';

const handler = createHelloHandler({
  async greet({ name }) {
    if (name === 'nobody') throw new NotFound({});
    return { message: `Hello, ${name}!` };
  },
}, {});

express().use(toExpress(handler)).listen(3000);
```

Client:

```ts
import { HelloClient } from './web/src/generated';

const hello = new HelloClient({ baseUrl: 'http://localhost:3000' });
console.log(await hello.greet({ name: 'Ada' }));   // { message: 'Hello, Ada!' }
```

The generated code has no runtime dependencies. See `spec/targets/typescript.md` for the exact API.
