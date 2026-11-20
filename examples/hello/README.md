# hello

One service, one endpoint. A Node server written by hand against the generated code, and a small
React app that uses the generated hooks. Everything the hand-written code imports comes from the
generated packages `@veld/api` (server) and `@veld/web` (client); there are no relative imports
into `src/generated`.

```
contracts/hello.veld        the contract
veld.toml                   the project: two targets (api, web)
server/                     Express app, hand-written (src/main.ts)
  src/generated/            generated, committed  ->  package @veld/api
web/                        Vite + React 18 app, hand-written (src/*.tsx)
  src/generated/            generated, committed  ->  package @veld/web
```

## 1. The contract

```veld
/// A greeting for one person.
model Greeting {
  message: string
}

/// Says hello.
service Hello at "/hello" {
  /// Greets the person with the given name.
  get "/{name}" greet(name: string) -> Greeting | NotFound
}
```

`NotFound` is a prelude error: it needs no declaration.

## 2. Generate and wire

`veld.toml` has two targets. The client one has `framework = "react"`, which adds the hooks.

```bash
veld generate        # writes server/src/generated and web/src/generated
veld setup           # adds "@veld/api" and "@veld/web" to the package.json files
(cd server && npm install)
(cd web && npm install)
```

Each generated folder contains a `package.json` (name `@veld/<target name>`, private, no
dependencies; the React target declares `react` as a peer dependency). `veld setup` adds
`"@veld/api": "file:src/generated"` to `server/package.json` and `"@veld/web": "file:src/generated"` to
`web/package.json`, nothing else. After `npm install` the packages resolve like any other, for
`tsc`, Node (through `tsx`), Vite and test runners alike.

## 3. The server

`server/src/main.ts` implements the generated `HelloService` and mounts the handler on Express:

```ts
import express from 'express';
import { NotFound, createHelloHandler, toExpress } from '@veld/api';
import type { GreetRequest, HelloService } from '@veld/api';

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
```

Throwing a generated error answers with its status (here `404`, as `application/problem+json`).

## 4. The frontend

`web/src/App.tsx` creates one client and provides it to the hooks:

```tsx
import { HelloClient } from '@veld/web';
import { VeldProvider } from '@veld/web/hello/react';

const client = new HelloClient({ baseUrl: window.location.origin });

export function App() {
  return (
    <VeldProvider clients={{ hello: client }}>
      <h1>Hello</h1>
      <Greeter />
    </VeldProvider>
  );
}
```

`web/src/Greeter.tsx` greets by name and shows the typed `NotFound`:

```tsx
import { NotFound } from '@veld/web';
import { useGreet } from '@veld/web/hello/react';

const greeting = useGreet(name === '' ? null : { name });   // null holds the request back
// greeting.data?.message, greeting.loading, greeting.error
{NotFound.matches(greeting.error) && <p role="alert">Nobody called {name} was found.</p>}
```

The framework files are not part of the `@veld/web` barrel, so the hooks are imported from
`@veld/web/hello/react`.

## 5. Run it

```bash
(cd server && npm start)            # http://127.0.0.1:3000   (PORT overrides)
(cd web && npm run dev)             # http://localhost:5173, proxies /hello to the server
```

`npm run build` type-checks and bundles each app. `cd web && VELD_HELLO_URL=http://127.0.0.1:3000 npm test`
renders `<Greeter>` with `react-test-renderer` against the running server.

`cargo test -p veld --test examples_e2e` does all of this automatically: regenerate (nothing may
change), `npm ci`, `tsc --noEmit`, boot the server, call it over HTTP and run the render test.
