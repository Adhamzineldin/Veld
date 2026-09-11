import { ArrowRight, ChevronRight, Github, Layers, ShieldCheck, Boxes, Zap } from 'lucide-react';
import { Link } from 'react-router-dom';
import { motion } from 'framer-motion';
import ContractOrb from '../ContractOrb';
import styles from './Hero.module.css';

/** Fixed scatter of corner-bracket glyphs, as in the reference art. */
const GLYPHS = [
  { top: 8, left: 5, size: 14, delay: 0 },
  { top: 16, left: 27, size: 11, delay: 1.4 },
  { top: 6, left: 48, size: 9, delay: 2.2 },
  { top: 34, left: 38, size: 12, delay: 0.7 },
  { top: 58, left: 8, size: 10, delay: 1.9 },
  { top: 72, left: 33, size: 13, delay: 2.6 },
  { top: 12, left: 82, size: 10, delay: 1.1 },
  { top: 46, left: 95, size: 12, delay: 0.4 },
  { top: 80, left: 88, size: 9, delay: 2.9 },
  { top: 88, left: 60, size: 11, delay: 1.6 },
];

const STRIP = [
  { icon: Layers, label: 'Typed interfaces' },
  { icon: ShieldCheck, label: 'Validated routes' },
  { icon: Boxes, label: '13+ target stacks' },
  { icon: Zap, label: 'Zero runtime deps' },
];

export default function Hero() {
  return (
    <section className={styles.hero}>
      <div className={styles.grid} />
      <div className={styles.glowRight} />
      <div className={styles.glowLeft} />

      <div className={styles.glyphField} aria-hidden="true">
        {GLYPHS.map((g, i) => (
          <span
            key={i}
            className={styles.glyph}
            style={{
              top: `${g.top}%`,
              left: `${g.left}%`,
              width: g.size,
              height: g.size,
              animationDelay: `${g.delay}s`,
            }}
          />
        ))}
      </div>

      <div className={styles.container}>
        <motion.div
          className={styles.copy}
          initial={{ opacity: 0, y: 30 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.6 }}
        >
          <div className={styles.eyebrow}>
            <span className={styles.eyebrowDot} />
            Contract-First Code Generation
          </div>

          <h1 className={styles.title}>
            API contracts that
            <br />
            <span className={styles.gradient}>write your code</span>
          </h1>

          <p className={styles.subtitle}>
            Define your API once in <code>.veld</code> files. Veld generates the typed backend,
            the frontend SDK, validation, OpenAPI specs and more — for any stack, with zero
            runtime dependencies.
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
              <Github size={17} />
              View on GitHub
              <ChevronRight size={16} />
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

          <div className={styles.tagline}>Backend · Contract · Frontend</div>
        </motion.div>

        <motion.div
          className={styles.visual}
          initial={{ opacity: 0, scale: 0.94 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ duration: 0.9, delay: 0.15 }}
        >
          <ContractOrb />
        </motion.div>
      </div>

      <div className={styles.strip}>
        {STRIP.map(({ icon: Icon, label }) => (
          <div key={label} className={styles.stripItem}>
            <Icon size={15} />
            <span>{label}</span>
          </div>
        ))}
      </div>
    </section>
  );
}
