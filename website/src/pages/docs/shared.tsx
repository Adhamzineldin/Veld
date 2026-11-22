import type { ReactNode } from 'react';
import { highlightVeld, highlightTS } from '../../components/SyntaxHighlighter';
import { styles } from './ctx';

export type Lang = 'veld' | 'ts' | 'bash' | 'toml' | 'java' | 'text';

/** A titled code block. `veld` and `ts` are highlighted, everything else is shown as is. */
export function CodeBlock({ title, children, lang }: { title?: string; children: string; lang?: Lang }) {
  const rendered = lang === 'veld' ? highlightVeld(children) : lang === 'ts' ? highlightTS(children) : null;
  return (
    <div className={styles.codeBlock}>
      {title && (
        <div className={styles.codeHeader}>
          <span className={styles.codeDot} style={{ background: '#f85149' }} />
          <span className={styles.codeDot} style={{ background: '#f0883e' }} />
          <span className={styles.codeDot} style={{ background: '#3fb950' }} />
          <span className={styles.codeHeaderTitle}>{title}</span>
        </div>
      )}
      <pre className={styles.codeContent}>{rendered ?? children}</pre>
    </div>
  );
}

/** A top-level docs section. `id` is the anchor used by the sidebar. */
export function Section({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section id={id} className={styles.section}>
      <h2 className={styles.sectionTitle}>{title}</h2>
      {children}
    </section>
  );
}

/** A sub-heading that is also a sidebar anchor. */
export function Sub({ id, children }: { id?: string; children: ReactNode }) {
  return (
    <h3 id={id} className={styles.sectionSubtitle} style={id ? { scrollMarginTop: 80 } : undefined}>
      {children}
    </h3>
  );
}

export function P({ children }: { children: ReactNode }) {
  return <p className={styles.sectionDesc}>{children}</p>;
}

export function Info({ children }: { children: ReactNode }) {
  return <div className={styles.infoCard}>{children}</div>;
}

export function Warn({ children }: { children: ReactNode }) {
  return <div className={styles.warningCard}>{children}</div>;
}

export function List({ children }: { children: ReactNode }) {
  return <ul className={styles.featureList}>{children}</ul>;
}

export function Tree({ children }: { children: string }) {
  return <div className={styles.tree}>{children}</div>;
}

/** A small table. Cells are plain strings, or nodes for inline markup. */
export function Table({ head, rows }: { head: string[]; rows: ReactNode[][] }) {
  return (
    <table className={styles.table}>
      <thead>
        <tr>
          {head.map((h) => (
            <th key={h}>{h}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((r, i) => (
          <tr key={i}>
            {r.map((c, j) => (
              <td key={j}>{c}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Marks a feature that is new in Veld 2.0. */
export function NewBadge() {
  return (
    <span
      style={{
        marginLeft: 8,
        padding: '2px 8px',
        borderRadius: 999,
        fontSize: '0.68rem',
        fontWeight: 700,
        letterSpacing: '0.04em',
        textTransform: 'uppercase',
        background: 'rgba(63,185,80,0.15)',
        color: 'var(--accent2)',
        verticalAlign: 'middle',
      }}
    >
      new in 2.0
    </span>
  );
}

