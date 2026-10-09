import type { Session } from './transport.js';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';
export interface DaemonEvent { kind?: string; type?: string; [key: string]: unknown }
export interface Socket {
  onopen: ((event: Event) => void) | null;
  onmessage: ((event: MessageEvent) => void) | null;
  onerror: ((event: Event) => void) | null;
  onclose: ((event: CloseEvent) => void) | null;
  close(): void;
}
export interface EventOptions {
  createSocket?: (url: string, protocols: string[]) => Socket;
  onEvent: (event: DaemonEvent) => void;
  onState: (state: ConnectionState) => void;
  /** Refetch authoritative state on reconnect and while disconnected. */
  synchronize: () => Promise<void>;
  onError?: (error: unknown) => void;
  retryMs?: number;
  pollMs?: number;
}

export class EventBus {
  private socket: Socket | undefined;
  private retry: ReturnType<typeof setTimeout> | undefined;
  private poll: ReturnType<typeof setInterval> | undefined;
  private stopped = true;
  private attempts = 0;
  private synchronizing = false;
  constructor(private readonly session: Session, private readonly options: EventOptions) {}
  start(): void {
    if (!this.stopped) return;
    this.stopped = false;
    this.connect();
    this.poll = setInterval(() => void this.synchronize(), this.options.pollMs ?? 3000);
  }
  stop(): void {
    this.stopped = true;
    clearTimeout(this.retry);
    clearInterval(this.poll);
    const socket = this.socket;
    this.socket = undefined;
    socket?.close();
    this.options.onState('disconnected');
  }
  private async synchronize(): Promise<void> {
    if (this.stopped || this.synchronizing) return;
    this.synchronizing = true;
    try { await this.options.synchronize(); }
    catch (error) { this.options.onError?.(error); }
    finally { this.synchronizing = false; }
  }
  private schedule(): void {
    if (this.stopped || this.retry !== undefined) return;
    this.options.onState('reconnecting');
    const delay = Math.min((this.options.retryMs ?? 500) * 2 ** this.attempts++, 15000);
    this.retry = setTimeout(() => { this.retry = undefined; this.connect(); }, delay);
  }
  private connect(): void {
    if (this.stopped) return;
    this.options.onState(this.attempts ? 'reconnecting' : 'connecting');
    const url = new URL('/api/v1/events', this.session.base_url);
    url.protocol = 'ws:';
    try {
      const createSocket = this.options.createSocket ?? ((address, protocols) => new WebSocket(address, protocols));
      const socket = createSocket(url.toString(), ['iris', `iris-token.${this.session.token}`]);
      this.socket = socket;
      socket.onopen = () => {
        if (this.socket !== socket || this.stopped) return;
        this.attempts = 0;
        this.options.onState('connected');
        void this.synchronize();
      };
      socket.onmessage = event => {
        if (this.socket !== socket || this.stopped) return;
        try {
          const data: unknown = JSON.parse(String(event.data));
          if (typeof data !== 'object' || data === null || Array.isArray(data)) throw new Error('Invalid daemon event');
          this.options.onEvent(data as DaemonEvent);
        } catch (error) { this.options.onError?.(error); }
      };
      socket.onerror = () => { socket.close(); };
      socket.onclose = () => {
        if (this.socket !== socket || this.stopped) return;
        this.socket = undefined;
        this.schedule();
      };
    } catch (error) { this.options.onError?.(error); this.schedule(); }
  }
}
