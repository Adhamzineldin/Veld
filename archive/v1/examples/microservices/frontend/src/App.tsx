import { useState } from 'react';
import Dashboard from './pages/Dashboard';
import TransferPage from './pages/TransferPage';
import LoginPage from './pages/LoginPage';

// The token is set after successful login and used by AuthMiddleware
// on every service. In a real app you'd use a proper auth context/store.
export let authToken = '';

type Page = 'login' | 'dashboard' | 'transfer';

export default function App() {
  const [page, setPage] = useState<Page>('login');

  if (page === 'login') return <LoginPage onLogin={() => setPage('dashboard')} />;

  return (
    <div style={{ maxWidth: 800, margin: '0 auto', padding: 24, fontFamily: 'sans-serif' }}>
      <header style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 24 }}>
        <h1 style={{ margin: 0 }}>NexusBank</h1>
        <nav style={{ display: 'flex', gap: 16 }}>
          <button onClick={() => setPage('dashboard')}>Dashboard</button>
          <button onClick={() => setPage('transfer')}>Transfer</button>
        </nav>
      </header>

      {page === 'dashboard' && <Dashboard />}
      {page === 'transfer'  && <TransferPage />}
    </div>
  );
}
