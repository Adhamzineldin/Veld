// ---------------------------------------------------------------------------
// Express
// ---------------------------------------------------------------------------

export type ExpressMiddleware = (req: NodeRequestLike, res: NodeResponseLike, next: (err?: unknown) => void) => void;

/**
 * An Express-compatible middleware. It reads `req.body` when a body parser
 * already ran, otherwise it reads the request stream. Unmatched requests fall
 * through to `next()`.
 */
export function toExpress(handler: VeldHandler): ExpressMiddleware {
  return (req, res, next) => {
    dispatchNode(handler, req)
      .then((out) => (out === null ? next() : writeNode(res, out)))
      .catch((err: unknown) => next(err));
  };
}

