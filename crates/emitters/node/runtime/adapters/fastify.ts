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
  /** Needed for Server-Sent Events: takes the response over from Fastify. */
  hijack?(): unknown;
  raw?: NodeResponseLike;
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
    if (out.stream !== undefined && reply.raw !== undefined) {
      reply.hijack?.();
      await writeNode(reply.raw, out);
      return undefined;
    }
    reply.code(out.status);
    for (const name of Object.keys(out.headers)) reply.header(name, out.headers[name] ?? '');
    return reply.send(out.body ?? '');
  };
}

