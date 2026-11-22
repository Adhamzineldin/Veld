// Runs the Veld engine (WebAssembly) off the main thread. Requests carry an id; every reply
// echoes it, so the page can ignore results of stale requests.
import init, { compile, describe, exportOpenapi, format, generate } from '../wasm/veld_wasm.js';
import wasmUrl from '../wasm/veld_wasm_bg.wasm?url';

export type EngineRequest =
  | { id: number; op: 'init' }
  | { id: number; op: 'compile'; files: string; project: string }
  | { id: number; op: 'generate'; files: string; target: string; project: string }
  | { id: number; op: 'openapi'; files: string; project: string }
  | { id: number; op: 'format'; text: string };

export type EngineReply = { id: number; ok: true; json: string } | { id: number; ok: false; error: string };

let ready: Promise<unknown> | null = null;

function handle(req: EngineRequest): string {
  switch (req.op) {
    case 'init':
      return describe();
    case 'compile':
      return compile(req.files, req.project);
    case 'generate':
      return generate(req.files, req.target, req.project);
    case 'openapi':
      return exportOpenapi(req.files, req.project);
    case 'format':
      return format(req.text);
  }
}

self.addEventListener('message', (event: MessageEvent<EngineRequest>) => {
  const req = event.data;
  ready ??= init({ module_or_path: wasmUrl });
  ready
    .then(() => {
      const reply: EngineReply = { id: req.id, ok: true, json: handle(req) };
      self.postMessage(reply);
    })
    .catch((e: unknown) => {
      ready = null;
      const reply: EngineReply = { id: req.id, ok: false, error: e instanceof Error ? e.message : String(e) };
      self.postMessage(reply);
    });
});
