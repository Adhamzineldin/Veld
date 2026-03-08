import { motion } from 'framer-motion';
import styles from './SupportedStacks.module.css';

const backends = [
  { name: 'Node.js (TypeScript)', icon: '🟦' },
  { name: 'Node.js (JavaScript)', icon: '🟨' },
  { name: 'Python', icon: '🐍' },
  { name: 'Go', icon: '🐹' },
  { name: 'Rust', icon: '🦀' },
  { name: 'Java', icon: '☕' },
  { name: 'C#', icon: '💜' },
  { name: 'PHP', icon: '🐘' },
];

const frontends = [
  { name: 'TypeScript', icon: '🔷' },
  { name: 'JavaScript', icon: '🟡' },
  { name: 'React', icon: '⚛️' },
  { name: 'Vue', icon: '💚' },
  { name: 'Angular', icon: '🔺' },
  { name: 'Svelte', icon: '🔥' },
  { name: 'Dart / Flutter', icon: '🎯' },
  { name: 'Kotlin', icon: '🟣' },
  { name: 'Swift', icon: '🍎' },
];

const extras = [
  { name: 'OpenAPI 3.0', icon: '📋' },
  { name: 'Prisma Schema', icon: '💎' },
  { name: 'SQL DDL', icon: '🗄️' },
  { name: 'Dockerfile', icon: '🐳' },
  { name: 'GitHub Actions', icon: '⚡' },
  { name: 'GitLab CI', icon: '🦊' },
];

const container = {
  hidden: {},
  show: { transition: { staggerChildren: 0.03 } },
};

const item = {
  hidden: { opacity: 0, scale: 0.9 },
  show: { opacity: 1, scale: 1 },
};

export default function SupportedStacks() {
  return (
    <section className={styles.section} id="stacks">
      <div className={styles.container}>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <h2 className={styles.heading}>Works with your stack</h2>
          <p className={styles.subtitle}>
            Mix and match any backend with any frontend. Veld generates code for all of them.
          </p>
        </motion.div>

        <div className={styles.stackGroup}>
          <div className={styles.label}>Backends</div>
          <motion.div
            className={styles.row}
            variants={container}
            initial="hidden"
            whileInView="show"
            viewport={{ once: true }}
          >
            {backends.map((s) => (
              <motion.span key={s.name} className={styles.tag} variants={item}>
                <span className={styles.tagIcon}>{s.icon}</span>
                {s.name}
              </motion.span>
            ))}
          </motion.div>
        </div>

        <div className={styles.stackGroup}>
          <div className={styles.label}>Frontend SDKs</div>
          <motion.div
            className={styles.row}
            variants={container}
            initial="hidden"
            whileInView="show"
            viewport={{ once: true }}
          >
            {frontends.map((s) => (
              <motion.span key={s.name} className={styles.tag} variants={item}>
                <span className={styles.tagIcon}>{s.icon}</span>
                {s.name}
              </motion.span>
            ))}
          </motion.div>
        </div>

        <div className={styles.stackGroup}>
          <div className={styles.label}>Extras</div>
          <motion.div
            className={styles.row}
            variants={container}
            initial="hidden"
            whileInView="show"
            viewport={{ once: true }}
          >
            {extras.map((s) => (
              <motion.span key={s.name} className={styles.tag} variants={item}>
                <span className={styles.tagIcon}>{s.icon}</span>
                {s.name}
              </motion.span>
            ))}
          </motion.div>
        </div>
      </div>
    </section>
  );
}

