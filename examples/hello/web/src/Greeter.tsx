import { useState } from 'react';
import { NotFound } from '@veld/web';
import { useGreet } from '@veld/web/hello/react';

/** Greets whoever you type. `nobody` makes the server answer with the contract's typed NotFound. */
export function Greeter() {
  const [name, setName] = useState('');
  // A null request holds the call back until there is a name.
  const greeting = useGreet(name === '' ? null : { name });
  return (
    <section>
      <label>
        Name <input value={name} onChange={(e) => setName(e.target.value)} />
      </label>
      {greeting.loading && <p role="status">Loading...</p>}
      {greeting.data && <p role="status">{greeting.data.message}</p>}
      {NotFound.matches(greeting.error) && <p role="alert">Nobody called {name} was found.</p>}
    </section>
  );
}
