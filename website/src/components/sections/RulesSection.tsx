import type { ReactNode } from 'react';
import { motion } from 'framer-motion';
import { Link } from 'react-router-dom';
import { BookOpen } from 'lucide-react';
import { highlightVeld, highlightTS } from '../SyntaxHighlighter';
import styles from './CodeExample.module.css';

// The contract is checked with `veld check` and `veld test`. The generated excerpts are what
// `veld generate` writes for it (the node server and the typescript client). It is a smaller
// version of examples/bookings in the repository.
const contract = `// The models Booking and User are left out.
enum Status {
  pending,
  paid,
  cancelled,

  flow pending -> paid
  flow pending -> cancelled
  final paid, cancelled
}

model Payment {
  amount: decimal

  rule positive: amount > 0
    message "Pay a positive amount"
}

principal User

policy ManageBooking(user: User, booking: Booking) {
  permit when booking.guestId == user.id
  permit when "staff" in user.roles
  forbid when user.suspended
    message "A suspended account cannot manage bookings"
}

@auth(bearer)
service Bookings at "/bookings" {
  post "/{id}/pay" payBooking(id: uuid, body: Payment) -> 200 Booking | AmountMismatch {
    fact booking: Booking = id
    allow ManageBooking(user, booking)
    transition booking.status: pending -> paid
    require body.amount == booking.total else AmountMismatch { expected: booking.total }
  }
}`;

const example = `example "a paid booking cannot be paid again" {
  given booking = { guestId: "00000000-0000-4000-8000-0000000000aa", status: .paid, total: 179.00 }
  as { id: "00000000-0000-4000-8000-0000000000aa", roles: [], suspended: false }
  payBooking(id: booking.id, body: { amount: 179.00 })
    -> InvalidTransition { field: "status", from: "paid", to: "paid" }
}

$ veld test
ok   a paid booking cannot be paid again
✓ 1 passed`;

const server = `// server/bookings.ts (server)
export interface BookingsService<Ctx = veld.RequestContext> {
  /** Pays for a booking. The amount must be the price. */
  payBooking(req: PayBookingRequest & PayBookingFacts & PayBookingTransition, ctx: Ctx): Promise<Booking>;
}

export interface BookingsFactLoaders {
  booking(key: string, ctx: veld.RequestContext): Promise<Booking | null | undefined>;
}`;

const client = `// bookings/client.ts (client)
// Would the server let this call past its policies and state check?
// It sends nothing: show or hide a button. The server decides.
static canPayBooking(booking: Booking, user: User): boolean { /* generated */ }

// bookings/models/bookings.ts: the same machine, to show only the moves that exist
Status.nextStates('pending');   // ['paid', 'cancelled']`;

function Block({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className={styles.codeBlock}>
      <div className={styles.title}>
        <span className={styles.dot} style={{ background: '#f85149' }} />
        <span className={styles.dot} style={{ background: '#f0883e' }} />
        <span className={styles.dot} style={{ background: 'var(--accent2)' }} />
        <span>{title}</span>
      </div>
      <pre className={styles.pre}>{children}</pre>
    </div>
  );
}

export default function RulesSection() {
  return (
    <section className={styles.section} id="rules">
      <div className={styles.container}>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <h2 className={styles.heading}>Business rules in the contract</h2>
          <p className={styles.subtitle}>
            A rule, a state machine and a policy are written once. The generated server enforces them before
            your handler runs, and the generated client checks them before it sends a request.
          </p>
        </motion.div>

        <motion.div
          className={styles.grid}
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5, delay: 0.1 }}
        >
          <Block title="You write: a rule, a flow, a policy">{highlightVeld(contract)}</Block>
          <div style={{ display: 'grid', gap: 20, alignContent: 'start' }}>
            <Block title="You write: an example that runs">{highlightVeld(example)}</Block>
            <Block title="Generated server interface">{highlightTS(server)}</Block>
            <Block title="Generated client">{highlightTS(client)}</Block>
          </div>
        </motion.div>

        <div className={styles.tryRow}>
          <Link to="/docs#rules" className={styles.tryLink}>
            <BookOpen size={16} aria-hidden="true" /> Read about rules
          </Link>
          <span className={styles.tryNote}>
            A body that breaks <code>positive</code> gets a <code>422</code> with the violation. A paid booking
            cannot be paid again. A stranger gets a <code>403</code>.
          </span>
        </div>
      </div>
    </section>
  );
}
