import type { Describe, EmitterDesc, TargetSpec } from './types';

/** The pseudo target that exports the OpenAPI document instead of running a generator. */
export const OPENAPI_KEY = 'openapi:spec';

export interface TargetKind {
  key: string;
  emitter: string;
  role: 'server' | 'client';
  label: string;
}

const LABELS: Record<string, string> = {
  'node:server': 'Node.js server',
  'typescript:client': 'TypeScript client',
  'java:server': 'Java server',
  'java:client': 'Java client',
  'rust:server': 'Rust server',
  'rust:client': 'Rust client',
};

const ORDER = Object.keys(LABELS);

/** The targets the loaded engine can generate. A generator the engine lacks simply does not appear. */
export function kindsOf(engine: Describe): TargetKind[] {
  const kinds: TargetKind[] = [];
  for (const e of engine.emitters) {
    for (const role of e.roles) {
      if (role !== 'server' && role !== 'client') continue;
      const key = `${e.id}:${role}`;
      kinds.push({ key, emitter: e.id, role, label: LABELS[key] ?? `${e.id} ${role}` });
    }
  }
  const rank = (k: string) => {
    const i = ORDER.indexOf(k);
    return i < 0 ? ORDER.length : i;
  };
  return kinds.sort((a, b) => rank(a.key) - rank(b.key));
}

export function emitterOf(engine: Describe, id: string): EmitterDesc | undefined {
  return engine.emitters.find((e) => e.id === id);
}

/** What the user picked in the target bar. */
export interface Choice {
  key: string;
  /** `''` means no framework (clients only). */
  framework: string;
  service: string;
  validate: boolean;
  queryOptions: boolean;
  /** A language version such as `21`; `''` is the generator default. */
  language: string;
}

export function frameworksFor(engine: Describe, kind: TargetKind): string[] {
  const list = emitterOf(engine, kind.emitter)?.frameworks[kind.role] ?? [];
  return list.map((f) => f.id);
}

/** The first framework of a server, none for a client: what a new target starts with. */
export function defaultFramework(engine: Describe, kind: TargetKind): string {
  return kind.role === 'server' ? (frameworksFor(engine, kind)[0] ?? '') : '';
}

export function toSpec(choice: Choice, engine: Describe, services: string[]): TargetSpec | null {
  const kind = kindsOf(engine).find((k) => k.key === choice.key);
  if (!kind) return null;
  const options: Record<string, string | boolean> = {};
  if (!choice.validate) options.validate = false;
  if (kind.emitter === 'typescript' && choice.queryOptions) options['query-options'] = true;
  const language = emitterOf(engine, kind.emitter)?.language;
  if (language && choice.language) options[language.id] = choice.language;
  return {
    emitter: kind.emitter,
    role: kind.role,
    framework: choice.framework || null,
    options,
    service: kind.role === 'server' && services.length > 1 ? choice.service || services[0] : null,
  };
}
