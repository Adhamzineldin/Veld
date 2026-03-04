import { motion } from 'framer-motion';
import styles from './CodeExample.module.css';

const veldCode = `model User {
  id:    uuid
  email: string
  name:  string
  role:  Role   @default(user)
}

enum Role { admin user guest }

module Users {
  prefix: /api/v1

  action GetUser {
    method: GET
    path:   /users/:id
    output: User
    errors: [NotFound]
  }

  action CreateUser {
    method: POST
    path:   /users
    input:  CreateUserInput
    output: User
  }
}`;

const sdkCode = `import { api } from '@veld/client';
import { isErrorCode } from '@veld/client/errors';

// Fully typed — autocomplete for methods,
// params, and return types

const user = await api.Users.getUser('user-123');
// ^? Promise<User>

try {
  await api.Users.createUser({
    email: 'alice@co.dev',
    name:  'Alice',
  });
} catch (err) {
  if (isErrorCode(err,
    api.Users.errors.getUser.notFound)) {
    // handle typed error
  }
}`;

const backendCode = `// generated/routes/users.routes.ts
import { IUsersService } from '../interfaces/IUsersService';
import { CreateUserInputSchema } from '../schemas/schemas';

export function registerUsersRoutes(
  router: any,
  service: IUsersService
) {
  router.get('/api/v1/users/:id', async (req, res) => {
    try {
      const result = await service.getUser(req.params.id);
      res.status(200).json(result);
    } catch (err) {
      res.status(500).json({ error: 'Internal error' });
    }
  });

  router.post('/api/v1/users', async (req, res) => {
    try {
      const input = CreateUserInputSchema.parse(req.body);
      const result = await service.createUser(input);
      res.status(201).json(result);
    } catch (err) {
      // ZodError → 400 with validation details
    }
  });
}`;

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
            Write a contract on the left, get typed code on the right. It's that simple.
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
              <span>You write — users.veld</span>
            </div>
            <pre className={styles.pre}>{veldCode}</pre>
          </div>

          <div className={styles.codeBlock}>
            <div className={styles.title}>
              <span className={styles.dot} style={{ background: '#f85149' }} />
              <span className={styles.dot} style={{ background: '#f0883e' }} />
              <span className={styles.dot} style={{ background: 'var(--accent2)' }} />
              <span>Veld generates — Frontend SDK</span>
            </div>
            <pre className={styles.pre}>{sdkCode}</pre>
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
              <span>Veld generates — Backend Routes (Node.js)</span>
            </div>
            <pre className={styles.pre}>{backendCode}</pre>
          </div>
        </motion.div>
      </div>
    </section>
  );
}

