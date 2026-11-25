import { CodeBlock, Section, P, Info, Warn, List } from './shared';

export default function StateMachines() {
  return (
    <Section id="rules-states" title="State machines: which moves are legal">
      <P>
        A cancelled booking must not become paid. Say which moves exist on the enum, and which move an
        endpoint makes.
      </P>
      <CodeBlock title="examples/bookings/contracts/bookings.veld" lang="veld">{`enum Status {
  pending,
  paid,
  checkedIn,
  cancelled,
  refunded,

  flow pending -> paid -> checkedIn
  flow pending -> cancelled
  flow paid -> cancelled
  flow paid -> refunded
  final checkedIn, cancelled, refunded
}

/// Pays for a booking. The amount must be the price.
post "/{id}/pay" payBooking(id: uuid, body: Payment) -> 200 Booking | AmountMismatch {
  fact booking: Booking = id
  allow ManageBooking(user, booking)
  transition booking.status: pending -> paid
  require body.amount == booking.total else AmountMismatch { expected: booking.total }
}

/// Cancels a booking that has not been used.
post "/{id}/cancel" cancelBooking(id: uuid) -> 200 Booking {
  fact booking: Booking = id
  allow ManageBooking(user, booking)
  transition booking.status: pending, paid -> cancelled
}`}</CodeBlock>
      <List>
        <li>
          <code>flow</code> lists the legal moves. A chain (<code>pending -&gt; paid -&gt; checkedIn</code>) is two moves.
          A <code>final</code> state has no way out. <code>veld check</code> warns about a state you can get stuck in
          (<code>W0931</code>), a flow declared twice (<code>W0930</code>) and a variant that takes part in no flow (
          <code>W0932</code>).
        </li>
        <li>
          <code>transition booking.status: pending -&gt; paid</code> says this endpoint moves the booking from{' '}
          <code>pending</code> to <code>paid</code>. A list of sources (<code>pending, paid -&gt; cancelled</code>) means any
          of them. Each move must be a flow of the machine (<code>E0933</code>).
        </li>
        <li>
          If the loaded booking is in any other state, the server answers <code>409 InvalidTransition</code> with{' '}
          <code>field</code>, <code>from</code> and <code>to</code>, before your handler runs. An endpoint with a{' '}
          <code>transition</code> declares <code>InvalidTransition</code> without you listing it.
        </li>
        <li>When the endpoint returns the booking, the server also checks that your handler really moved it to <code>paid</code>.</li>
        <li>
          The check comes after the policies and before every <code>require</code>. A cancelled booking that is paid
          with the wrong amount is <code>InvalidTransition</code>, not <code>AmountMismatch</code>.
        </li>
      </List>

      <Warn>
        <strong>This is a guard, not a lock.</strong> Two requests can pass the check at the same moment. So
        the handler receives the move as data and must write only if the row is still where the check found
        it.
      </Warn>
      <P>
        This is <code>move</code> in <code>examples/bookings/server/src/service.ts</code>. The store is a{' '}
        <code>Map</code>, so the compare and the write cannot be separated there, but the shape is the one a
        database needs (<code>UPDATE ... WHERE id = $id AND status = $from</code>).
      </P>
      <CodeBlock title="examples/bookings/server/src/service.ts (excerpt)" lang="ts">{`function move(store: Store, id: string, transition: veld.Transition<Status>): Booking {
  const current = store.bookings.get(id);
  if (current === undefined) throw new NotFound({ resource: 'booking' });
  if (current.status !== transition.from) {
    throw new InvalidTransition({ field: transition.field, from: current.status, to: transition.to });
  }
  const moved: Booking = { ...current, status: transition.to };
  store.bookings.set(id, moved);
  return moved;
}

// the handler:
async payBooking({ id, transition }) {
  return move(store, id, transition);
},`}</CodeBlock>
      <P>
        <code>transition</code> is <code>{"{ field: 'status', from: 'pending', to: 'paid' }"}</code>, typed with the enum.
        The handler does not check the amount or who the caller is: the contract did.
      </P>

      <P>Clients get the same machine, to show only the buttons the server would accept.</P>
      <CodeBlock title="TypeScript" lang="ts">{`Status.nextStates('pending');   // ['paid', 'cancelled']
Status.isFinal('refunded');     // true`}</CodeBlock>
      <Info>
        Java has <code>nextStates()</code> and <code>isFinal()</code> on the enum, and Rust has <code>next_states()</code>{' '}
        and <code>is_final()</code>. For a button that also depends on who the caller is, see the{' '}
        <code>can&lt;Endpoint&gt;</code> helpers on the <a href="#rules-policies">policies page</a>.
      </Info>

      <P>Examples state the legal and the illegal moves, and the order of the checks.</P>
      <CodeBlock title="examples/bookings/contracts/bookings.veld" lang="veld">{`example "a paid booking cannot be paid again" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa", status: .paid, total: 179.00 }
  as { id: "00000000-0000-4000-8000-0000000000aa", roles: [], suspended: false }
  payBooking(id: booking.id, body: { amount: 179.00 })
    -> InvalidTransition { field: "status", from: "paid", to: "paid" }
}

example "the transition is checked before the amount" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa", status: .cancelled, total: 179.00 }
  as { id: "00000000-0000-4000-8000-0000000000aa", roles: [], suspended: false }
  payBooking(id: booking.id, body: { amount: 1.00 }) -> InvalidTransition { from: "cancelled" }
}`}</CodeBlock>
    </Section>
  );
}
