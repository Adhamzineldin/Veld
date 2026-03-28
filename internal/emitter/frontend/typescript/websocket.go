package typescript

import "strings"

// veldWebSocketClass returns the TypeScript VeldWebSocket class source to be
// appended to client/api.ts (or _internal.ts).
func veldWebSocketClass() string {
	var sb strings.Builder
	sb.WriteString(`
export class VeldWebSocket<TReceive, TSend = unknown> {
  private ws: WebSocket | null = null;
  private readonly listeners: Array<(msg: TReceive) => void> = [];
  private reconnectDelay = 1000;
  private closed = false;

  constructor(private readonly url: string) {}

  connect(): this {
    this.closed = false;
    this.ws = new WebSocket(this.url);
    this.ws.onmessage = (e: MessageEvent) => {
      try {
        const msg = JSON.parse(e.data as string) as TReceive;
        for (const fn of this.listeners) fn(msg);
      } catch { /* ignore malformed frames */ }
    };
    this.ws.onclose = () => {
      if (!this.closed) {
        setTimeout(() => this.connect(), this.reconnectDelay);
        this.reconnectDelay = Math.min(this.reconnectDelay * 2, 30_000);
      }
    };
    return this;
  }

  onMessage(cb: (msg: TReceive) => void): this {
    this.listeners.push(cb);
    return this;
  }

  send(msg: TSend): void {
    this.ws?.send(JSON.stringify(msg));
  }

  close(): void {
    this.closed = true;
    this.ws?.close();
  }
}
`)
	return sb.String()
}
