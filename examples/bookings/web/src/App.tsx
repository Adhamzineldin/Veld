import { useMemo, useState } from 'react';
import { BookingsClient } from '@veld/web';
import { VeldProvider, useWhoAmI } from '@veld/web/bookings/react';
import { BookingForm } from './BookingForm';
import { BookingList } from './BookingList';
import { describe } from './describe';

/** The demo accounts of the server (`server/src/service.ts`). A real app gets a token from a login. */
const ACCOUNTS = [
  { token: 'token-ann', label: 'Ann (guest)' },
  { token: 'token-bob', label: 'Bob (guest)' },
  { token: 'token-sam', label: 'Sam (staff)' },
  { token: 'token-eve', label: 'Eve (suspended)' },
];

export function App() {
  const [token, setToken] = useState(ACCOUNTS[0]?.token ?? '');

  // One client per account. Same origin: Vite proxies `/bookings` to the server in development.
  // The client checks the invariants of a request before it sends it. If the server's rules change
  // after this app was built, the client stops checking and says so here: rebuild the app.
  const client = useMemo(
    () =>
      new BookingsClient({
        baseUrl: window.location.origin,
        auth: { bearer: () => token },
        onContractMismatch: (info) => console.warn(`The ${info.service} rules changed on the server. Rebuild the app.`),
      }),
    [token],
  );

  return (
    <VeldProvider clients={{ bookings: client }}>
      <h1>Hotel bookings</h1>
      <label>
        Sign in as
        <select value={token} onChange={(e) => setToken(e.target.value)}>
          {ACCOUNTS.map((a) => (
            <option key={a.token} value={a.token}>
              {a.label}
            </option>
          ))}
        </select>
      </label>
      {/* `key` starts a fresh session (and fresh state) when the account changes. */}
      <Session key={token} />
    </VeldProvider>
  );
}

/** Everything a signed-in account sees. The caller comes from the server, typed as the contract's `User`. */
function Session() {
  const me = useWhoAmI();
  const [stamp, setStamp] = useState(0); // a new booking: show the list again from the server
  if (me.error !== undefined && me.error !== null) {
    return (
      <p role="alert" className="problem">
        {describe(me.error)}
      </p>
    );
  }
  if (me.data === undefined) return <p>Signing in...</p>;
  return (
    <>
      <p role="status">Signed in as {me.data.name}.</p>
      <h2>Book a room</h2>
      <BookingForm me={me.data} onBooked={() => setStamp((n) => n + 1)} />
      <h2>Bookings</h2>
      <BookingList key={stamp} me={me.data} />
    </>
  );
}
