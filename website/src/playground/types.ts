// Shapes exchanged with the WebAssembly engine (crates/veld-wasm/src/api.rs).

export interface SourceFile {
  path: string;
  text: string;
}

export interface PackageSpec {
  name: string;
  kind?: 'service' | 'library';
  baseUrl?: string;
  prefix?: string;
  consumes?: string[];
}

export interface ProjectSpec {
  packages: PackageSpec[];
}

export interface TargetSpec {
  name?: string;
  emitter: string;
  role: 'server' | 'client';
  framework?: string | null;
  options?: Record<string, string | boolean>;
  service?: string | null;
  services?: string[] | null;
  clients?: boolean;
}

export interface Diag {
  code: string;
  severity: 'error' | 'warning';
  message: string;
  file: string;
  line: number;
  col: number;
  endLine: number;
  endCol: number;
  help: string | null;
  notes: string[];
  rendered: string;
}

export interface Summary {
  services: number;
  endpoints: number;
  types: number;
}

export interface CompileResult {
  ok: boolean;
  diagnostics: Diag[];
  summary: Summary | null;
}

export interface GenerateResult {
  ok: boolean;
  files: SourceFile[];
  diagnostics: Diag[];
}

export interface FormatResult {
  ok: boolean;
  text: string;
  diagnostics: Diag[];
}

export interface OpenapiResult {
  ok: boolean;
  text: string;
  diagnostics: Diag[];
}

export interface FrameworkDesc {
  id: string;
  versions: string[];
}

export interface EmitterDesc {
  id: string;
  roles: string[];
  frameworks: Record<string, FrameworkDesc[]>;
  language: { id: string; versions: string[] } | null;
}

export interface Describe {
  version: string;
  emitters: EmitterDesc[];
}
