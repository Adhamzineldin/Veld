import { useState } from 'react';
import type { TransferInput, Transaction } from '@veld/generated/types';
import type { VeldApiError } from '@veld/client/_internal';
import { transactionsApi } from '@veld/client/transactionsApi';
import { useListAccounts } from '@veld/hooks/accountsHooks';
import { useGetHistory, useTransfer } from '@veld/hooks/transactionsHooks';

export default function TransferPage() {
  const { data: accounts = [] } = useListAccounts();
  const [selectedId, setSelectedId] = useState('');
  const [form, setForm] = useState<TransferInput>({
    fromAccountId: '', toIban: '', amount: 0, currency: 'EUR',
  });
  const [done, setDone]   = useState<Transaction | null>(null);
  const [err, setErr]     = useState('');

  // useGetHistory — GET /api/transactions/account/:accountId
  // Only fires when selectedId is set (enabled: !!selectedId)
  const { data: history } = useGetHistory(selectedId, { enabled: !!selectedId });

  // useTransfer wraps POST /api/transactions/transfer
  const transferMutation = useTransfer({
    onSuccess: txn => { setDone(txn); setErr(''); },
    onError: (e: VeldApiError) => {
      // err.code matches generated error constants:
      // transactionsApi.errors.transfer.badRequest
      // transactionsApi.errors.transfer.unprocessableEntity
      if (e.code === transactionsApi.errors.transfer.badRequest)
        setErr('Invalid transfer details — check the IBAN and amount.');
      else if (e.code === transactionsApi.errors.transfer.unprocessableEntity)
        setErr('Insufficient funds or account is frozen.');
      else
        setErr(e.message ?? 'Transfer failed');
    },
  });

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setDone(null);
    setErr('');
    transferMutation.mutate({ input: { ...form, fromAccountId: selectedId } });
  };

  return (
    <div>
      <h2>New Transfer</h2>

      <form onSubmit={submit} style={{ maxWidth: 420 }}>
        <label style={{ display: 'block', marginBottom: 12 }}>
          From account
          <select style={{ display: 'block', width: '100%', marginTop: 4 }}
            value={selectedId}
            onChange={e => { setSelectedId(e.target.value); setForm(f => ({ ...f, fromAccountId: e.target.value })); }}>
            <option value="">— Select —</option>
            {accounts.map(a => (
              <option key={a.id} value={a.id}>
                {a.name} · {a.currency} {a.balance.toFixed(2)} · {a.iban}
              </option>
            ))}
          </select>
        </label>

        <label style={{ display: 'block', marginBottom: 12 }}>
          Destination IBAN
          <input style={{ display: 'block', width: '100%', marginTop: 4 }}
            placeholder="DE89 3704 0044 0532 0130 00"
            value={form.toIban} onChange={e => setForm(f => ({ ...f, toIban: e.target.value }))} />
        </label>

        <label style={{ display: 'block', marginBottom: 12 }}>
          Amount (EUR)
          <input type="number" min="0.01" step="0.01"
            style={{ display: 'block', width: '100%', marginTop: 4 }}
            value={form.amount || ''} onChange={e => setForm(f => ({ ...f, amount: parseFloat(e.target.value) }))} />
        </label>

        {err  && <p style={{ color: 'red' }}>{err}</p>}
        {done && <p style={{ color: 'green' }}>Transfer {done.reference} submitted — {done.status}</p>}

        <button type="submit" disabled={!selectedId || transferMutation.isPending}>
          {transferMutation.isPending ? 'Sending…' : 'Send Transfer'}
        </button>
      </form>

      {/* Recent transactions for the selected account */}
      {selectedId && history && (
        <section style={{ marginTop: 32 }}>
          <h3>Recent transactions for this account</h3>
          {history.items.map(txn => (
            <div key={txn.id} style={{ padding: '8px 0', borderBottom: '1px solid #eee' }}>
              <span style={{ color: txn.type === 'credit' ? 'green' : 'inherit' }}>
                [{txn.type}]
              </span>{' '}
              {txn.description} — {txn.currency} {txn.amount.toFixed(2)}
              <span style={{ float: 'right', fontSize: 12, color: '#999' }}>{txn.reference}</span>
            </div>
          ))}
        </section>
      )}
    </div>
  );
}
