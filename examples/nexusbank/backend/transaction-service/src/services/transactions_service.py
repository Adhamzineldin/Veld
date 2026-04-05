import uuid, math
from datetime import datetime, timezone
from generated.interfaces.i_transactions_service import ITransactionsService
from generated.types.transactions import (
    Transaction, TransferInput, TransactionListResult,
)

# In-memory store — swap for Postgres in production.
_store: list[dict] = [
    {
        "id": str(uuid.uuid4()), "accountId": "acc-001",
        "type": "credit", "status": "completed",
        "amount": 2500.00, "currency": "EUR",
        "description": "Salary — April", "reference": "TXN-SAL001",
        "createdAt": "2024-04-01T08:00:00Z",
    },
    {
        "id": str(uuid.uuid4()), "accountId": "acc-001",
        "type": "debit", "status": "completed",
        "amount": 89.99, "currency": "EUR",
        "description": "Supermarket", "reference": "TXN-MKT001",
        "createdAt": "2024-04-02T14:22:00Z",
    },
]


class TransactionsService(ITransactionsService):
    def get_history(self, account_id: str) -> TransactionListResult:
        rows = [t for t in _store if t["accountId"] == account_id]
        return TransactionListResult(
            items=[Transaction(**t) for t in rows],
            total=len(rows),
            page=1,
            pageSize=len(rows),
        )

    def get_transaction(self, id: str) -> Transaction:
        row = next((t for t in _store if t["id"] == id), None)
        if not row:
            raise LookupError(f"Transaction {id} not found")
        return Transaction(**row)

    def transfer(self, input: TransferInput) -> Transaction:
        row = {
            "id": str(uuid.uuid4()),
            "accountId": input.fromAccountId,
            "type": "transfer",
            "status": "completed",
            "amount": input.amount,
            "currency": input.currency,
            "description": input.description or f"Transfer to {input.toIban}",
            "reference": f"TXN-{str(uuid.uuid4())[:8].upper()}",
            "createdAt": datetime.now(timezone.utc).isoformat(),
        }
        _store.append(row)
        return Transaction(**row)
