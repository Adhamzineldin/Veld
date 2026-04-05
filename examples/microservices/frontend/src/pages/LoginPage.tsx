import { useState } from 'react';
import { iamApi } from '@veld/client/iamApi';
import type { VeldApiError } from '@veld/client/_internal';
import { authToken } from '../App';

// iamApi is Veld-generated from iam.veld — fully typed, zero manual wiring.
export default function LoginPage({ onLogin }: { onLogin: () => void }) {
  const [email, setEmail]       = useState('alice@nexusbank.com');
  const [password, setPassword] = useState('secret');
  const [error, setError]       = useState('');
  const [loading, setLoading]   = useState(false);

  const handleLogin = async () => {
    setLoading(true);
    setError('');
    try {
      const tokens = await iamApi.login({ email, password });
      // Store token globally so AuthMiddleware can attach it to requests.
      (window as any).__nexusToken = tokens.accessToken;
      onLogin();
    } catch (e: unknown) {
      const err = e as VeldApiError;
      // err.code is typed — it's one of the generated error codes from iam.errors
      setError(err.message ?? 'Login failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ maxWidth: 360, margin: '80px auto', fontFamily: 'sans-serif' }}>
      <h1>NexusBank</h1>
      <input style={{ display: 'block', width: '100%', marginBottom: 8 }}
        placeholder="Email" value={email} onChange={e => setEmail(e.target.value)} />
      <input style={{ display: 'block', width: '100%', marginBottom: 8 }}
        type="password" placeholder="Password" value={password}
        onChange={e => setPassword(e.target.value)} />
      {error && <p style={{ color: 'red' }}>{error}</p>}
      <button onClick={handleLogin} disabled={loading}>
        {loading ? 'Signing in…' : 'Sign in'}
      </button>
    </div>
  );
}
