// ---------------------------------------------------------------------------
// Hono
// ---------------------------------------------------------------------------

export interface HonoContextLike {
  req: {
    method: string;
    url: string;
    header(): Record<string, string>;
    text(): Promise<string>;
  };
}

type ResponseBody = string | null | object;
type ResponseCtor = new (body: ResponseBody, init: { status: number; headers: Record<string, string> }) => unknown;

interface StreamControllerLike {
  enqueue(chunk: unknown): void;
  close(): void;
}

type StreamCtor = new (source: {
  pull(controller: StreamControllerLike): Promise<void>;
  cancel(): void;
}) => object;

/** The frames of a Server-Sent Events response as a byte stream, for `new Response(stream)`. */
function sseStream(frames: AsyncIterable<string>): object {
  const g = globalThis as unknown as {
    ReadableStream: StreamCtor;
    TextEncoder: new () => { encode(s: string): unknown };
  };
  const encoder = new g.TextEncoder();
  const it = frames[Symbol.asyncIterator]();
  return new g.ReadableStream({
    async pull(controller) {
      const step = await it.next();
      if (step.done === true) controller.close();
      else controller.enqueue(encoder.encode(step.value));
    },
    cancel() {
      void Promise.resolve(it.return?.()).catch(() => undefined);
    },
  });
}

export type HonoHandler = (c: HonoContextLike, next?: () => Promise<void>) => Promise<unknown>;

/** A Hono handler or middleware (`app.use('*', toHono(handler))`). Unmatched requests call `next()` when given. */
export function toHono(handler: VeldHandler): HonoHandler {
  return async (c, next) => {
    const { path, query } = splitUrl(c.req.url.replace(/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\/[^/]+/, ''));
    const out = await handler({
      method: c.req.method,
      path,
      query,
      headers: c.req.header(),
      text: () => c.req.text(),
      raw: c,
    });
    if (out === null && next) return next();
    const { Response } = globalThis as unknown as { Response: ResponseCtor };
    const res = out ?? NOT_FOUND;
    return new Response(res.stream === undefined ? res.body : sseStream(res.stream), {
      status: res.status,
      headers: res.headers,
    });
  };
}
