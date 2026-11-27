import { AmountMismatch, Forbidden, InvalidTransition, NotFound, Unauthorized, ValidationError } from '@veld/web';

/**
 * What to tell a person about a failed call. The server's own words are used wherever it has some:
 * the message of the policy that said no, the two states of an illegal move, the violated rules.
 */
export function describe(error: unknown): string {
  if (Forbidden.matches(error)) return error.data.message ?? 'Not permitted.';
  if (InvalidTransition.matches(error)) return `A booking cannot go from ${error.data.from} to ${error.data.to}.`;
  if (AmountMismatch.matches(error)) return `The price is ${error.data.expected}.`;
  if (ValidationError.matches(error) && error.violations.length > 0) return error.violations.map((v) => v.message).join(' ');
  if (NotFound.matches(error)) return `There is no such ${error.data.resource ?? 'record'}.`;
  if (Unauthorized.matches(error)) return 'Sign in first.';
  return error instanceof Error ? error.message : 'Something went wrong.';
}
