import type { ProjectSpec, SourceFile, TargetSpec } from './types';

export interface Preset {
  id: string;
  title: string;
  blurb: string;
  project: ProjectSpec;
  target: TargetSpec;
  files: SourceFile[];
}

interface PresetMeta {
  title: string;
  blurb: string;
  project: ProjectSpec;
  target: TargetSpec;
}

// The contracts are real files under ./presets/<id>/<package>/*.veld, so the native parity test
// (crates/veld-wasm/tests/parity.rs) generates every one of them with the CLI too.
const sources = import.meta.glob('./presets/*/*/**/*.veld', { query: '?raw', import: 'default', eager: true }) as Record<
  string,
  string
>;
const metas = import.meta.glob('./presets/*/preset.json', { import: 'default', eager: true }) as Record<
  string,
  PresetMeta
>;

const ORDER = ['hello', 'microservices', 'library-package', 'streaming', 'errors'];

function load(): Preset[] {
  const presets: Preset[] = [];
  for (const [path, meta] of Object.entries(metas)) {
    const id = path.split('/')[2];
    const prefix = `./presets/${id}/`;
    const files = Object.entries(sources)
      .filter(([p]) => p.startsWith(prefix))
      .map(([p, text]) => ({ path: p.slice(prefix.length), text }))
      .sort((a, b) => a.path.localeCompare(b.path));
    presets.push({ id, ...meta, files });
  }
  const rank = (id: string) => {
    const i = ORDER.indexOf(id);
    return i < 0 ? ORDER.length : i;
  };
  return presets.sort((a, b) => rank(a.id) - rank(b.id) || a.id.localeCompare(b.id));
}

export const presets: Preset[] = load();
export const defaultPreset: Preset = presets.find((p) => p.id === 'hello') ?? presets[0];
