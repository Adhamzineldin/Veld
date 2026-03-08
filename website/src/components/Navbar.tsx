import { useState, useEffect } from 'react';
import { useLocation, Link } from 'react-router-dom';
import { Menu, X, Github } from 'lucide-react';
import styles from './Navbar.module.css';

const homeLinks = [
  { label: 'Features', href: '#features' },
  { label: 'How It Works', href: '#how-it-works' },
  { label: 'Example', href: '#example' },
  { label: 'Stacks', href: '#stacks' },
  { label: 'Install', href: '#install' },
  { label: 'Contributors', href: '#contributors' },
];

export default function Navbar() {
  const [scrolled, setScrolled] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const location = useLocation();
  const isHome = location.pathname === '/';

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 20);
    window.addEventListener('scroll', onScroll);
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  return (
    <nav className={`${styles.nav} ${scrolled ? styles.scrolled : ''}`}>
      <div className={styles.container}>
        <Link to="/" className={styles.logo}>
          <span className={styles.bracket}>&lt;</span>Veld<span className={styles.bracket}>/&gt;</span>
        </Link>

        <div className={`${styles.links} ${mobileOpen ? styles.open : ''}`}>
          {isHome && homeLinks.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className={styles.link}
              onClick={() => setMobileOpen(false)}
            >
              {link.label}
            </a>
          ))}
          {!isHome && (
            <Link to="/" className={styles.link} onClick={() => setMobileOpen(false)}>
              Home
            </Link>
          )}
          <Link
            to="/docs"
            className={`${styles.link} ${location.pathname === '/docs' ? styles.activeLink : ''}`}
            onClick={() => setMobileOpen(false)}
          >
            Docs
          </Link>
        </div>

        <div className={styles.actions}>
          <a
            href="https://github.com/Adhamzineldin/Veld"
            target="_blank"
            rel="noopener noreferrer"
            className={styles.githubBtn}
          >
            <Github size={16} />
            GitHub
          </a>
          <button
            className={styles.menuBtn}
            onClick={() => setMobileOpen(!mobileOpen)}
            aria-label="Toggle menu"
          >
            {mobileOpen ? <X size={22} /> : <Menu size={22} />}
          </button>
        </div>
      </div>
    </nav>
  );
}

