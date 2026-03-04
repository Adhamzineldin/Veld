import { useState } from 'react';
import { motion } from 'framer-motion';
import { Play, CheckCircle, Terminal } from 'lucide-react';
import styles from './LiveDemo.module.css';

const steps = [
  { cmd: 'mkdir my-api && cd my-api', output: '' },
  {
    cmd: 'veld init',
    output: `  Veld — project setup

  Backend — which server runtime?
     1  csharp
     2  go
     3  java
     4  node-js
  ▸  5  node-ts (default)
     6  php
     7  python
     8  rust

  Choose [5]: 5

  Frontend — which client framework?
     1  typescript (default)
     2  react
     3  vue
     4  angular

  Choose [1]: 2

  ✓ Created veld/veld.config.json
  ✓ Created veld/app.veld
  ✓ Created veld/models/user.veld
  ✓ Created veld/modules/users.veld
  ✓ Project initialized!`,
  },
  {
    cmd: 'veld generate',
    output: `  ✓ Loaded config from veld/veld.config.json
  ✓ Parsed 3 files (2 models, 1 module, 4 actions)
  ✓ Validation passed
  ✓ Generated backend  → generated/
  ✓ Generated frontend → generated/client/
  ✓ Generated schemas  → generated/schemas/
  
  Done in 23ms — 12 files written`,
  },
  {
    cmd: 'veld validate',
    output: `  ✓ Contract is valid (2 models, 1 enum, 4 actions)
  ✓ No circular dependencies
  ✓ All types resolved
  ✓ No breaking changes detected`,
  },
];

export default function LiveDemo() {
  const [currentStep, setCurrentStep] = useState(0);
  const [running, setRunning] = useState(false);

  const runStep = () => {
    if (currentStep >= steps.length) {
      setCurrentStep(0);
      return;
    }
    setRunning(true);
    setTimeout(() => {
      setRunning(false);
      setCurrentStep((prev) => prev + 1);
    }, 800);
  };

  return (
    <section className={styles.section} id="try-it">
      <div className={styles.container}>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <h2 className={styles.heading}>Try it yourself</h2>
          <p className={styles.subtitle}>
            Walk through a real Veld workflow — click to run each command.
          </p>
        </motion.div>

        <motion.div
          className={styles.terminal}
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5, delay: 0.1 }}
        >
          <div className={styles.termHeader}>
            <span className={styles.dot} style={{ background: '#f85149' }} />
            <span className={styles.dot} style={{ background: '#f0883e' }} />
            <span className={styles.dot} style={{ background: 'var(--accent2)' }} />
            <span className={styles.termTitle}>
              <Terminal size={14} /> Terminal
            </span>
          </div>

          <div className={styles.termBody}>
            {steps.slice(0, currentStep).map((step, i) => (
              <div key={i} className={styles.termBlock}>
                <div className={styles.termLine}>
                  <span className={styles.prompt}>$</span>
                  <span className={styles.cmd}>{step.cmd}</span>
                  <CheckCircle size={14} className={styles.check} />
                </div>
                {step.output && (
                  <pre className={styles.output}>{step.output}</pre>
                )}
              </div>
            ))}

            {currentStep < steps.length && (
              <div className={styles.termLine}>
                <span className={styles.prompt}>$</span>
                <span className={styles.cmd}>
                  {running ? steps[currentStep].cmd : ''}
                  {running && <span className={styles.cursor}>▋</span>}
                </span>
              </div>
            )}

            {currentStep >= steps.length && (
              <div className={styles.termLine}>
                <span className={styles.prompt}>$</span>
                <span className={styles.readyMsg}>
                  ✨ Your typed API is ready to use!
                </span>
              </div>
            )}
          </div>

          <div className={styles.termFooter}>
            <button className={styles.runBtn} onClick={runStep} disabled={running}>
              <Play size={16} />
              {currentStep >= steps.length
                ? 'Reset'
                : currentStep === 0
                ? 'Start Demo'
                : 'Next Step'}
            </button>
            <span className={styles.stepCount}>
              Step {Math.min(currentStep + 1, steps.length)} of {steps.length}
            </span>
          </div>
        </motion.div>
      </div>
    </section>
  );
}

