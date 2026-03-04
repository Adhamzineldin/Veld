import styles from './Footer.module.css';
import { Github, Heart } from 'lucide-react';

export default function Footer() {
  return (
    <footer className={styles.footer}>
      <div className={styles.container}>
        <div className={styles.top}>
          <div className={styles.brand}>
            <span className={styles.logo}>
              <span className={styles.bracket}>&lt;</span>Veld<span className={styles.bracket}>/&gt;</span>
            </span>
            <p className={styles.tagline}>
              Contract-first API code generator.
              <br />
              Write once, generate everywhere.
            </p>
          </div>

          <div className={styles.columns}>
            <div className={styles.column}>
              <h4>Product</h4>
              <a href="#features">Features</a>
              <a href="#stacks">Supported Stacks</a>
              <a href="#how-it-works">How It Works</a>
              <a href="#install">Installation</a>
            </div>
            <div className={styles.column}>
              <h4>Resources</h4>
              <a href="https://github.com/Adhamzineldin/Veld/blob/master/docs/guides/getting-started.md" target="_blank" rel="noopener noreferrer">Documentation</a>
              <a href="https://github.com/Adhamzineldin/Veld/tree/master/examples" target="_blank" rel="noopener noreferrer">Examples</a>
              <a href="https://github.com/Adhamzineldin/Veld/blob/master/docs/changelog-v0.1.md" target="_blank" rel="noopener noreferrer">Changelog</a>
              <a href="https://github.com/Adhamzineldin/Veld/blob/master/docs/roadmap.md" target="_blank" rel="noopener noreferrer">Roadmap</a>
            </div>
            <div className={styles.column}>
              <h4>Community</h4>
              <a href="https://github.com/Adhamzineldin/Veld" target="_blank" rel="noopener noreferrer">
                <Github size={14} /> GitHub
              </a>
              <a href="https://github.com/Adhamzineldin/Veld/issues" target="_blank" rel="noopener noreferrer">Issues</a>
              <a href="https://github.com/Adhamzineldin/Veld/blob/master/LICENSE" target="_blank" rel="noopener noreferrer">MIT License</a>
            </div>
          </div>
        </div>

        <div className={styles.bottom}>
          <p>
            Made with <Heart size={14} className={styles.heart} /> by the Veld team &bull; &copy; {new Date().getFullYear()} Veld
          </p>
        </div>
      </div>
    </footer>
  );
}

