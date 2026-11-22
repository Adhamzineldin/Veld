import type {
  CompileResult,
  Describe,
  FormatResult,
  GenerateResult,
  OpenapiResult,
  ProjectSpec,
  SourceFile,
  TargetSpec,
} from './types';
import type { EngineReply, EngineRequest } from './engine.worker';

type Distribute<T> = T extends { id: number } ? Omit<T, 'id'> : never;

/** A typed client for the engine worker. The WebAssembly module loads on the first call. */
export class Engine {
  #worker: Worker;
  #next = 1;
  #pending = new Map<number, { resolve: (json: string) => void; reject: (e: Error) => void }>();

  constructor() {
    this.#worker = new Worker(new URL('./engine.worker.ts', import.meta.url), { type: 'module' });
    this.#worker.addEventListener('message', (event: MessageEvent<EngineReply>) => {
      const reply = event.data;
      const entry = this.#pending.get(reply.id);
      if (!entry) return;
      this.#pending.delete(reply.id);
      if (reply.ok) entry.resolve(reply.json);
      else entry.reject(new Error(reply.error));
    });
    this.#worker.addEventListener('error', (event) => {
      const error = new Error(event.message || 'the engine failed to start');
      for (const entry of this.#pending.values()) entry.reject(error);
      this.#pending.clear();
    });
  }

  #call<T>(req: Distribute<EngineRequest>): Promise<T> {
    const id = this.#next++;
    return new Promise<string>((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#worker.postMessage({ ...req, id });
    }).then((json) => JSON.parse(json) as T);
  }

  /** Loads the module and reports the engine version and its generators. */
  describe(): Promise<Describe> {
    return this.#call<Describe>({ op: 'init' });
  }

  compile(files: SourceFile[], project: ProjectSpec): Promise<CompileResult> {
    return this.#call<CompileResult>({ op: 'compile', files: JSON.stringify(files), project: JSON.stringify(project) });
  }

  generate(files: SourceFile[], target: TargetSpec, project: ProjectSpec): Promise<GenerateResult> {
    return this.#call<GenerateResult>({
      op: 'generate',
      files: JSON.stringify(files),
      target: JSON.stringify(target),
      project: JSON.stringify(project),
    });
  }

  openapi(files: SourceFile[], project: ProjectSpec): Promise<OpenapiResult> {
    return this.#call<OpenapiResult>({ op: 'openapi', files: JSON.stringify(files), project: JSON.stringify(project) });
  }

  format(text: string): Promise<FormatResult> {
    return this.#call<FormatResult>({ op: 'format', text });
  }

  terminate(): void {
    this.#worker.terminate();
    this.#pending.clear();
  }
}
