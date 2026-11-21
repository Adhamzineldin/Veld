
import type { Reconnect } from './veld';

// -- streaming (appended only when a contract has `get ... -> stream` or `ws` endpoints) ------

export interface StreamOptions<C> {
  /** Set to `false` to hold the stream back. Default: `true`. May be a ref or a getter. */
  enabled?: MaybeRefOrGetter<boolean>;
  /** A client to use instead of the one from the plugin. */
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
  items: Readonly<Ref<T[]>>;
  /** The newest item. */
  latest: Readonly<Ref<T | undefined>>;
  /** The error that ended the stream: typed errors work with `matches()` as usual. */
  error: Readonly<Ref<unknown>>;
  /** `true` from the start until the stream ends, fails or is stopped. */
  streaming: Readonly<Ref<boolean>>;
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

/**
 * Consumes a stream (Server-Sent Events or a WebSocket channel) and re-opens it when the canonical
 * bytes of the request change or `restart` is called. The previous stream is aborted on a change
 * and when the scope is disposed. A `null` request holds it back.
 */
export function useStream<Req, T, Out = never>(
  open: (req: Req, init: CallInit & { reconnect?: Reconnect | false }) => AsyncIterable<T>,
  req: MaybeRefOrGetter<Req | null>,
  options: { enabled?: MaybeRefOrGetter<boolean>; accumulate?: boolean; max?: number; reconnect?: Reconnect | false } = {},
): ChannelResult<T, Out> {
  const items = shallowRef<T[]>([]);
  const latest = shallowRef<T | undefined>(undefined);
  const error = shallowRef<unknown>(undefined);
  const streaming = shallowRef(false);
  let aborter: Aborter | null = null;
  let opened: Opened<T> | null = null;

  const run = (): void => {
    aborter?.abort();
    aborter = null;
    opened = null;
    const current = toValue(req);
    if (current === null || toValue(options.enabled ?? true) === false) {
      streaming.value = false;
      return;
    }
    const mine = newAborter();
    aborter = mine;
    items.value = [];
    latest.value = undefined;
    error.value = undefined;
    streaming.value = true;
    const stream = open(current, { signal: mine.signal, reconnect: options.reconnect ?? {} }) as Opened<T>;
    opened = stream;
    void (async () => {
      try {
        for await (const item of stream) {
          if (mine.signal.aborted) return;
          latest.value = item;
          if (options.accumulate !== false) {
            const kept = [...items.value, item];
            items.value = options.max !== undefined && kept.length > options.max ? kept.slice(-options.max) : kept;
          }
        }
      } catch (failure) {
        if (!mine.signal.aborted) error.value = failure;
      }
      if (!mine.signal.aborted) streaming.value = false;
    })();
  };

  watch(
    () => {
      const current = toValue(req);
      return current === null || toValue(options.enabled ?? true) === false ? null : canonical(current);
    },
    run,
    { immediate: true },
  );
  if (getCurrentScope()) onScopeDispose(() => aborter?.abort());
  const send = (message: Out): void => {
    if (!opened?.send) throw new Error('send: this stream is not an open WebSocket channel');
    opened.send(message as never);
  };
  return { items, latest, error, streaming, restart: run, send };
}
