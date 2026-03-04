import { motion } from 'framer-motion';
import { ArrowRight, Github } from 'lucide-react';
import styles from './CTA.module.css';

export default function CTA() {
  return (
    <section className={styles.section}>
      <div className={styles.container}>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
          className={styles.content}
        >
          <h2 className={styles.heading}>
            Stop writing <span className={styles.gradient}>boilerplate</span>
          </h2>
          <p className={styles.subtitle}>
            Define your API contract once. Let Veld handle the rest.
            <br />
            Typed backends, frontend SDKs, validation — in seconds.
          </p>
          <div className={styles.actions}>
            <a href="#install" className={styles.btnPrimary}>
              Get Started <ArrowRight size={18} />
            </a>
            <a
              href="https://github.com/Adhamzineldin/Veld/blob/master/docs/guides/getting-started.md"
              target="_blank"
              rel="noopener noreferrer"
              className={styles.btnSecondary}
            >
              Read the Guide
            </a>
            <a
              href="https://github.com/Adhamzineldin/Veld"
              target="_blank"
              rel="noopener noreferrer"
              className={styles.btnSecondary}
            >
              <Github size={18} />
              GitHub
            </a>
          </div>
        </motion.div>
      </div>

      <div className={styles.glow} />
    </section>
  );
}

