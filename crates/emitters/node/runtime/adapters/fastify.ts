// ---------------------------------------------------------------------------
// Fastify
// ---------------------------------------------------------------------------

export interface FastifyRequestLike {
  method: string;
  url: string;
  headers: HeaderMap;
  body?: unknown;
  raw: unknown;
}

export interface FastifyReplyLike {
  code(statusCode: number): unknown;
  header(name: string, value: string): unknown;
  send(payload?: unknown): unknown;
}

export type FastifyRouteHandler = (request: FastifyRequestLike, reply: FastifyReplyLike) => Promise<unknown>;

/** A catch-all route handler: `fastify.all('/*', toFastify(handler))`. */
export function toFastify(handler: VeldHandler): FastifyRouteHandler {
  return async (request, reply) => {
    const { path, query } = splitUrl(request.url);
    const out =
      (await handler({
        method: request.method,
        path,
        query,
        headers: request.headers,
        body: request.body ?? undefined,
        raw: request,
      })) ?? NOT_FOUND;
    reply.code(out.status);
    for (const name of Object.keys(out.headers)) reply.header(name, out.headers[name] ?? '');
    return reply.send(out.body ?? '');
  };
}

