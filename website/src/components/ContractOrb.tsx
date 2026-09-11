import { useEffect, useState } from 'react';
import type { CSSProperties } from 'react';
import styles from './ContractOrb.module.css';

type PhaseKey = 'contract' | 'backend' | 'frontend' | 'sync';

const PHASES: { key: PhaseKey; step: string; title: string; body: string }[] = [
  {
    key: 'contract',
    step: '01',
    title: 'Write one contract',
    body: 'Models, modules and actions live in .veld files — a single source of truth.',
  },
  {
    key: 'backend',
    step: '02',
    title: 'Generate the backend',
    body: 'Typed interfaces, validated routes and schemas — Node, Go, Python, Rust, Java…',
  },
  {
    key: 'frontend',
    step: '03',
    title: 'Generate the frontend',
    body: 'A fetch-based SDK carrying the exact same types — React, Vue, Swift, Kotlin…',
  },
  {
    key: 'sync',
    step: '04',
    title: 'Both ends stay in sync',
    body: 'Change the contract, regenerate — backend and frontend can never drift apart.',
  },
];

const CYCLE = 4000;
const DUR = `${CYCLE}ms`;

const PATH_CONTRACT = 'M280 82 L280 167';
const PATH_BACKEND = 'M188 359 L84 442';
const PATH_FRONTEND = 'M372 359 L476 442';

/** A data packet travelling a connector, then waiting out the rest of the cycle. */
function Pulse({ path, color, leg }: { path: string; color: string; leg: 'in' | 'out' }) {
  const keyPoints = leg === 'in' ? '0;1;1' : '0;0;1;1';
  const keyTimes = leg === 'in' ? '0;0.35;1' : '0;0.35;0.75;1';
  // keyTimes must start at 0 and end at 1, or the browser drops the animation.
  const fadeTimes = leg === 'in' ? '0;0.04;0.30;0.36;1' : '0;0.34;0.40;0.74;0.80;1';
  const core = leg === 'in' ? '0;1;1;0;0' : '0;0;1;1;0;0';
  const halo = leg === 'in' ? '0;0.4;0.4;0;0' : '0;0;0.4;0.4;0;0';

  const motion = (
    <animateMotion
      dur={DUR}
      repeatCount="indefinite"
      calcMode="linear"
      keyPoints={keyPoints}
      keyTimes={keyTimes}
      path={path}
    />
  );

  return (
    <g className={styles.pulseGroup}>
      <circle r="12" fill={color} opacity="0" filter="url(#veldSoft)">
        {motion}
        <animate
          attributeName="opacity"
          dur={DUR}
          repeatCount="indefinite"
          values={halo}
          keyTimes={fadeTimes}
        />
      </circle>
      <circle r="4.5" fill="#ffffff" opacity="0">
        {motion}
        <animate
          attributeName="opacity"
          dur={DUR}
          repeatCount="indefinite"
          values={core}
          keyTimes={fadeTimes}
        />
      </circle>
    </g>
  );
}

function Node({
  x,
  y,
  glyph,
  label,
  sub,
  accent,
  active,
  side = 'below',
}: {
  x: number;
  y: number;
  glyph: string;
  label: string;
  sub: string;
  accent: string;
  active: boolean;
  /** 'below' stacks the label under the marker, 'right' sets it beside — used where a connector would cross it. */
  side?: 'below' | 'right';
}) {
  const anchor = side === 'right' ? 'start' : 'middle';
  const lx = side === 'right' ? x + 30 : x;
  const ly = side === 'right' ? y - 2 : y + 46;
  const sy = side === 'right' ? y + 14 : y + 64;

  return (
    <g
      className={`${styles.node} ${active ? styles.nodeActive : ''}`}
      style={{ '--node-accent': accent } as CSSProperties}
    >
      <rect x={x - 18} y={y - 18} width="36" height="36" rx="11" className={styles.nodeBox} />
      <text x={x} y={y + 6} textAnchor="middle" className={styles.nodeGlyph}>
        {glyph}
      </text>
      <text x={lx} y={ly} textAnchor={anchor} className={styles.nodeLabel}>
        {label}
      </text>
      <text x={lx} y={sy} textAnchor={anchor} className={styles.nodeSub}>
        {sub}
      </text>
    </g>
  );
}

export default function ContractOrb() {
  const [phase, setPhase] = useState(0);

  useEffect(() => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    const id = setInterval(() => setPhase((p) => (p + 1) % PHASES.length), CYCLE);
    return () => clearInterval(id);
  }, []);

  const current = PHASES[phase];
  const lit = (key: PhaseKey) => current.key === key || current.key === 'sync';

  return (
    <div className={styles.wrap}>
      <svg
        className={styles.svg}
        viewBox="0 0 560 600"
        role="img"
        aria-label="A .veld contract at the centre generating a typed backend on one side and a typed frontend SDK on the other"
      >
        <defs>
          <radialGradient id="veldSphere" cx="34%" cy="28%" r="82%">
            <stop offset="0%" stopColor="#daedff" />
            <stop offset="16%" stopColor="#7db4ff" />
            <stop offset="42%" stopColor="#2f6bf5" />
            <stop offset="74%" stopColor="#2b35c8" />
            <stop offset="100%" stopColor="#151b70" />
          </radialGradient>
          <radialGradient id="veldHalo" cx="50%" cy="50%" r="50%">
            <stop offset="0%" stopColor="#3b82ff" stopOpacity="0.5" />
            <stop offset="52%" stopColor="#6b6cff" stopOpacity="0.14" />
            <stop offset="100%" stopColor="#6b6cff" stopOpacity="0" />
          </radialGradient>
          <radialGradient id="veldSpec" cx="50%" cy="50%" r="50%">
            <stop offset="0%" stopColor="#ffffff" stopOpacity="0.65" />
            <stop offset="100%" stopColor="#ffffff" stopOpacity="0" />
          </radialGradient>
          <linearGradient id="veldRingA" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0%" stopColor="#35d6ff" stopOpacity="0" />
            <stop offset="34%" stopColor="#35d6ff" stopOpacity="0.85" />
            <stop offset="68%" stopColor="#a855f7" stopOpacity="0.65" />
            <stop offset="100%" stopColor="#a855f7" stopOpacity="0" />
          </linearGradient>
          <linearGradient id="veldRingB" x1="0" y1="1" x2="1" y2="0">
            <stop offset="0%" stopColor="#a855f7" stopOpacity="0" />
            <stop offset="50%" stopColor="#7c5cff" stopOpacity="0.75" />
            <stop offset="100%" stopColor="#35d6ff" stopOpacity="0" />
          </linearGradient>
          <filter id="veldSoft" x="-150%" y="-150%" width="400%" height="400%">
            <feGaussianBlur stdDeviation="5" />
          </filter>
        </defs>

        <circle cx="280" cy="285" r="252" fill="url(#veldHalo)" className={styles.halo} />

        <g className={styles.orbits}>
          <circle cx="280" cy="285" r="238" className={styles.dashRing} />
          <circle cx="280" cy="285" r="197" className={styles.dashRingFine} />
          <g className={styles.spinSlow}>
            <ellipse
              cx="280"
              cy="285"
              rx="232"
              ry="88"
              transform="rotate(-18 280 285)"
              stroke="url(#veldRingA)"
              fill="none"
              strokeWidth="1.6"
            />
          </g>
          <g className={styles.spinMed}>
            <ellipse
              cx="280"
              cy="285"
              rx="104"
              ry="226"
              transform="rotate(26 280 285)"
              stroke="url(#veldRingB)"
              fill="none"
              strokeWidth="1.4"
            />
          </g>
        </g>

        <g className={styles.links}>
          <path
            d={PATH_CONTRACT}
            className={`${styles.link} ${lit('contract') ? styles.linkActive : ''}`}
          />
          <path
            d={PATH_BACKEND}
            className={`${styles.link} ${lit('backend') ? styles.linkActive : ''}`}
          />
          <path
            d={PATH_FRONTEND}
            className={`${styles.link} ${lit('frontend') ? styles.linkActive : ''}`}
          />
        </g>

        <g className={styles.sphere}>
          <circle cx="280" cy="285" r="118" className={styles.echo} />
          <circle cx="280" cy="285" r="118" className={styles.echo2} />
          <circle cx="280" cy="285" r="118" fill="url(#veldSphere)" />
          <ellipse cx="240" cy="236" rx="54" ry="38" fill="url(#veldSpec)" />
          <circle
            cx="280"
            cy="285"
            r="118"
            fill="none"
            stroke="rgba(255,255,255,0.22)"
            strokeWidth="1"
          />
          <rect
            x="256"
            y="261"
            width="48"
            height="48"
            rx="13"
            fill="rgba(255,255,255,0.10)"
            stroke="rgba(255,255,255,0.55)"
            strokeWidth="1.2"
          />
          <text x="280" y="293" textAnchor="middle" className={styles.coreGlyph}>
            {'{ }'}
          </text>
        </g>

        <g className={styles.spinFront}>
          <path
            d="M 100 330 A 180 180 0 0 0 460 330"
            fill="none"
            stroke="#35d6ff"
            strokeOpacity="0.5"
            strokeWidth="2"
            strokeLinecap="round"
          />
        </g>

        <Pulse path={PATH_CONTRACT} color="#8ed0ff" leg="in" />
        <Pulse path={PATH_BACKEND} color="#4d8dff" leg="out" />
        <Pulse path={PATH_FRONTEND} color="#c08bff" leg="out" />

        <Node
          x={280}
          y={62}
          glyph="⬡"
          label="CONTRACT"
          sub="app.veld"
          accent="#35d6ff"
          active={lit('contract')}
          side="right"
        />
        <Node
          x={84}
          y={452}
          glyph="⚙"
          label="BACKEND"
          sub="routes · interfaces"
          accent="#4d8dff"
          active={lit('backend')}
        />
        <Node
          x={476}
          y={452}
          glyph="◍"
          label="FRONTEND"
          sub="typed SDK"
          accent="#c08bff"
          active={lit('frontend')}
        />

        <text x="52" y="196" className={styles.meta}>
          ZERO DEPS
        </text>
        <text x="424" y="176" className={styles.meta}>
          SCHEMA 1.0
        </text>
        <text x="238" y="578" className={styles.meta}>
          TYPES · ROUTES · SDK
        </text>
        <circle cx="122" cy="190" r="2.5" className={styles.dot} />
        <circle cx="412" cy="170" r="2.5" className={styles.dot} />
      </svg>

      <div className={styles.caption}>
        <div className={styles.steps}>
          {PHASES.map((p, i) => (
            <button
              key={p.key}
              type="button"
              className={`${styles.stepDot} ${i === phase ? styles.stepDotActive : ''}`}
              onClick={() => setPhase(i)}
              aria-label={`${p.step} — ${p.title}`}
            />
          ))}
        </div>
        <div key={current.key} className={styles.captionBody}>
          <span className={styles.captionStep}>{current.step}</span>
          <div>
            <strong className={styles.captionTitle}>{current.title}</strong>
            <p className={styles.captionText}>{current.body}</p>
          </div>
        </div>
      </div>
    </div>
  );
}
