import { useState } from 'react';
import { motion } from 'framer-motion';
import { Copy, Check, Package, Terminal } from 'lucide-react';
import styles from './Installation.module.css';

const methods = [
  {
    id: 'npm',
    label: 'npm',
    icon: <Package size={16} />,
    command: 'npm install @maayn/veld',
    run: 'npx @maayn/veld generate',
  },
  {
    id: 'pip',
    label: 'pip',
    icon: <Package size={16} />,
    command: 'pip install maayn-veld',
    run: 'veld generate',
  },
  {
    id: 'brew',
    label: 'Homebrew',
    icon: <Terminal size={16} />,
    command: 'brew install veld-dev/tap/veld',
    run: 'veld generate',
  },
  {
    id: 'go',
    label: 'Go',
    icon: <Terminal size={16} />,
    command: 'go install github.com/Adhamzineldin/Veld/cmd/veld@latest',
    run: 'veld generate',
  },
  {
    id: 'composer',
    label: 'Composer',
    icon: <Package size={16} />,
    command: 'composer require veld-dev/veld',
    run: 'vendor/bin/veld generate',
  },
];

export default function Installation() {
  const [active, setActive] = useState('npm');
  const [copied, setCopied] = useState(false);

  const current = methods.find((m) => m.id === active)!;

  const copyCommand = () => {
    navigator.clipboard.writeText(current.command);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <section className={styles.section} id="install">
      <div className={styles.container}>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <h2 className={styles.heading}>Install in seconds</h2>
          <p className={styles.subtitle}>
            Available on every major package manager. Pick your favorite.
          </p>
        </motion.div>

        <motion.div
          className={styles.installBox}
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5, delay: 0.1 }}
        >
          <div className={styles.tabs}>
            {methods.map((m) => (
              <button
                key={m.id}
                className={`${styles.tab} ${active === m.id ? styles.active : ''}`}
                onClick={() => {
                  setActive(m.id);
                  setCopied(false);
                }}
              >
                {m.icon}
                {m.label}
              </button>
            ))}
          </div>

          <div className={styles.cmdBox}>
            <div className={styles.cmdLine}>
              <span className={styles.prompt}>$</span>
              <code className={styles.cmd}>{current.command}</code>
              <button className={styles.copyBtn} onClick={copyCommand} title="Copy">
                {copied ? <Check size={16} /> : <Copy size={16} />}
              </button>
            </div>
            <div className={styles.cmdLine}>
              <span className={styles.prompt}>$</span>
              <code className={styles.cmd}>{current.run}</code>
            </div>
          </div>
        </motion.div>

        <motion.div
          className={styles.extras}
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5, delay: 0.2 }}
        >
          <div className={styles.extraCard}>
            <h4>📦 Editor Plugins</h4>
            <p>
              <strong>VS Code:</strong> Search "Veld" in the Extensions marketplace
            </p>
            <p>
              <strong>JetBrains:</strong> Settings → Plugins → Marketplace → "Veld"
            </p>
          </div>
          <div className={styles.extraCard}>
            <h4>📥 Manual Download</h4>
            <p>
              Pre-built binaries for <strong>Linux</strong>, <strong>macOS</strong>, and{' '}
              <strong>Windows</strong> (amd64 & arm64) are available on{' '}
              <a
                href="https://github.com/Adhamzineldin/Veld/releases"
                target="_blank"
                rel="noopener noreferrer"
              >
                GitHub Releases
              </a>
              .
            </p>
          </div>
        </motion.div>
      </div>
    </section>
  );
}

