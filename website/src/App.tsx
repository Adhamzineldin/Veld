import { lazy, Suspense } from 'react';
import { Routes, Route } from 'react-router-dom';
import Navbar from './components/Navbar';
import Footer from './components/Footer';
import LandingPage from './pages/LandingPage';
import DocsPage from './pages/DocsPage';
import ScrollToTop from './components/ScrollToTop';

// The playground carries the WebAssembly engine, so it loads only when its route opens.
const PlaygroundPage = lazy(() => import('./pages/PlaygroundPage'));

export default function App() {
  return (
    <>
      <ScrollToTop />
      <Navbar />
      <main>
        <Routes>
          <Route path="/" element={<LandingPage />} />
          <Route path="/docs" element={<DocsPage />} />
          <Route
            path="/playground"
            element={
              <Suspense fallback={<p style={{ padding: 48, textAlign: 'center', color: 'var(--fg2)' }}>Loading the playground</p>}>
                <PlaygroundPage />
              </Suspense>
            }
          />
        </Routes>
      </main>
      <Footer />
    </>
  );
}
