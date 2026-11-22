import { useState } from 'react';
import { InsufficientFunds, NotFound } from '@veld/web';
import { useTransfer } from '@veld/web/transactions/react';

/**
 * Screen two: move money. The call goes to the Java transactions service; its typed
 * `InsufficientFunds` arrives here as a real class with typed `data`.
 */
export function Transfer({ accountId, onDone }: { accountId: string; onDone: () => void }) {
  const [iban, setIban] = useState('');
  const [amount, setAmount] = useState('');
  const transfer = useTransfer();

  async function send(): Promise<void> {
    const receipt = await transfer.mutate({
      body: { fromAccountId: accountId, toIban: iban, amount, currency: 'EUR' },
    });
    if (receipt) onDone();
  }

  return (
    <form onSubmit={(e) => e.preventDefault()}>
      <h3>Transfer</h3>
      <input data-testid="to-iban" placeholder="To IBAN" value={iban} onChange={(e) => setIban(e.target.value)} />
      <input data-testid="amount" placeholder="Amount" value={amount} onChange={(e) => setAmount(e.target.value)} />
      <button data-testid="send" onClick={() => void send()}>
        Send
      </button>
      {transfer.data && <p data-testid="receipt">Sent. New balance {transfer.data.source.balance}.</p>}
      {InsufficientFunds.matches(transfer.error) && (
        <p role="alert" data-testid="insufficient">
          Insufficient funds: {transfer.error.data.available} available, {transfer.error.data.required} required.
        </p>
      )}
      {NotFound.matches(transfer.error) && <p role="alert">That account was not found.</p>}
    </form>
  );
}
