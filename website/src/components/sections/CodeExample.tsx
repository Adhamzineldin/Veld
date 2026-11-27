import { motion } from 'framer-motion';
import { highlightVeld, highlightTS } from '../SyntaxHighlighter';
import styles from './CodeExample.module.css';

// Everything below is real code from examples/hello in the Veld repository.
const veldCode = `/// A greeting for one person.
model Greeting {
  message: string
}

/// Says hello.
@auth(none)
service Hello at "/hello" {
  /// Greets the person with the given name.
  get "/{name}" greet(name: string) -> Greeting | NotFound
}`;

const clientCode = `import { HelloClient, NotFound } from '@veld/web';

const client = new HelloClient({ baseUrl: 'http://127.0.0.1:3000' });

// Fully typed: the request, the result and the error
try {
  const greeting = await client.greet({ name: 'Ada' });
  console.log(greeting.message);
} catch (e) {
  if (NotFound.matches(e)) {
    // the contract's typed NotFound
  }
}`;

const serverCode = `// server/src/main.ts: implement the generated HelloService and mount its handler on Express.
import express from 'express';
import { NotFound, createHelloHandler, toExpress } from '@veld/api';
import type { GreetRequest, HelloService } from '@veld/api';

export const hello: HelloService = {
  async greet({ name }: GreetRequest) {
    if (name.toLowerCase() === 'nobody') {
      throw new NotFound({ resource: name });
    }
    return { message: \`Hello, \${name}!\` };
  },
};

const app = express();
app.use(toExpress(createHelloHandler(hello)));`;

export default function CodeExample() {
  return (
    <section className={styles.section} id="example">
      <div className={styles.container}>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <h2 className={styles.heading}>See it in action</h2>
          <p className={styles.subtitle}>
            One contract, a typed server and a typed client. This is the hello example from the repository.
          </p>
        </motion.div>

        <motion.div
          className={styles.grid}
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5, delay: 0.1 }}
        >
          <div className={styles.codeBlock}>
            <div className={styles.title}>
              <span className={styles.dot} style={{ background: '#f85149' }} />
              <span className={styles.dot} style={{ background: '#f0883e' }} />
              <span className={styles.dot} style={{ background: 'var(--accent2)' }} />
              <span>You write — hello.veld</span>
            </div>
            <pre className={styles.pre}>{highlightVeld(veldCode)}</pre>
          </div>

          <div className={styles.codeBlock}>
            <div className={styles.title}>
              <span className={styles.dot} style={{ background: '#f85149' }} />
              <span className={styles.dot} style={{ background: '#f0883e' }} />
              <span className={styles.dot} style={{ background: 'var(--accent2)' }} />
              <span>You write — a typed client call</span>
            </div>
            <pre className={styles.pre}>{highlightTS(clientCode)}</pre>
          </div>
        </motion.div>

        <motion.div
          className={styles.fullBlock}
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5, delay: 0.2 }}
        >
          <div className={styles.codeBlock}>
            <div className={styles.title}>
              <span className={styles.dot} style={{ background: '#f85149' }} />
              <span className={styles.dot} style={{ background: '#f0883e' }} />
              <span className={styles.dot} style={{ background: 'var(--accent2)' }} />
              <span>You write — the Express server</span>
            </div>
            <pre className={styles.pre}>{highlightTS(serverCode)}</pre>
          </div>
        </motion.div>
      </div>
    </section>
  );
}

