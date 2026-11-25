import { CodeBlock, Section, Sub, P, Info, List, Table } from './shared';

export default function EndpointRules() {
  return (
    <>
      <Section id="rules-endpoint" title="Endpoint rules: fact, require, ensure">
        <P>
          Some rules need more than the request. Booking a room depends on the room. An endpoint can have a
          block that loads that data and states its rules. The block goes after the response, in braces.
        </P>
        <CodeBlock title="examples/bookings/contracts/bookings.veld" lang="veld">{`post "/" createBooking(body: NewBooking) -> 201 Booking | RoomFull | RoomClosed {
  fact room: Room = body.roomId
  allow MayBook(user)
  require not room.closed else RoomClosed
  require body.guests <= room.capacity else RoomFull { capacity: room.capacity }
  require notInPast: body.start >= now
    message "A booking cannot start in the past"
  ensure keepsRoom: result.roomId == body.roomId
  ensure keepsGuests: result.guests == body.guests
  ensure belongsToCaller: result.guestId == user.id
  ensure startsPending: result.status == .pending
}`}</CodeBlock>
        <P>
          <code>allow</code> says who may call. It comes on the <a href="#rules-policies">policies page</a>.
        </P>
        <List>
          <li>
            <code>fact room: Room = body.roomId</code> says: load a <code>Room</code> with this key. If there is
            none, the answer is <code>404 NotFound</code> and its <code>resource</code> is <code>"room"</code>. Facts load in
            the order written, and are handed to your handler, so it does not load them a second time.
          </li>
          <li>
            <code>require ... else Error</code> is a precondition. If it fails, the server answers the declared
            error. <code>RoomFull {'{ capacity: room.capacity }'}</code> fills the error's data from the fact. The
            first failing <code>require</code> wins, in the order written.
          </li>
          <li>
            <code>require</code> without <code>else</code> answers <code>422 ValidationError</code> with a violation, like an
            invariant. A name before the colon (<code>notInPast</code>) and a <code>message</code> are optional.
          </li>
          <li>
            <code>now</code> is the time of the request, read once, so a rule that uses it is testable.
          </li>
          <li>
            <code>ensure</code> is a promise about what your handler returns. If your code breaks it, that is a bug
            on the server, so the caller gets <code>500 Internal</code> with no detail. In the example the handler
            may not change the number of guests, and the booking it returns must belong to the caller and start{' '}
            <code>pending</code>. <code>result</code> is what the handler returned. <code>old(x)</code> in an{' '}
            <code>ensure</code> is the value <code>x</code> had before the handler ran. A fact read in an{' '}
            <code>ensure</code> must be inside <code>old(...)</code>.
          </li>
        </List>
        <Info>
          The order of the checks is fixed: invalid parameters, a missing fact, a denied <code>allow</code>, the
          transition guard, then each <code>require</code>. <code>ensure</code> runs after your handler. The same order
          holds in every generator.
        </Info>

        <Sub id="rules-handler">What you write after generating</Sub>
        <P>Two things, and no checks.</P>
        <P>
          <strong>A loader per fact.</strong> The generated <code>BookingsFactLoaders</code> has one method per fact.
          A loader that finds nothing returns <code>undefined</code> or <code>null</code>, and the server answers{' '}
          <code>404</code>.
        </P>
        <CodeBlock title="examples/bookings/server/src/service.ts" lang="ts">{`export function createLoaders(store: Store): BookingsFactLoaders {
  return {
    async room(key) {
      return store.rooms.get(key);   // undefined or null: the server answers 404
    },
    async booking(key) {
      return store.bookings.get(key);
    },
  };
}`}</CodeBlock>
        <P>
          <strong>The handler.</strong> It receives the request and the loaded facts, already checked.
        </P>
        <CodeBlock title="examples/bookings/server/src/service.ts (excerpt)" lang="ts">{`async createBooking({ body, room }, ctx) {
  const booking: Booking = {
    id: randomUUID(),
    guestId: caller(ctx).id,
    roomId: body.roomId,
    guests: body.guests,
    start: body.start,
    end: body.end,
    total: price(room.nightly, body.start, body.end),
    status: 'pending',
  };
  store.bookings.set(booking.id, booking);
  return booking;
},`}</CodeBlock>
        <P>The server mounts it with the loaders and the auth hook.</P>
        <CodeBlock title="examples/bookings/server/src/main.ts (excerpt)" lang="ts">{`createBookingsHandler(createService(store), {
  facts: createLoaders(store),
  auth: { bearer: authenticate },
})`}</CodeBlock>
        <P>
          This handler does not test the dates, the party size or whether the room is closed. The contract
          already did. If you find yourself writing such a check in a handler, move it into the contract.
        </P>
        <Info>
          Facts are trusted values of your application: they are not validated. A <code>fact</code> with a{' '}
          <code>?</code> (<code>fact x?: T = key</code>) may be absent, and a rule that reads it needs <code>has(x)</code>.
          Endpoint rules are not supported on WebSocket endpoints: every server target stops with{' '}
          <code>E0801</code>.
        </Info>
      </Section>

      <Section id="rules-output" title="Output mode and the violation hook">
        <P>
          The server also checks what <strong>it</strong> sends: the invariants of the returned value and the{' '}
          <code>ensure</code> lines. The default, <code>strict</code>, answers <code>500 Internal</code> with no detail when
          your code returns something that breaks a rule. Choose another mode when you create the handler.
        </P>
        <CodeBlock title="Handler options" lang="ts">{`createBookingsHandler(service, {
  facts: loaders,
  rules: { output: 'audit' },      // 'strict' (default), 'audit' or 'off'
  onRuleViolation: (event) => log.warn(event.rule, event.outcome, event.position),
});`}</CodeBlock>
        <Table
          head={['Mode', 'A broken rule in your own answer']}
          rows={[
            [<code>strict</code>, <>The caller gets <code>500 Internal</code>. The hook is called.</>],
            [<code>audit</code>, 'The answer is sent unchanged. The hook is called. Use it to find violations in existing data before you enforce.'],
            [<code>off</code>, 'Not checked.'],
          ]}
        />
        <P>
          The hook <code>onRuleViolation</code> also runs for rejected requests (<code>position: 'input'</code>). It
          is for logs and metrics. An exception inside it never changes the response.
        </P>
        <Info>
          The output mode is an option of the handler. It is set in code, not in <code>veld.toml</code>.
        </Info>
      </Section>
    </>
  );
}
