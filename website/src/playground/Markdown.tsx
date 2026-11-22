import type { ReactNode } from 'react';
import { highlight, type Lang } from './highlight';

/** The fence languages the generated READMEs use, mapped to the highlighter's. */
function fenceLang(info: string): Lang {
  switch (info.trim().toLowerCase()) {
    case 'ts':
    case 'tsx':
    case 'typescript':
    case 'js':
    case 'javascript':
      return 'ts';
    case 'java':
      return 'java';
    case 'rust':
    case 'rs':
      return 'rust';
    case 'toml':
      return 'toml';
    case 'json':
      return 'json';
    case 'veld':
      return 'veld';
    default:
      return 'text';
  }
}

/** `code`, **bold** and plain text of one line or paragraph. */
function inline(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  const pattern = /`([^`]+)`|\*\*([^*]+)\*\*/g;
  let last = 0;
  let key = 0;
  for (const m of text.matchAll(pattern)) {
    const at = m.index ?? 0;
    if (at > last) out.push(text.slice(last, at));
    out.push(m[1] !== undefined ? <code key={key++}>{m[1]}</code> : <strong key={key++}>{m[2]}</strong>);
    last = at + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

type Block =
  | { kind: 'heading'; level: number; text: string }
  | { kind: 'code'; lang: Lang; text: string }
  | { kind: 'list'; items: string[] }
  | { kind: 'para'; text: string };

/** Splits the small Markdown subset of generated READMEs into blocks. */
function parseMarkdown(source: string): Block[] {
  const lines = source.replace(/\r\n/g, '\n').split('\n');
  const blocks: Block[] = [];
  let para: string[] = [];
  let list: string[] | null = null;
  const flush = (): void => {
    if (para.length > 0) blocks.push({ kind: 'para', text: para.join(' ') });
    if (list) blocks.push({ kind: 'list', items: list });
    para = [];
    list = null;
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const fence = /^```(.*)$/.exec(line);
    if (fence) {
      flush();
      const body: string[] = [];
      for (i++; i < lines.length && !lines[i].startsWith('```'); i++) body.push(lines[i]);
      blocks.push({ kind: 'code', lang: fenceLang(fence[1]), text: body.join('\n') });
      continue;
    }
    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    const item = /^\s*[-*]\s+(.*)$/.exec(line);
    if (heading) {
      flush();
      blocks.push({ kind: 'heading', level: heading[1].length, text: heading[2] });
    } else if (item) {
      if (para.length > 0) flush();
      (list ??= []).push(item[1]);
    } else if (line.trim() === '') {
      flush();
    } else if (list && /^\s+/.test(line)) {
      list[list.length - 1] += ' ' + line.trim();
    } else {
      if (list) flush();
      para.push(line.trim());
    }
  }
  flush();
  return blocks;
}

/** Renders a generated README: headings, paragraphs, lists and highlighted code blocks. */
export function Markdown({ source, className }: { source: string; className?: string }) {
  return (
    <div className={className}>
      {parseMarkdown(source).map((b, i) => {
        switch (b.kind) {
          case 'heading': {
            const Tag = (b.level <= 2 ? 'h4' : 'h5') as 'h4' | 'h5';
            return <Tag key={i}>{inline(b.text)}</Tag>;
          }
          case 'code':
            return (
              <pre key={i}>
                <code>{highlight(b.text, b.lang)}</code>
              </pre>
            );
          case 'list':
            return (
              <ul key={i}>
                {b.items.map((it, j) => (
                  <li key={j}>{inline(it)}</li>
                ))}
              </ul>
            );
          case 'para':
            return <p key={i}>{inline(b.text)}</p>;
        }
      })}
    </div>
  );
}
