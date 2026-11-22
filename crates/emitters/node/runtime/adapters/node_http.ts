// ---------------------------------------------------------------------------
// Node http
// ---------------------------------------------------------------------------

export type NodeHttpListener = (req: NodeRequestLike, res: NodeResponseLike) => void;

/** A `http.createServer` listener. Unmatched requests get a 404 problem. */
export function toNodeHttp(handler: VeldHandler): NodeHttpListener {
  return (req, res) => {
    dispatchNode(handler, req)
      .then(
        (out) => writeNode(res, out ?? NOT_FOUND),
        () => writeNode(res, FAILED),
      )
      .catch(() => res.end());
  };
}

