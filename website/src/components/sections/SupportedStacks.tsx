import { motion } from 'framer-motion';
import styles from './SupportedStacks.module.css';

const backends = [
  { name: 'Node.js (TypeScript)', icon: '🟦' },
  { name: 'Express', icon: '🚂' },
  { name: 'Fastify', icon: '⚡' },
  { name: 'Hono', icon: '🔥' },
  { name: 'node:http', icon: '🟩' },
  { name: 'NestJS', icon: '🐈' },
  { name: 'Java (JDK HTTP server)', icon: '☕' },
  { name: 'Spring Boot 3', icon: '🍃' },
];

const frontends = [
  { name: 'TypeScript client', icon: '🔷' },
  { name: 'React hooks', icon: '⚛️' },
  { name: 'Vue composables', icon: '💚' },
  { name: 'Java client', icon: '☕' },
];

const extras = [
  { name: 'OpenAPI 3.1', icon: '📋' },
  { name: 'IR JSON export', icon: '🧩' },
  { name: 'Emitter plugins', icon: '🔌' },
  { name: 'Package registry', icon: '📦' },
  { name: 'Language server', icon: '🧠' },
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
            Servers and clients are generated from the same contract, in the language you choose. Anything else is an emitter plugin away.
          </p>
        </motion.div>

        <div className={styles.stackGroup}>
          <div className={styles.label}>Servers</div>
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
          <div className={styles.label}>Clients</div>
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

