import { motion } from 'framer-motion';
import {
  Zap,
  Layers,
  ShieldCheck,
  AlertTriangle,
  GitBranch,
  Globe,
  FileCode,
  Eye,
  Terminal,
} from 'lucide-react';
import styles from './Features.module.css';

const features = [
  {
    icon: <Zap size={22} />,
    title: 'Zero Runtime Dependencies',
    description:
      'Generated code has no runtime dependencies by default. Types, routes, and SDK work out of the box — no npm install needed.',
  },
  {
    icon: <Layers size={22} />,
    title: 'Framework Agnostic',
    description:
      'Works with Express, Fastify, Hono, Flask, Chi, Actix — any router with .get() and .post(). No lock-in, ever.',
  },
  {
    icon: <ShieldCheck size={22} />,
    title: 'Runtime Validation',
    description:
      'Opt-in zero-dep validators. 400 on bad input, 500 on contract violation. Zod and Pydantic schemas generated automatically.',
  },
  {
    icon: <AlertTriangle size={22} />,
    title: 'Typed Errors',
    description:
      'Define error codes per action. Generated error factories on the backend, typed error matchers on the frontend SDK.',
  },
  {
    icon: <GitBranch size={22} />,
    title: 'Deterministic Output',
    description:
      'Same input always produces identical output. Safe for CI/CD pipelines, code review, and version control.',
  },
  {
    icon: <Globe size={22} />,
    title: 'Multi-Stack',
    description:
      '8 backend languages, 10 frontend targets, plus OpenAPI, database schemas, Docker, and CI/CD generation.',
  },
  {
    icon: <FileCode size={22} />,
    title: 'OpenAPI & Docs',
    description:
      'Auto-generate OpenAPI 3.0 specs and API documentation from the same contract. Always in sync, never stale.',
  },
  {
    icon: <Eye size={22} />,
    title: 'Watch Mode',
    description:
      'Run veld watch for auto-regeneration on every file save with 500ms debounce. Instant feedback loop.',
  },
  {
    icon: <Terminal size={22} />,
    title: 'IDE Support',
    description:
      'VS Code extension, JetBrains plugin, and built-in LSP server. Syntax highlighting, diagnostics, and completions.',
  },
];

const container = {
  hidden: {},
  show: {
    transition: { staggerChildren: 0.06 },
  },
};

const item = {
  hidden: { opacity: 0, y: 20 },
  show: { opacity: 1, y: 0, transition: { duration: 0.4 } },
};

export default function Features() {
  return (
    <section className={styles.section} id="features">
      <div className={styles.container}>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <h2 className={styles.heading}>Everything you need</h2>
          <p className={styles.subtitle}>
            Veld handles the boilerplate so you can focus on business logic.
          </p>
        </motion.div>

        <motion.div
          className={styles.grid}
          variants={container}
          initial="hidden"
          whileInView="show"
          viewport={{ once: true }}
        >
          {features.map((f) => (
            <motion.div key={f.title} className={styles.card} variants={item}>
              <div className={styles.icon}>{f.icon}</div>
              <h3>{f.title}</h3>
              <p>{f.description}</p>
            </motion.div>
          ))}
        </motion.div>
      </div>
    </section>
  );
}

