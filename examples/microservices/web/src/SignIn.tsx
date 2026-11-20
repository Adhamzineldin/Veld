import { useState } from 'react';
import { EmailTaken, Unauthorized } from '@veld/web';
import { useLogin, useRegister } from '@veld/web/iam/react';

/** Sign in, or create an account first. Both use the generated hooks of the iam service. */
export function SignIn({ onToken }: { onToken: (accessToken: string) => void }) {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const register = useRegister();
  const login = useLogin();

  async function submit(create: boolean): Promise<void> {
    if (create) {
      const user = await register.mutate({
        body: { email, password, firstName: 'Demo', lastName: 'User' },
      });
      if (!user) return;
    }
    const tokens = await login.mutate({ body: { email, password } });
    if (tokens) onToken(tokens.accessToken);
  }

  return (
    <form onSubmit={(e) => e.preventDefault()}>
      <h2>Sign in</h2>
      <input data-testid="email" placeholder="Email" value={email} onChange={(e) => setEmail(e.target.value)} />
      <input
        data-testid="password"
        type="password"
        placeholder="Password (12+ characters)"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
      />
      <button data-testid="login" onClick={() => void submit(false)}>
        Sign in
      </button>
      <button data-testid="register" onClick={() => void submit(true)}>
        Create account
      </button>
      {EmailTaken.matches(register.error) && <p role="alert">{register.error.data.email} is already registered.</p>}
      {Unauthorized.matches(login.error) && <p role="alert">Wrong email or password.</p>}
    </form>
  );
}
