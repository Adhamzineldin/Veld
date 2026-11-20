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

type ResponseCtor = new (body: string | null, init: { status: number; headers: Record<string, string> }) => unknown;

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
    return new Response(res.body, { status: res.status, headers: res.headers });
  };
}
