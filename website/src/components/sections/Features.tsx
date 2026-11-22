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
      'Generated code imports no package. Only the adapter for the framework you chose (React, Vue, NestJS, Spring) imports that framework.',
  },
  {
    icon: <Layers size={22} />,
    title: 'Your Framework',
    description:
      'Server adapters for Express, Fastify, Hono, node:http, NestJS, the JDK HTTP server and Spring Boot 3. The Node adapters use structural types, so there is no lock-in.',
  },
  {
    icon: <ShieldCheck size={22} />,
    title: 'Validation Everywhere',
    description:
      'Constraints from the contract are enforced on the server (422 with issues) and in the client before the request is sent.',
  },
  {
    icon: <AlertTriangle size={22} />,
    title: 'Typed Errors',
    description:
      'Errors are declarations. They travel as RFC 9457 problem+json and arrive as typed classes, in TypeScript and in Java.',
  },
  {
    icon: <GitBranch size={22} />,
    title: 'Same Error, Everywhere',
    description:
      'Signatures and same() recognise an error across packages and languages: a Node NotFound is the same NotFound in the Java service that calls it.',
  },
  {
    icon: <Globe size={22} />,
    title: 'Microservices and Streaming',
    description:
      'Workspaces with consumes (cycles allowed), a generated client per consumed service, plus Server-Sent Events and WebSockets.',
  },
  {
    icon: <FileCode size={22} />,
    title: 'Safe, Deterministic Output',
    description:
      'Same contract, same bytes. Veld writes through a staged manifest and never overwrites a file you edited.',
  },
  {
    icon: <Eye size={22} />,
    title: 'Breaking-Change Diff',
    description:
      'veld diff compares against veld.lock.ir.json or a git ref and, with --strict, fails CI on a breaking change. veld export openapi gives OpenAPI 3.1.',
  },
  {
    icon: <Terminal size={22} />,
    title: 'IDE Support',
    description:
      'VS Code and JetBrains plugins, both on one language server (veld lsp): diagnostics, hover, completion, rename, formatting.',
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

