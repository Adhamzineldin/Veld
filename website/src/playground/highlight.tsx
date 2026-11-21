import type { ReactNode } from 'react';
import styles from './highlight.module.css';

export type Lang = 'veld' | 'ts' | 'java' | 'rust' | 'json' | 'toml' | 'md' | 'text';

/** A text range to underline (offsets into the source), for diagnostics. */
export interface Mark {
  start: number;
  end: number;
  severity: 'error' | 'warning';
}

export function langOf(path: string): Lang {
  const ext = path.slice(path.lastIndexOf('.') + 1).toLowerCase();
  switch (ext) {
    case 'veld':
      return 'veld';
    case 'ts':
    case 'tsx':
    case 'js':
    case 'mjs':
    case 'vue':
      return 'ts';
    case 'java':
      return 'java';
    case 'rs':
      return 'rust';
    case 'json':
      return 'json';
    case 'toml':
      return 'toml';
    case 'md':
      return 'md';
    default:
      return 'text';
  }
}

const words = (list: string) => `\\b(?:${list.split(' ').join('|')})\\b`;

const TS_KEYWORDS = words(
  'import export from default const let var async await function return if else try catch finally throw new interface type class extends implements readonly enum namespace declare typeof instanceof in of for while do switch case break continue void null undefined true false this as satisfies keyof static private public protected abstract',
);
const JAVA_KEYWORDS = words(
  'package import public private protected static final abstract class interface enum record extends implements return if else try catch finally throw throws new void null true false this super switch case break default for while do instanceof var sealed permits synchronized volatile',
);
const RUST_KEYWORDS = words(
  'use pub mod fn let mut const static struct enum impl trait type where for in if else match loop while return break continue as ref move async await dyn crate self Self super true false unsafe extern',
);
const VELD_KEYWORDS = words('model enum error union service const use as at extends stream true false null');
const VELD_BUILTINS = words('string int long float decimal bool date datetime time uuid bytes json');
const VELD_VERBS = '\\b(?:get|post|put|patch|delete|head|options|ws)(?=\\s+")';

// Each rule is [class, source]. The first rule that matches at a position wins.
type Rule = [string, string];

const STRING_DQ = '"(?:\\\\.|[^"\\\\\\n])*"';
const STRING_SQ = "'(?:\\\\.|[^'\\\\\\n])*'";
const NUMBER = '\\b\\d[\\d_]*(?:\\.\\d+)?(?:[eE][+-]?\\d+)?\\b';

const RULES: Record<Lang, Rule[]> = {
  veld: [
    ['doc', '///[^\\n]*'],
    ['comment', '//[^\\n]*'],
    ['string', STRING_DQ],
    ['annotation', '@\\w+'],
    ['verb', VELD_VERBS],
    ['keyword', VELD_KEYWORDS],
    ['builtin', VELD_BUILTINS],
    ['type', '\\b[A-Z][A-Za-z0-9]*\\b'],
    ['number', NUMBER],
  ],
  ts: [
    ['comment', '//[^\\n]*|/\\*[\\s\\S]*?\\*/'],
    ['string', `${STRING_DQ}|${STRING_SQ}|\`(?:\\\\.|[^\`\\\\])*\``],
    ['keyword', TS_KEYWORDS],
    ['type', '\\b[A-Z][A-Za-z0-9]*\\b'],
    ['number', NUMBER],
  ],
  java: [
    ['comment', '//[^\\n]*|/\\*[\\s\\S]*?\\*/'],
    ['string', STRING_DQ],
    ['annotation', '@\\w+'],
    ['keyword', JAVA_KEYWORDS],
    ['type', '\\b[A-Z][A-Za-z0-9]*\\b'],
    ['number', NUMBER],
  ],
  rust: [
    ['comment', '//[^\\n]*|/\\*[\\s\\S]*?\\*/'],
    ['string', STRING_DQ],
    ['annotation', '#!?\\[[^\\]\\n]*\\]'],
    ['keyword', RUST_KEYWORDS],
    ['type', '\\b[A-Z][A-Za-z0-9]*\\b'],
    ['number', NUMBER],
  ],
  json: [
    ['key', `${STRING_DQ}(?=\\s*:)`],
    ['string', STRING_DQ],
    ['keyword', words('true false null')],
    ['number', '-?\\b\\d+(?:\\.\\d+)?(?:[eE][+-]?\\d+)?\\b'],
  ],
  toml: [
    ['comment', '#[^\\n]*'],
    ['type', '^\\[[^\\]\\n]+\\]'],
    ['string', STRING_DQ],
    ['key', '^[\\w.-]+(?=\\s*=)'],
    ['keyword', words('true false')],
    ['number', NUMBER],
  ],
  md: [
    ['type', '^#{1,6} [^\\n]*'],
    ['string', '`[^`\\n]+`'],
    ['annotation', '\\*\\*[^*\\n]+\\*\\*'],
  ],
  text: [],
};

const compiled = new Map<Lang, RegExp>();

function matcher(lang: Lang): RegExp | null {
  const rules = RULES[lang];
  if (rules.length === 0) return null;
  let re = compiled.get(lang);
  if (!re) {
    re = new RegExp(rules.map(([, src], i) => `(?<r${i}>${src})`).join('|'), 'gm');
    compiled.set(lang, re);
  }
  re.lastIndex = 0;
  return re;
}

interface Token {
  start: number;
  end: number;
  cls: string | null;
}

/** Splits `text` into classified tokens that cover it completely. */
export function tokenize(text: string, lang: Lang): Token[] {
  const re = matcher(lang);
  if (!re) return [{ start: 0, end: text.length, cls: null }];
  const rules = RULES[lang];
  const out: Token[] = [];
  let pos = 0;
  for (let m = re.exec(text); m; m = re.exec(text)) {
    if (m[0].length === 0) {
      re.lastIndex++;
      continue;
    }
    const i = rules.findIndex((_, k) => m.groups?.[`r${k}`] !== undefined);
    if (m.index > pos) out.push({ start: pos, end: m.index, cls: null });
    out.push({ start: m.index, end: m.index + m[0].length, cls: rules[i][0] });
    pos = m.index + m[0].length;
  }
  if (pos < text.length) out.push({ start: pos, end: text.length, cls: null });
  return out;
}

/**
 * Highlights `text`; the result is a flat list of spans that spells exactly `text`, so it can
 * sit behind a textarea. `marks` are underlined (diagnostics).
 */
export function highlight(text: string, lang: Lang, marks: Mark[] = []): ReactNode[] {
  const tokens = tokenize(text, lang);
  const cuts = new Set<number>();
  for (const m of marks) {
    cuts.add(m.start);
    cuts.add(m.end);
  }
  const out: ReactNode[] = [];
  let key = 0;
  for (const t of tokens) {
    let from = t.start;
    const inside = [...cuts].filter((c) => c > t.start && c < t.end).sort((a, b) => a - b);
    for (const to of [...inside, t.end]) {
      const piece = text.slice(from, to);
      const mark = marks.find((m) => from >= m.start && to <= m.end);
      const cls = [t.cls ? styles[t.cls] : '', mark ? styles[mark.severity === 'error' ? 'squiggleError' : 'squiggleWarn'] : '']
        .filter(Boolean)
        .join(' ');
      out.push(cls ? <span key={key++} className={cls}>{piece}</span> : piece);
      from = to;
    }
  }
  return out;
}
