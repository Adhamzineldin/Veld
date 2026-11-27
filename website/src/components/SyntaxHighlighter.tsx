import React from 'react';

// ── Color tokens (matching the index.html theme) ──
// Keywords:   #d2a8ff  (purple - accent3)
// Types:      #58a6ff  (blue - accent)
// Strings:    #3fb950  (green - accent2)
// Comments:   #8b949e  (gray - fg2)
// Functions:  #d2a8ff  (purple)
// Numbers:    #79c0ff  (light blue)
// Punctuation:#e6edf3  (white - fg)

type Token = { text: string; color?: string };

// ── Veld syntax highlighter ──

export function highlightVeld(code: string): React.ReactNode[] {
  return code.split('\n').map((line, li) => {
    const tokens: Token[] = [];
    let rest = line;

    // Comments
    const commentIdx = rest.indexOf('//');
    let comment = '';
    if (commentIdx !== -1) {
      comment = rest.slice(commentIdx);
      rest = rest.slice(0, commentIdx);
    }

    // Tokenize
    const veldKeywords = /\b(model|enum|error|union|service|const|use|as|at|extends|stream|true|false|null)\b/g;
    const veldBuiltinTypes = /\b(string|int|long|float|decimal|bool|date|datetime|time|uuid|bytes|json)\b/g;
    const veldUserTypes = /\b([A-Z][A-Za-z0-9]*)\b/g;
    const veldMethods = /\b(get|post|put|patch|delete|head|options|ws)(?=\s+")/g;
    const decorators = /@\w+(?:\([^)]*\))?/g;
    const strings = /"[^"]*"/g;
    const veldArraySuffix = /\[\]/g;

    // Build a list of colored ranges
    const ranges: { start: number; end: number; color: string }[] = [];

    let m: RegExpExecArray | null;
    while ((m = veldKeywords.exec(rest)) !== null) {
      ranges.push({ start: m.index, end: m.index + m[0].length, color: '#d2a8ff' });
    }
    // The words of business rules. Most are contextual, so only the ones that start a line (and are
    // not a field name, as in `message: string`) and `else` / `when` are coloured.
    const ruleStart = /^(\s*)(rule|require|ensure|fact|allow|transition|flow|final|principal|policy|permit|forbid|example|given|returns|message)\b(?!\s*[:?])/;
    const ruleWords = /\b(else|when)\b/g;
    const lead = ruleStart.exec(rest);
    if (lead) {
      const start = lead[1].length;
      ranges.push({ start, end: start + lead[2].length, color: '#d2a8ff' });
    }
    while ((m = ruleWords.exec(rest)) !== null) {
      ranges.push({ start: m.index, end: m.index + m[0].length, color: '#d2a8ff' });
    }
    while ((m = veldBuiltinTypes.exec(rest)) !== null) {
      ranges.push({ start: m.index, end: m.index + m[0].length, color: '#58a6ff' });
    }
    while ((m = veldUserTypes.exec(rest)) !== null) {
      // PascalCase words that aren't keywords → type color (blue)
      const word = m[0];
      ranges.push({ start: m.index, end: m.index + word.length, color: '#58a6ff' });
    }
    while ((m = veldMethods.exec(rest)) !== null) {
      ranges.push({ start: m.index, end: m.index + m[0].length, color: '#3fb950' });
    }
    while ((m = decorators.exec(rest)) !== null) {
      ranges.push({ start: m.index, end: m.index + m[0].length, color: '#f0883e' });
    }
    while ((m = veldArraySuffix.exec(rest)) !== null) {
      ranges.push({ start: m.index, end: m.index + m[0].length, color: '#58a6ff' });
    }
    while ((m = strings.exec(rest)) !== null) {
      ranges.push({ start: m.index, end: m.index + m[0].length, color: '#3fb950' });
    }

    // Sort and remove overlaps
    ranges.sort((a, b) => a.start - b.start);
    const merged: typeof ranges = [];
    for (const r of ranges) {
      if (merged.length === 0 || r.start >= merged[merged.length - 1].end) {
        merged.push(r);
      }
    }

    // Build token list from merged ranges
    let pos = 0;
    for (const r of merged) {
      if (r.start > pos) tokens.push({ text: rest.slice(pos, r.start) });
      tokens.push({ text: rest.slice(r.start, r.end), color: r.color });
      pos = r.end;
    }
    if (pos < rest.length) tokens.push({ text: rest.slice(pos) });
    if (comment) tokens.push({ text: comment, color: '#8b949e' });

    return React.createElement(
      'div',
      { key: li, style: { minHeight: '1.4em' } },
      tokens.map((t, ti) =>
        t.color
          ? React.createElement('span', { key: ti, style: { color: t.color } }, t.text)
          : React.createElement(React.Fragment, { key: ti }, t.text)
      ),
      // Empty line
      tokens.length === 0 && !comment ? '\n' : null
    );
  });
}

// ── TypeScript / JavaScript syntax highlighter ──

export function highlightTS(code: string): React.ReactNode[] {
  return code.split('\n').map((line, li) => {
    const tokens: Token[] = [];
    let rest = line;

    // Comments
    const commentIdx = rest.indexOf('//');
    let comment = '';
    if (commentIdx !== -1) {
      comment = rest.slice(commentIdx);
      rest = rest.slice(0, commentIdx);
    }

    const ranges: { start: number; end: number; color: string }[] = [];
    let m: RegExpExecArray | null;

    // Keywords
    const tsKeywords = /\b(import|export|from|const|let|var|async|await|function|return|if|else|try|catch|throw|new|interface|type|class|extends|implements|readonly|enum|module|namespace|declare|typeof|instanceof|in|of|for|while|do|switch|case|break|default|void|null|undefined|true|false)\b/g;
    while ((m = tsKeywords.exec(rest)) !== null) {
      ranges.push({ start: m.index, end: m.index + m[0].length, color: '#d2a8ff' });
    }

    // Types / Interfaces (PascalCase words, common TS types, I*Service, *Schema)
    const tsTypes = /\b(Promise|string|number|boolean|any|void|never|Record|Array|Map|Set|VeldApiError|ZodError|I[A-Z]\w*Service|[A-Z]\w*Schema|[A-Z][a-z]\w*)\b/g;
    while ((m = tsTypes.exec(rest)) !== null) {
      // Skip HTTP method names that happen to be PascalCase
      if (/^(GET|POST|PUT|DELETE|PATCH)$/.test(m[0])) continue;
      ranges.push({ start: m.index, end: m.index + m[0].length, color: '#58a6ff' });
    }

    // Strings (single-quoted, double-quoted, backtick)
    const strRegex = /('[^']*'|"[^"]*"|`[^`]*`)/g;
    while ((m = strRegex.exec(rest)) !== null) {
      ranges.push({ start: m.index, end: m.index + m[0].length, color: '#3fb950' });
    }

    // Numbers
    const numRegex = /\b(\d+)\b/g;
    while ((m = numRegex.exec(rest)) !== null) {
      ranges.push({ start: m.index, end: m.index + m[0].length, color: '#79c0ff' });
    }

    // Function calls  name(
    const fnRegex = /\b(\w+)(?=\s*\()/g;
    while ((m = fnRegex.exec(rest)) !== null) {
      const word = m[1];
      // Skip keywords and types already colored
      const isKeyword = /^(import|export|from|const|let|var|async|await|function|return|if|else|try|catch|throw|new|interface|type|class|for|while|do|switch)$/.test(word);
      if (!isKeyword) {
        ranges.push({ start: m.index, end: m.index + word.length, color: '#d2a8ff' });
      }
    }

    // Sort and deduplicate
    ranges.sort((a, b) => a.start - b.start);
    const merged: typeof ranges = [];
    for (const r of ranges) {
      if (merged.length === 0 || r.start >= merged[merged.length - 1].end) {
        merged.push(r);
      }
    }

    let pos = 0;
    for (const r of merged) {
      if (r.start > pos) tokens.push({ text: rest.slice(pos, r.start) });
      tokens.push({ text: rest.slice(r.start, r.end), color: r.color });
      pos = r.end;
    }
    if (pos < rest.length) tokens.push({ text: rest.slice(pos) });
    if (comment) tokens.push({ text: comment, color: '#8b949e' });

    return React.createElement(
      'div',
      { key: li, style: { minHeight: '1.4em' } },
      tokens.map((t, ti) =>
        t.color
          ? React.createElement('span', { key: ti, style: { color: t.color } }, t.text)
          : React.createElement(React.Fragment, { key: ti }, t.text)
      ),
      tokens.length === 0 && !comment ? '\n' : null
    );
  });
}

