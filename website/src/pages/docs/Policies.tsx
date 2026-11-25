import { CodeBlock, Section, P, Info, List } from './shared';

export default function Policies() {
  return (
    <Section id="rules-policies" title="Policies: who may call">
      <P>
        A policy says who may do something. It is written once and used by every endpoint that needs it.
        The contract also says how every endpoint is authenticated: <code>@auth(bearer)</code> on the service, and{' '}
        <code>@auth(none)</code> on an endpoint that is public.
      </P>
      <CodeBlock title="examples/bookings/contracts/bookings.veld" lang="veld">{`model User {
  id: uuid
  name: string
  roles: string[]
  suspended: bool
}

/// The caller of every endpoint that is not public. In an endpoint block it is called \`user\`.
principal User

/// May this caller manage this booking? The guest who made it, and the staff. A suspended account never.
policy ManageBooking(user: User, booking: Booking) {
  permit when booking.guestId == user.id
  permit when "staff" in user.roles
  forbid when user.suspended
    message "A suspended account cannot manage bookings"
}

@auth(bearer)
service Bookings at "/bookings" {
  @auth(none)
  get "/rooms" listRooms() -> Room[]

  get "/{id}" getBooking(id: uuid) -> Booking {
    fact booking: Booking = id
    allow ManageBooking(user, booking)
  }
  // ...
}`}</CodeBlock>
      <List>
        <li><code>principal User</code> says what a caller is. In an endpoint block the caller is called <code>user</code>.</li>
        <li>A policy permits when at least one <code>permit</code> is true and no <code>forbid</code> is. A <code>forbid</code> always wins. A policy with no <code>permit</code> permits nobody (<code>W0940</code>).</li>
        <li>
          <code>allow</code> runs after the request has been validated and the facts are loaded, and before the
          state check and every <code>require</code>. A denial is <code>403 Forbidden</code>, with the message of the{' '}
          <code>forbid</code> that was true, or <code>not permitted</code>. An endpoint with an <code>allow</code> declares{' '}
          <code>Forbidden</code> without you listing it.
        </li>
        <li>A record that does not exist is <code>404</code> before the policy is asked.</li>
      </List>

      <P>The examples show the three kinds of answer, and the order.</P>
      <CodeBlock title="examples/bookings/contracts/bookings.veld" lang="veld">{`example "another guest does not see it" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa" }
  as { id: "00000000-0000-4000-8000-0000000000bb", roles: [], suspended: false }
  getBooking(id: booking.id) -> Forbidden { message: "not permitted" }
}

example "forbid beats permit: suspended staff is refused too" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa" }
  as { id: "00000000-0000-4000-8000-0000000000cc", roles: ["staff"], suspended: true }
  getBooking(id: booking.id) -> Forbidden { message: "A suspended account cannot manage bookings" }
}

example "the policy is asked before the transition" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa", status: .cancelled, total: 179.00 }
  as { id: "00000000-0000-4000-8000-0000000000bb", roles: [], suspended: false }
  payBooking(id: booking.id, body: { amount: 179.00 }) -> Forbidden
}`}</CodeBlock>

      <P>
        <strong>The auth hook</strong> returns the caller, with the generated type (<code>Promise&lt;User | null&gt;</code>;{' '}
        <code>null</code> is a <code>401</code>). In the example it is a lookup in a table of demo tokens. A handler that
        needs to know the caller reads <code>ctx.principal</code>. It is typed <code>unknown</code>, so the example
        narrows it once.
      </P>
      <CodeBlock title="examples/bookings/server/src/service.ts" lang="ts">{`export async function authenticate(token: string): Promise<User | null> {
  return USERS[token] ?? null;
}

function caller(ctx: veld.RequestContext): User {
  return ctx.principal as User;
}`}</CodeBlock>

      <P>
        <strong>The client gets the policy too.</strong> <code>canPayBooking</code> answers whether the server would let
        this call past its policies and its state check, so the app can hide a button the server would refuse. It
        never sends anything, and the server still decides. Its arguments are the facts, then the principal, then
        the parameters. The booking list of the web app shows a button only when the state machine has the move and
        the helper agrees.
      </P>
      <CodeBlock title="examples/bookings/web/src/BookingList.tsx (excerpt)" lang="ts">{`{Status.nextStates(b.status).map((next) => {
  const move = moves[next];
  if (move === undefined || !move.can(b)) return null;
  return (
    <button key={next} type="button" onClick={() => void act(b, next)}>
      {move.label}
    </button>
  );
})}

// with, for example:  can: (b) => BookingsClient.canPayBooking(b, me)`}</CodeBlock>
      <P>
        When a call is refused anyway (the booking moved in another window), show the server's own words: the{' '}
        <code>message</code> of a <code>Forbidden</code>, the two states of an <code>InvalidTransition</code>.
      </P>

      <Info>
        <strong>Say who may call every endpoint.</strong> <code>veld check</code> warns about an endpoint with no{' '}
        <code>@auth</code> at all (lint <code>L0014</code>, <code>auth-required</code>). Write <code>@auth(none)</code> when it is
        meant to be public. A policy that the contract uses in an <code>allow</code> is generated as a function in
        every target, clients included.
      </Info>
    </Section>
  );
}
