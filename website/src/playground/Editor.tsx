import { useImperativeHandle, useLayoutEffect, useMemo, useRef } from 'react';
import type { KeyboardEvent, Ref } from 'react';
import type { Diag } from './types';
import { highlight, type Lang, type Mark } from './highlight';
import styles from './Editor.module.css';

export interface EditorHandle {
  /** Moves the caret to a 1-based position and scrolls it into view. */
  reveal(line: number, col: number): void;
}

interface Props {
  value: string;
  lang: Lang;
  diagnostics?: Diag[];
  readOnly?: boolean;
  ariaLabel: string;
  onChange?: (text: string) => void;
  ref?: Ref<EditorHandle>;
}

function lineStarts(text: string): number[] {
  const starts = [0];
  for (let i = 0; i < text.length; i++) if (text[i] === '\n') starts.push(i + 1);
  return starts;
}

function offsetOf(starts: number[], text: string, line: number, col: number): number {
  const start = starts[Math.max(0, Math.min(line - 1, starts.length - 1))];
  return Math.min(start + Math.max(0, col - 1), text.length);
}

/** Diagnostics as underline ranges. A zero-width span covers the word at its position. */
function marksOf(text: string, diagnostics: Diag[]): Mark[] {
  const starts = lineStarts(text);
  const marks: Mark[] = [];
  for (const d of diagnostics) {
    if (d.line < 1) continue;
    const start = offsetOf(starts, text, d.line, d.col);
    let end = offsetOf(starts, text, d.endLine, d.endCol);
    if (end <= start) {
      end = start;
      while (end < text.length && /\w/.test(text[end])) end++;
      if (end === start) end = Math.min(start + 1, text.length);
    }
    if (end > start) marks.push({ start, end, severity: d.severity });
  }
  return marks;
}

/** A code editor: a transparent textarea over a syntax-highlighted layer, with a line gutter. */
export default function Editor({ value, lang, diagnostics = [], readOnly = false, ariaLabel, onChange, ref }: Props) {
  const area = useRef<HTMLTextAreaElement>(null);
  const caret = useRef<number | null>(null);
  const marks = useMemo(() => marksOf(value, diagnostics), [value, diagnostics]);
  const nodes = useMemo(() => highlight(value, lang, marks), [value, lang, marks]);
  const lines = useMemo(() => value.split('\n').length, [value]);
  const errorLines = useMemo(() => {
    const map = new Map<number, Diag>();
    for (const d of diagnostics) if (d.line > 0 && (!map.has(d.line) || d.severity === 'error')) map.set(d.line, d);
    return map;
  }, [diagnostics]);

  useImperativeHandle(ref, () => ({
    reveal(line, col) {
      const el = area.current;
      if (!el) return;
      const at = offsetOf(lineStarts(el.value), el.value, line, col);
      el.focus({ preventScroll: true });
      el.setSelectionRange(at, at);
      // The textarea spans the whole text, so scroll the surrounding box to the caret's line.
      const box = el.closest(`.${styles.scroll}`);
      const lineHeight = parseFloat(getComputedStyle(el).lineHeight) || 22;
      if (box) box.scrollTop = Math.max(0, (line - 3) * lineHeight);
    },
  }));

  useLayoutEffect(() => {
    if (caret.current !== null && area.current) {
      area.current.setSelectionRange(caret.current, caret.current);
      caret.current = null;
    }
  });

  function edit(el: HTMLTextAreaElement, from: number, to: number, insert: string) {
    const next = el.value.slice(0, from) + insert + el.value.slice(to);
    caret.current = from + insert.length;
    onChange?.(next);
  }

  function onKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (readOnly || e.nativeEvent.isComposing) return;
    const el = e.currentTarget;
    const { selectionStart: s, selectionEnd: t, value: v } = el;
    if (e.key === 'Tab' && !e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      edit(el, s, t, '  ');
    } else if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
      e.preventDefault();
      const lineStart = v.lastIndexOf('\n', s - 1) + 1;
      const indent = /^[ \t]*/.exec(v.slice(lineStart, s))?.[0] ?? '';
      const opens = v.slice(lineStart, s).trimEnd().endsWith('{');
      edit(el, s, t, `\n${indent}${opens ? '  ' : ''}`);
    }
  }

  return (
    <div className={styles.editor}>
      <div className={styles.scroll}>
        <div className={styles.gutter} aria-hidden="true">
          {Array.from({ length: lines }, (_, i) => {
            const d = errorLines.get(i + 1);
            return (
              <div
                key={i}
                className={`${styles.num} ${d ? (d.severity === 'error' ? styles.numError : styles.numWarn) : ''}`}
                title={d ? `${d.code}: ${d.message}` : undefined}
              >
                {i + 1}
              </div>
            );
          })}
        </div>
        <div className={styles.stack}>
          <pre className={styles.layer} aria-hidden="true">
            {nodes}
            {'\n'}
          </pre>
          <textarea
            ref={area}
            className={styles.input}
            value={value}
            readOnly={readOnly}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            autoComplete="off"
            wrap="off"
            aria-label={ariaLabel}
            onChange={(e) => onChange?.(e.target.value)}
            onKeyDown={onKeyDown}
          />
        </div>
      </div>
    </div>
  );
}
