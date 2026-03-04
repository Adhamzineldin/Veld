import { useState, useEffect } from 'react';
import { Menu, X, Github } from 'lucide-react';
import styles from './Navbar.module.css';

const navLinks = [
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

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 20);
    window.addEventListener('scroll', onScroll);
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  return (
    <nav className={`${styles.nav} ${scrolled ? styles.scrolled : ''}`}>
      <div className={styles.container}>
        <a href="#" className={styles.logo}>
          <span className={styles.bracket}>&lt;</span>Veld<span className={styles.bracket}>/&gt;</span>
        </a>

        <div className={`${styles.links} ${mobileOpen ? styles.open : ''}`}>
          {navLinks.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className={styles.link}
              onClick={() => setMobileOpen(false)}
            >
              {link.label}
            </a>
          ))}
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

