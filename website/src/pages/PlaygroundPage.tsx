import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import {
  AlertTriangle,
  Check,
  Copy,
  Download,
  FileCode2,
  Link2,
  Loader2,
  Plus,
  RotateCcw,
  Sparkles,
  X,
  XCircle,
} from 'lucide-react';
import { Engine } from '../playground/engine';
import Editor, { type EditorHandle } from '../playground/Editor';
import FileTree from '../playground/FileTree';
import { langOf } from '../playground/highlight';
import { defaultPreset, presets, type Preset } from '../playground/presets';
import {
  OPENAPI_KEY,
  defaultFramework,
  emitterOf,
  frameworksFor,
  kindsOf,
  toSpec,
  type Choice,
} from '../playground/targets';
import type { CompileResult, Describe, Diag, GenerateResult, ProjectSpec, SourceFile } from '../playground/types';
import { zip } from '../playground/zip';
import styles from './PlaygroundPage.module.css';

// ── persisted and shared state ──────────────────────────────────────────────

interface Snapshot {
  presetId: string;
  files: SourceFile[];
  project: ProjectSpec;
  choice: Choice;
}

const STORAGE_KEY = 'veld-playground-v1';

function choiceFor(preset: Preset): Choice {
  return {
    key: `${preset.target.emitter}:${preset.target.role}`,
    framework: preset.target.framework ?? '',
    service: preset.target.service ?? '',
    validate: true,
    queryOptions: false,
    language: '',
  };
}

function snapshotOf(preset: Preset): Snapshot {
  return { presetId: preset.id, files: preset.files, project: preset.project, choice: choiceFor(preset) };
}

function isSnapshot(v: unknown): v is Snapshot {
  const s = v as Snapshot | null;
  return !!s && Array.isArray(s.files) && s.files.length > 0 && !!s.project && !!s.choice && typeof s.choice.key === 'string';
}

/** Recover links and saved work created when the streaming preset stored the label "none". */
function restoreSnapshot(s: Snapshot): Snapshot {
  return s.choice.key === 'typescript:client' && s.choice.framework === 'none'
    ? { ...s, choice: { ...s.choice, framework: '' } }
    : s;
}

function encodeShare(s: Snapshot): string {
  const bytes = new TextEncoder().encode(JSON.stringify(s));
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '');
}

function decodeShare(text: string): Snapshot | null {
  try {
    const bin = atob(text.replaceAll('-', '+').replaceAll('_', '/'));
    const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
    const value: unknown = JSON.parse(new TextDecoder().decode(bytes));
    return isSnapshot(value) ? value : null;
  } catch {
    return null;
  }
}

function initialSnapshot(): Snapshot {
  try {
    const shared = /^#s=(.+)$/.exec(window.location.hash)?.[1];
    const fromLink = shared ? decodeShare(shared) : null;
    if (fromLink) return restoreSnapshot(fromLink);
    const saved: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? 'null');
    if (isSnapshot(saved)) return restoreSnapshot(saved);
  } catch {
    // Storage can be blocked; the page works without it.
  }
  return snapshotOf(defaultPreset);
}

// ── small helpers ───────────────────────────────────────────────────────────

function servicesOf(files: SourceFile[], project: ProjectSpec): string[] {
  const libraries = new Set(project.packages.filter((p) => p.kind === 'library').map((p) => p.name));
  const names = new Set(files.map((f) => f.path.split('/')[0]).filter((n) => !libraries.has(n)));
  return [...names].sort();
}

function pickDefault(files: SourceFile[]): string | null {
  const paths = files.map((f) => f.path);
  const lib = (p: string) => !p.includes('runtime/') && !/readme|package\.json|tsconfig|cargo\.toml|pom\.xml/i.test(p);
  return (
    paths.find((p) => lib(p) && /(^|\/)(client|server|index|lib|main)\.(ts|java|rs)$/.test(p)) ??
    paths.find((p) => lib(p) && /\.(ts|java|rs|json)$/.test(p)) ??
    paths[0] ??
    null
  );
}

function plural(n: number, word: string): string {
  return `${n} ${word}${n === 1 ? '' : 's'}`;
}

async function writeClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

function download(blob: Blob, name: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

/** Registers the offline cache once the engine is running (production builds only). */
function registerOfflineCache() {
  if (import.meta.env.PROD && 'serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js').catch(() => undefined);
  }
}

// ── the page ────────────────────────────────────────────────────────────────

type Pane = 'contract' | 'output';

export default function PlaygroundPage() {
  const [initial] = useState(initialSnapshot);
  const [presetId, setPresetId] = useState(initial.presetId);
  const [files, setFiles] = useState(initial.files);
  const [project, setProject] = useState(initial.project);
  const [choice, setChoice] = useState(initial.choice);
  const [active, setActive] = useState(initial.files[0].path);
  const [pane, setPane] = useState<Pane>('contract');

  const [engine, setEngine] = useState<Engine | null>(null);
  const [info, setInfo] = useState<Describe | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [compiled, setCompiled] = useState<CompileResult | null>(null);
  const [output, setOutput] = useState<(GenerateResult & {
    sources: SourceFile[];
    project: ProjectSpec;
    targetKey: string;
  }) | null>(null);
  const [busy, setBusy] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);

  const [newFile, setNewFile] = useState<string | null>(null);
  const [copied, setCopied] = useState<'output' | 'link' | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const editor = useRef<EditorHandle>(null);

  // Load the engine (WebAssembly in a worker) when the page opens.
  useEffect(() => {
    const e = new Engine();
    let alive = true;
    e.describe()
      .then((d) => {
        if (!alive) return;
        setInfo(d);
        setEngine(e);
        registerOfflineCache();
      })
      .catch((err: unknown) => alive && setLoadError(err instanceof Error ? err.message : String(err)));
    return () => {
      alive = false;
      e.terminate();
    };
  }, []);

  const kinds = useMemo(() => (info ? kindsOf(info) : []), [info]);
  const kind = kinds.find((k) => k.key === choice.key);
  const isSpec = choice.key === OPENAPI_KEY;
  const services = useMemo(() => servicesOf(files, project), [files, project]);
  const spec = useMemo(() => (info && !isSpec ? toSpec(choice, info, services) : null), [info, choice, services, isSpec]);
  const specKey = JSON.stringify(spec);

  // Compile, then generate, a moment after the last change. Stale results are dropped.
  useEffect(() => {
    if (!engine) return;
    let alive = true;
    const timer = setTimeout(() => {
      setBusy(true);
      (async () => {
        const c = await engine.compile(files, project);
        if (!alive) return;
        setCompiled(c);
        if (!c.ok) return;
        let out: GenerateResult;
        if (isSpec) {
          const o = await engine.openapi(files, project);
          out = { ok: o.ok, files: o.ok ? [{ path: 'openapi.json', text: o.text }] : [], diagnostics: o.diagnostics };
        } else if (spec) {
          out = await engine.generate(files, spec, project);
        } else {
          return;
        }
        if (!alive) return;
        setOutput({ ...out, files: out.ok ? out.files : [], sources: files, project, targetKey: specKey });
      })()
        .catch((err: unknown) => alive && setLoadError(err instanceof Error ? err.message : String(err)))
        .finally(() => alive && setBusy(false));
    }, 250);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
    // `specKey` stands for the parsed target, which is rebuilt whenever its inputs change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [engine, files, project, specKey, isSpec]);

  // Remember the work in progress (best effort).
  useEffect(() => {
    const timer = setTimeout(() => {
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify({ presetId, files, project, choice } satisfies Snapshot));
      } catch {
        // Private windows can refuse storage.
      }
    }, 500);
    return () => clearTimeout(timer);
  }, [presetId, files, project, choice]);

  const stale = output !== null && (output.sources !== files || output.project !== project || output.targetKey !== specKey);
  const outFiles = useMemo(() => output?.files ?? [], [output]);
  const shown = outFiles.find((f) => f.path === selected) ?? outFiles.find((f) => f.path === pickDefault(outFiles)) ?? null;
  const diagnostics = useMemo(() => {
    const seen = new Set<string>();
    const all: Diag[] = [];
    for (const d of [...(compiled?.diagnostics ?? []), ...(stale ? [] : output?.diagnostics ?? [])]) {
      if (seen.has(d.rendered)) continue;
      seen.add(d.rendered);
      all.push(d);
    }
    return all;
  }, [compiled, output, stale]);
  const activeFile = files.find((f) => f.path === active) ?? files[0];
  const errors = diagnostics.filter((d) => d.severity === 'error').length;
  const warnings = diagnostics.length - errors;

  const setText = useCallback(
    (text: string) => setFiles((fs) => fs.map((f) => (f.path === activeFile.path ? { ...f, text } : f))),
    [activeFile.path],
  );

  function loadPreset(p: Preset) {
    const s = snapshotOf(p);
    setPresetId(p.id);
    setFiles(s.files);
    setProject(s.project);
    setActive(s.files[0].path);
    setSelected(null);
    setNote(null);
    const wanted = s.choice;
    setChoice(kinds.some((k) => k.key === wanted.key) || !info ? wanted : { ...wanted, key: kinds[0]?.key ?? wanted.key });
  }

  function pickKind(key: string) {
    const k = kinds.find((x) => x.key === key);
    if (!k || !info) return setChoice((c) => ({ ...c, key }));
    setSelected(null);
    setChoice((c) => ({ ...c, key, framework: defaultFramework(info, k), language: '', queryOptions: false }));
  }

  async function format() {
    if (!engine) return;
    const r = await engine.format(activeFile.text);
    if (r.ok) {
      setText(r.text);
      setNote(null);
    } else {
      const first = r.diagnostics[0];
      setNote(first ? `Cannot format: ${first.message} (line ${first.line})` : 'Cannot format this file');
      if (first) editor.current?.reveal(first.line, first.col);
    }
  }

  async function copyOutput() {
    if (shown && (await writeClipboard(shown.text))) {
      setCopied('output');
      setTimeout(() => setCopied(null), 1500);
    }
  }

  async function share() {
    const snap: Snapshot = { presetId, files, project, choice };
    const url = `${window.location.origin}${window.location.pathname}#s=${encodeShare(snap)}`;
    window.history.replaceState(null, '', url);
    if (await writeClipboard(url)) {
      setCopied('link');
      setTimeout(() => setCopied(null), 1500);
    }
  }

  function downloadZip() {
    if (stale || outFiles.length === 0) return;
    const folder = `veld-${presetId}-${choice.key.replace(':', '-')}`;
    download(zip(outFiles, folder), `${folder}.zip`);
  }

  function addFile() {
    if (newFile === null) return;
    let path = newFile.trim().replace(/^\/+/, '');
    if (!path) return setNewFile(null);
    if (!path.endsWith('.veld')) path += '.veld';
    if (!path.includes('/')) path = `${activeFile.path.split('/')[0]}/${path}`;
    if (!files.some((f) => f.path === path)) setFiles((fs) => [...fs, { path, text: '' }]);
    setActive(path);
    setNewFile(null);
  }

  function removeFile(path: string) {
    if (files.length < 2) return;
    const rest = files.filter((f) => f.path !== path);
    setFiles(rest);
    if (active === path) setActive(rest[0].path);
  }

  function jump(d: Diag) {
    if (!files.some((f) => f.path === d.file)) return;
    setActive(d.file);
    setPane('contract');
    requestAnimationFrame(() => editor.current?.reveal(d.line, d.col));
  }

  const frameworks = info && kind ? frameworksFor(info, kind) : [];
  const language = info && kind ? emitterOf(info, kind.emitter)?.language : null;
  const loading = !info && !loadError;

  return (
    <div
      className={styles.page}
      onKeyDown={(e) => {
        if ((e.ctrlKey || e.metaKey) && e.key === 's') {
          e.preventDefault();
          void format();
        }
      }}
    >
      <header className={styles.head}>
        <div>
          <h1 className={styles.title}>
            Playground
            <span className={styles.badge}>
              <Sparkles size={12} aria-hidden="true" /> real engine, in your browser
            </span>
          </h1>
          <p className={styles.lead}>
            Write a contract and watch the generated code appear. This is the same compiler and the same generators
            as the <code>veld</code> CLI, compiled to WebAssembly: the output is byte for byte what{' '}
            <code>veld generate</code> writes. Nothing leaves your browser.
          </p>
        </div>
        <div className={styles.engine} role="status">
          {loading && (
            <>
              <Loader2 size={14} className={styles.spin} aria-hidden="true" /> Loading the engine
            </>
          )}
          {info && (
            <>
              <span className={styles.dot} aria-hidden="true" /> Veld {info.version}
            </>
          )}
        </div>
      </header>

      <div className={styles.bar}>
        <div className={styles.group} role="group" aria-label="Examples">
          <span className={styles.label}>Example</span>
          <div className={styles.chips}>
            {presets.map((p) => (
              <button
                key={p.id}
                type="button"
                className={`${styles.chip} ${p.id === presetId ? styles.chipOn : ''}`}
                title={p.blurb}
                onClick={() => loadPreset(p)}
              >
                {p.title}
              </button>
            ))}
          </div>
        </div>

        <div className={styles.group} role="group" aria-label="Target">
          <span className={styles.label}>Generate</span>
          <div className={styles.chips}>
            {kinds.map((k) => (
              <button
                key={k.key}
                type="button"
                className={`${styles.chip} ${k.key === choice.key ? styles.chipOn : ''}`}
                onClick={() => pickKind(k.key)}
              >
                {k.label}
              </button>
            ))}
            <button
              type="button"
              className={`${styles.chip} ${isSpec ? styles.chipOn : ''}`}
              onClick={() => pickKind(OPENAPI_KEY)}
            >
              OpenAPI
            </button>
          </div>
        </div>

        {!isSpec && kind && (
          <div className={styles.options}>
            {frameworks.length > 0 && (
              <label className={styles.field}>
                <span>Framework</span>
                <select value={choice.framework} onChange={(e) => setChoice({ ...choice, framework: e.target.value })}>
                  {kind.role === 'client' && <option value="">none</option>}
                  {frameworks.map((f) => (
                    <option key={f} value={f}>
                      {f}
                    </option>
                  ))}
                </select>
              </label>
            )}
            {kind.role === 'server' && services.length > 1 && (
              <label className={styles.field}>
                <span>Service</span>
                <select
                  value={choice.service || services[0]}
                  onChange={(e) => setChoice({ ...choice, service: e.target.value })}
                >
                  {services.map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
              </label>
            )}
            {language && language.versions.length > 0 && (
              <label className={styles.field}>
                <span>{language.id === 'typescript' ? 'ECMAScript' : language.id}</span>
                <select value={choice.language} onChange={(e) => setChoice({ ...choice, language: e.target.value })}>
                  <option value="">default</option>
                  {language.versions.map((v) => (
                    <option key={v} value={v}>
                      {v}
                    </option>
                  ))}
                </select>
              </label>
            )}
            <label className={styles.check}>
              <input
                type="checkbox"
                checked={choice.validate}
                onChange={(e) => setChoice({ ...choice, validate: e.target.checked })}
              />
              Runtime validation
            </label>
            {kind.emitter === 'typescript' && (
              <label className={styles.check}>
                <input
                  type="checkbox"
                  checked={choice.queryOptions}
                  onChange={(e) => setChoice({ ...choice, queryOptions: e.target.checked })}
                />
                Query options
              </label>
            )}
          </div>
        )}
      </div>

      <div className={styles.paneTabs} role="tablist" aria-label="Panes">
        <button type="button" role="tab" aria-selected={pane === 'contract'} onClick={() => setPane('contract')}>
          Contract
        </button>
        <button type="button" role="tab" aria-selected={pane === 'output'} onClick={() => setPane('output')}>
          Output{outFiles.length > 0 && <span className={styles.count}>{outFiles.length}</span>}
        </button>
      </div>

      <div className={styles.panes}>
        <section className={`${styles.pane} ${pane === 'contract' ? '' : styles.hidden}`} aria-label="Contract">
          <div className={styles.paneHead}>
            <div className={styles.fileTabs} role="tablist" aria-label="Contract files">
              {files.map((f) => (
                <span key={f.path} className={`${styles.fileTab} ${f.path === activeFile.path ? styles.fileTabOn : ''}`}>
                  <button type="button" role="tab" aria-selected={f.path === activeFile.path} onClick={() => setActive(f.path)}>
                    {f.path}
                  </button>
                  {files.length > 1 && f.path === activeFile.path && (
                    <button type="button" className={styles.x} aria-label={`Remove ${f.path}`} onClick={() => removeFile(f.path)}>
                      <X size={12} />
                    </button>
                  )}
                </span>
              ))}
              {newFile === null ? (
                <button type="button" className={styles.add} aria-label="Add a contract file" onClick={() => setNewFile(`${activeFile.path.split('/')[0]}/new.veld`)}>
                  <Plus size={14} />
                </button>
              ) : (
                <input
                  className={styles.newFile}
                  value={newFile}
                  autoFocus
                  aria-label="New file path"
                  onChange={(e) => setNewFile(e.target.value)}
                  onBlur={addFile}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') addFile();
                    if (e.key === 'Escape') setNewFile(null);
                  }}
                />
              )}
            </div>
            <div className={styles.actions}>
              <button type="button" className={styles.btn} onClick={() => void format()} disabled={!engine} title="Format (Ctrl+S)">
                <FileCode2 size={14} aria-hidden="true" /> Format
              </button>
              <button type="button" className={styles.btn} onClick={() => loadPreset(presets.find((p) => p.id === presetId) ?? defaultPreset)} title="Restore this example">
                <RotateCcw size={14} aria-hidden="true" /> Reset
              </button>
              <button type="button" className={styles.btn} onClick={() => void share()} title="Copy a link to this contract">
                {copied === 'link' ? <Check size={14} aria-hidden="true" /> : <Link2 size={14} aria-hidden="true" />}
                {copied === 'link' ? 'Link copied' : 'Share'}
              </button>
            </div>
          </div>

          <Editor
            key={activeFile.path}
            ref={editor}
            value={activeFile.text}
            lang="veld"
            ariaLabel={`Contract ${activeFile.path}`}
            diagnostics={diagnostics.filter((d) => d.file === activeFile.path)}
            onChange={setText}
          />

          <div className={styles.problems} aria-live="polite">
            <div className={styles.problemsHead}>
              {errors > 0 ? (
                <span className={styles.bad}>
                  <XCircle size={14} aria-hidden="true" /> {plural(errors, 'error')}
                </span>
              ) : compiled ? (
                <span className={styles.good}>
                  <Check size={14} aria-hidden="true" />{' '}
                  {compiled.summary
                    ? `${plural(compiled.summary.services, 'service')}, ${plural(compiled.summary.endpoints, 'endpoint')}, ${plural(compiled.summary.types, 'type')}`
                    : 'Compiles'}
                </span>
              ) : (
                <span className={styles.muted}>Waiting for the engine</span>
              )}
              {warnings > 0 && (
                <span className={styles.warn}>
                  <AlertTriangle size={14} aria-hidden="true" /> {plural(warnings, 'warning')}
                </span>
              )}
              {busy && <Loader2 size={13} className={styles.spin} aria-label="Working" />}
              {note && <span className={styles.muted}>{note}</span>}
            </div>
            {diagnostics.length > 0 && (
              <ul className={styles.diagList}>
                {diagnostics.map((d, i) => (
                  <li key={`${d.rendered}-${i}`} className={d.severity === 'error' ? styles.diagError : styles.diagWarn}>
                    <div className={styles.diagTop}>
                      {d.code === 'INPUT' ? (
                        <code>{d.code}</code>
                      ) : (
                        <Link to={`/docs#${d.code}`} className={styles.diagCode} title={`${d.code} in the diagnostics reference`}>
                          {d.code}
                        </Link>
                      )}
                      <span className={styles.msg}>{d.message}</span>
                      {d.line > 0 && files.some((f) => f.path === d.file) && (
                        <button type="button" className={styles.loc} onClick={() => jump(d)}>
                          {d.file}:{d.line}:{d.col}
                        </button>
                      )}
                    </div>
                    {d.help && <div className={styles.help}>{d.help}</div>}
                    {d.notes.map((n) => (
                      <div key={n} className={styles.help}>
                        {n}
                      </div>
                    ))}
                  </li>
                ))}
              </ul>
            )}
          </div>
        </section>

        <section className={`${styles.pane} ${pane === 'output' ? '' : styles.hidden}`} aria-label="Generated output">
          <div className={styles.paneHead}>
            <div className={styles.outTitle}>
              {outFiles.length > 0 ? plural(outFiles.length, 'file') : 'Generated output'}
              {stale && <span className={styles.stale}>last good output</span>}
            </div>
            <div className={styles.actions}>
              <button type="button" className={styles.btn} onClick={() => void copyOutput()} disabled={!shown}>
                {copied === 'output' ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}
                {copied === 'output' ? 'Copied' : 'Copy'}
              </button>
              <button type="button" className={`${styles.btn} ${styles.primary}`} onClick={downloadZip} disabled={stale || outFiles.length === 0}>
                <Download size={14} aria-hidden="true" /> Download .zip
              </button>
            </div>
          </div>

          {loading && (
            <div className={styles.empty}>
              <Loader2 size={22} className={styles.spin} aria-hidden="true" />
              <p>Loading the Veld engine (WebAssembly, about 0.5 MB compressed). It is cached for offline use afterwards.</p>
            </div>
          )}
          {loadError && (
            <div className={styles.empty}>
              <XCircle size={22} aria-hidden="true" />
              <p>The engine could not start: {loadError}</p>
              <button type="button" className={styles.btn} onClick={() => window.location.reload()}>
                Reload
              </button>
            </div>
          )}
          {!loading && !loadError && outFiles.length === 0 && (
            <div className={styles.empty}>
              <p>
                {errors > 0
                  ? 'Fix the errors in the contract to see the generated code.'
                  : output && output.diagnostics.length > 0
                    ? 'This target cannot be generated for this contract. See the problems under the contract.'
                    : 'Generating'}
              </p>
            </div>
          )}
          {outFiles.length > 0 && shown && (
            <div className={styles.viewer}>
              <nav className={styles.tree} aria-label="Files">
                <FileTree paths={outFiles.map((f) => f.path)} selected={shown.path} onSelect={setSelected} />
              </nav>
              <div className={styles.code}>
                <div className={styles.codePath}>
                  <span>{shown.path}</span>
                  <span className={styles.muted}>{plural(shown.text.split('\n').length - 1, 'line')}</span>
                </div>
                <Editor key={shown.path} value={shown.text} lang={langOf(shown.path)} readOnly ariaLabel={`Generated ${shown.path}`} />
              </div>
            </div>
          )}
        </section>
      </div>

      <p className={styles.foot}>
        The engine is <code>crates/veld-wasm</code>, built from the same crates as the CLI, and a parity test generates every example with
        both and compares the bytes. See <Link to="/docs#generate">veld generate</Link> and <Link to="/docs#getting-started">Getting started</Link>.
      </p>
    </div>
  );
}
