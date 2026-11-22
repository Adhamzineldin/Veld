import { HelloClient } from '@veld/web';
import { VeldProvider } from '@veld/web/hello/react';
import { Greeter } from './Greeter';

/** One client for the whole app. Same origin: Vite proxies `/hello` to the server in development. */
const client = new HelloClient({ baseUrl: window.location.origin });

export function App() {
  return (
    <VeldProvider clients={{ hello: client }}>
      <h1>Hello</h1>
      <Greeter />
    </VeldProvider>
  );
}
