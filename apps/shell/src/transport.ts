export interface Session { base_url: string; token: string; version: string }
export class ApiError extends Error {
  constructor(public readonly status: number, public readonly body: unknown) {
    const detail = typeof body === 'object' && body !== null && 'error' in body ? String(body.error) : typeof body === 'string' ? body : '';
    super(`Iris API request failed (${status})${detail ? `: ${detail}` : ''}`);
    this.name = 'ApiError';
  }
}
export type Fetch = typeof fetch;

/** Tokens remain in memory and headers: never URLs, local storage, or logs. */
export class Transport {
  readonly baseUrl: string;
  constructor(readonly session: Session, private readonly fetcher: Fetch = globalThis.fetch.bind(globalThis)) {
    const url = new URL(session.base_url);
    if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || url.username || url.password || url.search || url.hash) {
      throw new Error('The daemon must use an authenticated loopback HTTP address');
    }
    if (!session.token) throw new Error('Missing daemon session token');
    this.baseUrl = url.origin;
  }
  async request<T>(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<T> {
    if (!path.startsWith('/api/v1/')) throw new Error('Invalid API path');
    const response = await this.fetcher(this.baseUrl + path, {
      method,
      headers: { Authorization: `Bearer ${this.session.token}`, ...(body === undefined ? {} : { 'Content-Type': 'application/json' }) },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      ...(signal === undefined ? {} : { signal }),
      cache: 'no-store', redirect: 'error',
    });
    if (!response.ok) {
      const payload = await response.text();
      let parsed: unknown = payload;
      try { parsed = JSON.parse(payload); } catch { /* Plain text transport errors are valid. */ }
      throw new ApiError(response.status, parsed);
    }
    if (response.status === 204) return undefined as T;
    return response.json() as Promise<T>;
  }
  async blob(path: string, signal?: AbortSignal): Promise<Blob> {
    if (!path.startsWith('/api/v1/')) throw new Error('Invalid API path');
    const response = await this.fetcher(this.baseUrl + path, {
      headers: { Authorization: `Bearer ${this.session.token}` },
      ...(signal === undefined ? {} : { signal }), redirect: 'error', cache: 'no-store',
    });
    if (!response.ok) throw new ApiError(response.status, await response.text());
    return response.blob();
  }
}
