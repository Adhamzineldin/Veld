import { motion } from 'framer-motion';
import styles from './HowItWorks.module.css';

const steps = [
  {
    num: '1',
    label: 'Define',
    desc: 'Write models & endpoints in .veld contract files',
    color: 'var(--accent)',
  },
  {
    num: '2',
    label: 'Generate',
    desc: 'Run veld generate from your terminal',
    color: 'var(--accent2)',
  },
  {
    num: '3',
    label: 'Implement',
    desc: 'Write business logic against typed interfaces',
    color: 'var(--accent3)',
  },
  {
    num: '4',
    label: 'Ship',
    desc: 'Frontend SDK is ready to call your API',
    color: '#f0883e',
  },
];

export default function HowItWorks() {
  return (
    <section className={styles.section} id="how-it-works">
      <div className={styles.container}>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <h2 className={styles.heading}>How it works</h2>
          <p className={styles.subtitle}>
            From contract to production in four simple steps.
          </p>
        </motion.div>

        <div className={styles.steps}>
          {steps.map((step, i) => (
            <motion.div
              key={step.num}
              className={styles.stepRow}
              initial={{ opacity: 0, y: 30 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ duration: 0.4, delay: i * 0.12 }}
            >
              {i > 0 && <div className={styles.arrow}>↓</div>}
              <div className={styles.step}>
                <div className={styles.numBadge} style={{ background: step.color }}>
                  {step.num}
                </div>
                <div className={styles.stepContent}>
                  <div className={styles.label}>{step.label}</div>
                  <div className={styles.desc}>{step.desc}</div>
                </div>
              </div>
            </motion.div>
          ))}
        </div>


      </div>
    </section>
  );
}

