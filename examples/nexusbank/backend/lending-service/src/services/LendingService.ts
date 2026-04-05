import { ILendingService } from '@veld/generated/interfaces/ILendingService';
import { Loan, LoanApplicationInput, LoanPaymentInput } from '@veld/generated/types/lending';
import { lendingErrors } from '@veld/generated/errors/lending.errors';
import { randomUUID } from 'crypto';

const store: Loan[] = [
  {
    id: 'loan-001', userId: 'user-001', accountId: 'acc-001',
    principalAmount: 10000, outstandingBalance: 8500,
    interestRate: 5.9, termMonths: 36, status: 'active',
    nextPaymentDate: '2024-05-01', createdAt: '2024-02-01T00:00:00Z',
  },
];

export class LendingService implements ILendingService {
  async listLoans(req: any): Promise<Loan[]> {
    return store.filter(l => l.userId === req.userId);
  }

  async getLoan(req: any, id: string): Promise<Loan> {
    const l = store.find(l => l.id === id && l.userId === req.userId);
    if (!l) throw lendingErrors.getLoan.notFound(`Loan ${id} not found`);
    return l;
  }

  async applyForLoan(req: any, input: LoanApplicationInput): Promise<Loan> {
    if (input.amount <= 0 || input.termMonths <= 0)
      throw lendingErrors.applyForLoan.badRequest('Amount and term must be positive');
    const loan: Loan = {
      id: randomUUID(), userId: req.userId, accountId: input.accountId,
      principalAmount: input.amount, outstandingBalance: input.amount,
      interestRate: 5.9, termMonths: input.termMonths, status: 'pending',
      nextPaymentDate: new Date(Date.now() + 30 * 864e5).toISOString().slice(0, 10),
      createdAt: new Date().toISOString(),
    };
    store.push(loan);
    return loan;
  }

  async makePayment(req: any, input: LoanPaymentInput): Promise<Loan> {
    const loan = store.find(l => l.id === input.loanId && l.userId === req.userId);
    if (!loan) throw lendingErrors.makePayment.notFound(`Loan ${input.loanId} not found`);
    if (input.amount <= 0) throw lendingErrors.makePayment.badRequest('Payment must be positive');
    loan.outstandingBalance = Math.max(0, loan.outstandingBalance - input.amount);
    if (loan.outstandingBalance === 0) loan.status = 'paid_off';
    return loan;
  }
}
