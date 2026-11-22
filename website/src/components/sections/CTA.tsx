import { motion } from 'framer-motion';
import { Link } from 'react-router-dom';
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
            Typed servers, typed clients and validation, from one contract.
          </p>
          <div className={styles.actions}>
            <Link to="/docs" className={styles.btnPrimary}>
              Get Started <ArrowRight size={18} />
            </Link>
            <Link to="/docs#first-contract" className={styles.btnSecondary}>
              Read the Guide
            </Link>
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

