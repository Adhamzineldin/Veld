import { motion } from 'framer-motion';
import { Github, Crown, Code2, Sparkles } from 'lucide-react';
import styles from './Contributors.module.css';

const contributors = [
  {
    name: 'Adham Zineldin',
    role: 'Lead Developer & Creator',
    avatar: 'https://github.com/Adhamzineldin.png',
    bio: 'Designed and built the entire Veld ecosystem — compiler, CLI, emitters, IDE plugins, and package publishing pipeline.',
    badge: 'founder',
    links: {
      github: 'https://github.com/Adhamzineldin',
    },
  },
  {
    name: 'Eyad Gamal',
    role: 'Core Contributor',
    avatar: 'https://ui-avatars.com/api/?name=Eyad+Gamal&background=30363d&color=e6edf3&size=200&bold=true',
    bio: 'Contributed to core features, testing, and documentation. Key collaborator on the project architecture and design decisions.',
    badge: 'core',
    links: {
      github: '#',
    },
  },
];

function BadgeIcon({ type }: { type: string }) {
  if (type === 'founder') return <Crown size={14} />;
  if (type === 'core') return <Code2 size={14} />;
  return <Sparkles size={14} />;
}

function badgeLabel(type: string) {
  if (type === 'founder') return 'Founder';
  if (type === 'core') return 'Core';
  return 'Contributor';
}

export default function Contributors() {
  return (
    <section className={styles.section} id="contributors">
      <div className={styles.container}>
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5 }}
        >
          <h2 className={styles.heading}>Built by</h2>
          <p className={styles.subtitle}>
            The people behind Veld — turning an idea into a real tool.
          </p>
        </motion.div>

        <div className={styles.grid}>
          {contributors.map((c, i) => (
            <motion.div
              key={c.name}
              className={styles.card}
              initial={{ opacity: 0, y: 30 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ duration: 0.5, delay: i * 0.1 }}
            >
              <div className={styles.cardHeader}>
                <img
                  src={c.avatar}
                  alt={c.name}
                  className={styles.avatar}
                  loading="lazy"
                />
                <div>
                  <h3 className={styles.name}>{c.name}</h3>
                  <p className={styles.role}>{c.role}</p>
                  <span className={`${styles.badge} ${styles[c.badge]}`}>
                    <BadgeIcon type={c.badge} />
                    {badgeLabel(c.badge)}
                  </span>
                </div>
              </div>
              <p className={styles.bio}>{c.bio}</p>
              <div className={styles.links}>
                {c.links.github && (
                  <a href={c.links.github} target="_blank" rel="noopener noreferrer" title="GitHub">
                    <Github size={18} />
                  </a>
                )}
              </div>
            </motion.div>
          ))}
        </div>

        <motion.div
          className={styles.contribute}
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5, delay: 0.3 }}
        >
          <h4>Want to contribute?</h4>
          <p>
            Veld is open source. Check out the{' '}
            <a
              href="https://github.com/Adhamzineldin/Veld"
              target="_blank"
              rel="noopener noreferrer"
            >
              GitHub repository
            </a>{' '}
            — issues, PRs, and new emitters are always welcome.
          </p>
        </motion.div>
      </div>
    </section>
  );
}


