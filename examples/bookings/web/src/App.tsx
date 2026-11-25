import { BookingsClient } from '@veld/web';
import { VeldProvider } from '@veld/web/bookings/react';
import { BookingForm } from './BookingForm';

/**
 * One client for the whole app. Same origin: Vite proxies `/bookings` to the server in development.
 * The client checks the invariants of a request before it sends it. If the server's rules change
 * after this app was built, the client stops checking and says so here: rebuild the app.
 */
const client = new BookingsClient({
  baseUrl: window.location.origin,
  onContractMismatch: (info) => console.warn(`The ${info.service} rules changed on the server. Rebuild the app.`),
});

export function App() {
  return (
    <VeldProvider clients={{ bookings: client }}>
      <h1>Book a room</h1>
      <BookingForm />
    </VeldProvider>
  );
}
