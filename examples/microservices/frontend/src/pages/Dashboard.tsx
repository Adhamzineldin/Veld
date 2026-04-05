import type { Account, Notification } from '@veld/generated/types';
import type { VeldApiError } from '@veld/client/_internal';

// All hooks are Veld-generated from the contracts.
// useListAccounts → GET /api/accounts/
// useGetSummary   → GET /api/accounts/summary
// useListNotifications → GET /api/notifications/
import { useListAccounts, useGetSummary } from '@veld/hooks/accountsHooks';
import { useListNotifications, useMarkAsRead } from '@veld/hooks/notificationsHooks';
import { useListLoans } from '@veld/hooks/lendingHooks';

export default function Dashboard() {
  const { data: accounts = [], isLoading: accsLoading, error: accsError } = useListAccounts();
  const { data: summary }                                                  = useGetSummary();
  const { data: notifs = [], isLoading: notifsLoading }                   = useListNotifications();
  const { data: loans  = [] }                                              = useListLoans();

  const markReadMutation = useMarkAsRead({
    onError: (err: VeldApiError) => console.error('markAsRead failed:', err.message),
  });

  if (accsLoading) return <p>Loading…</p>;
  if (accsError) return <p style={{ color: 'red' }}>Error (HTTP {accsError.status}): {accsError.message}</p>;

  return (
    <div>
      {/* ── Portfolio Summary ───────────────────────────────────── */}
      {summary && (
        <section style={{ background: '#f5f5f5', padding: 16, borderRadius: 8, marginBottom: 24 }}>
          <h2 style={{ margin: '0 0 4px' }}>Total Balance</h2>
          <p style={{ fontSize: 28, margin: 0 }}>
            EUR {summary.totalBalance.toLocaleString('de-DE', { minimumFractionDigits: 2 })}
          </p>
          <small>{summary.accountCount} accounts</small>
        </section>
      )}

      {/* ── Accounts ────────────────────────────────────────────── */}
      <section style={{ marginBottom: 24 }}>
        <h2>Accounts</h2>
        {accounts.map((acc: Account) => (
          <div key={acc.id} style={{
            display: 'flex', justifyContent: 'space-between',
            padding: '12px 16px', border: '1px solid #ddd',
            borderRadius: 6, marginBottom: 8,
          }}>
            <div>
              <strong>{acc.name}</strong>
              <span style={{ marginLeft: 8, color: '#666', fontSize: 12 }}>[{acc.type}]</span>
            </div>
            <div>
              <strong>{acc.currency} {acc.balance.toFixed(2)}</strong>
              <span style={{ marginLeft: 8, color: acc.status === 'active' ? 'green' : 'orange', fontSize: 12 }}>
                {acc.status}
              </span>
            </div>
          </div>
        ))}
      </section>

      {/* ── Active Loans ────────────────────────────────────────── */}
      {loans.length > 0 && (
        <section style={{ marginBottom: 24 }}>
          <h2>Active Loans</h2>
          {loans.map(loan => (
            <div key={loan.id} style={{ padding: '12px 16px', border: '1px solid #ddd', borderRadius: 6, marginBottom: 8 }}>
              <span>EUR {loan.outstandingBalance.toFixed(2)} outstanding</span>
              <span style={{ marginLeft: 16, color: '#666' }}>{loan.interestRate}% — {loan.termMonths} months</span>
              <span style={{ marginLeft: 16, fontSize: 12, color: '#999' }}>next payment {loan.nextPaymentDate}</span>
            </div>
          ))}
        </section>
      )}

      {/* ── Notifications ───────────────────────────────────────── */}
      <section>
        <h2>Notifications {notifs.filter(n => !n.read).length > 0 &&
          <span style={{ fontSize: 14, color: 'blue' }}>({notifs.filter(n => !n.read).length} unread)</span>}
        </h2>
        {notifsLoading ? <p>Loading…</p> : notifs.map((n: Notification) => (
          <div key={n.id} style={{
            padding: '10px 14px', marginBottom: 8, borderRadius: 6,
            background: n.read ? '#fafafa' : '#fffbe6',
            border: `1px solid ${n.read ? '#eee' : '#ffe58f'}`,
          }}>
            <strong>{n.title}</strong>
            <p style={{ margin: '4px 0', fontSize: 14 }}>{n.body}</p>
            {!n.read && (
              <button style={{ fontSize: 12 }}
                onClick={() => markReadMutation.mutate({ id: n.id })}
                disabled={markReadMutation.isPending}>
                Mark as read
              </button>
            )}
          </div>
        ))}
      </section>
    </div>
  );
}
