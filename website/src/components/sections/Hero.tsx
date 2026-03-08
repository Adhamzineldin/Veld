import { ArrowRight, Github, Terminal } from 'lucide-react';
import { Link } from 'react-router-dom';
import { motion } from 'framer-motion';
import styles from './Hero.module.css';

export default function Hero() {
  return (
    <section className={styles.hero}>
      <div className={styles.container}>
        <motion.div
          initial={{ opacity: 0, y: 30 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.6 }}
        >
          <div className={styles.badge}>
            <Terminal size={14} />
            Contract-First Code Generation
          </div>

          <h1 className={styles.title}>
            API contracts that
            <br />
            <span className={styles.gradient}>write your code</span>
          </h1>

          <p className={styles.subtitle}>
            Define your API once in <code>.veld</code> files. Get typed backends, frontend SDKs,
            validation, OpenAPI specs, and more — for any stack, with zero runtime dependencies.
          </p>

          <div className={styles.actions}>
            <Link to="/docs" className={styles.btnPrimary}>
              Get Started <ArrowRight size={18} />
            </Link>
            <a
              href="https://github.com/Adhamzineldin/Veld"
              target="_blank"
              rel="noopener noreferrer"
              className={styles.btnSecondary}
            >
              <Github size={18} />
              View on GitHub
            </a>
          </div>

          <div className={styles.installBar}>
            <span className={styles.prompt}>$</span>
            <code className={styles.installCode}>npm install @maayn/veld</code>
          </div>

          <div className={styles.installOptions}>
            <span>Also available via</span>
            <code>pip</code>
            <code>brew</code>
            <code>go install</code>
            <code>composer</code>
          </div>
        </motion.div>
      </div>

      {/* Background glow effect */}
      <div className={styles.glowTop} />
      <div className={styles.glowBottom} />
    </section>
  );
}

