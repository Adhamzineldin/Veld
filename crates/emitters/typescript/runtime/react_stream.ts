
import type { Reconnect } from './veld';

// -- streaming (appended only when a contract has `get ... -> stream` or `ws` endpoints) ------

export interface StreamOptions<C> {
  /** Set to `false` to hold the stream back. Default: `true`. */
  enabled?: boolean;
  /** A client to use instead of the one from the provider. */
  client?: C;
  /** Keep every item in `items` (default) or only `latest`. */
  accumulate?: boolean;
  /** Keep at most this many items: the newest ones. Default: unlimited. */
  max?: number;
  /** Reconnect with exponential backoff after a connection failure. On by default here; `false` switches it off. */
  reconnect?: Reconnect | false;
}

export interface StreamResult<T> {
  /** Every item received so far, oldest first (empty when `accumulate` is `false`). */
  items: T[];
  /** The newest item. */
  latest: T | undefined;
  /** The error that ended the stream: typed errors work with `matches()` as usual. */
  error: unknown;
  /** `true` from the start until the stream ends, fails or is stopped. */
  streaming: boolean;
  /** Opens the stream again. */
  restart: () => void;
}

export interface ChannelResult<In, Out> extends StreamResult<In> {
  /** Sends a message on the open WebSocket; messages sent before it is open are queued. */
  send: (message: Out) => void;
}

interface Opened<T> extends AsyncIterable<T> {
  send?: (message: never) => void;
}

interface StreamState<T> {
  key: string;
  round: number;
  items: T[];
  latest: T | undefined;
  error: unknown;
  done: boolean;
}

/**
 * Consumes a stream (Server-Sent Events or a WebSocket channel) and re-opens it when the canonical
 * bytes of `req` change or `restart` is called. The previous stream is aborted on a change and on
 * unmount. `null` holds it back.
 */
export function useStream<Req, T, Out = never>(
  open: (req: Req, init: CallInit & { reconnect?: Reconnect | false }) => AsyncIterable<T>,
  req: Req | null,
  options: { enabled?: boolean; accumulate?: boolean; max?: number; reconnect?: Reconnect | false } = {},
): ChannelResult<T, Out> {
  const key = req === null || options.enabled === false ? null : canonical(req);
  const [state, setState] = useState<StreamState<T> | null>(null);
  const [round, setRound] = useState(0);
  const latest = useRef({ open, req, options });
  latest.current = { open, req, options };
  const opened = useRef<Opened<T> | null>(null);
  useEffect(() => {
    if (key === null) return undefined;
    const aborter = newAborter();
    const { open: openStream, req: current } = latest.current;
    setState({ key, round, items: [], latest: undefined, error: undefined, done: false });
    const stream = openStream(current as Req, { signal: aborter.signal, reconnect: latest.current.options.reconnect ?? {} }) as Opened<T>;
    opened.current = stream;
    const mine = (s: StreamState<T> | null): s is StreamState<T> => s !== null && s.key === key && s.round === round;
    void (async () => {
      try {
        for await (const item of stream) {
          if (aborter.signal.aborted) return;
          const { accumulate, max } = latest.current.options;
          setState((s) => {
            if (!mine(s)) return s;
            const kept = accumulate === false ? s.items : [...s.items, item];
            return { ...s, latest: item, items: max !== undefined && kept.length > max ? kept.slice(-max) : kept };
          });
        }
        if (!aborter.signal.aborted) setState((s) => (mine(s) ? { ...s, done: true } : s));
      } catch (error) {
        if (!aborter.signal.aborted) setState((s) => (mine(s) ? { ...s, error, done: true } : s));
      }
    })();
    return () => {
      aborter.abort();
      opened.current = null;
    };
  }, [key, round]);
  const restart = useCallback(() => setRound((n) => n + 1), []);
  const send = useCallback((message: Out) => {
    const channel = opened.current;
    if (!channel?.send) throw new Error('send: this stream is not an open WebSocket channel');
    channel.send(message as never);
  }, []);
  const settled = state !== null && state.key === key && state.round === round ? state : null;
  return {
    items: settled?.items ?? [],
    latest: settled?.latest,
    error: settled?.error,
    streaming: key !== null && (settled === null || !settled.done),
    restart,
    send,
  };
}
