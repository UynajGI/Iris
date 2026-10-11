export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly body: unknown
  ) {
    const detail = errorDetail(body);
    super(`Iris API request failed (${status})${detail ? `: ${detail}` : ''}`);
    this.name = 'ApiError';
  }
}

function errorDetail(body: unknown): string {
  if (typeof body === 'object' && body !== null && 'error' in body) return String(body.error);
  return typeof body === 'string' ? body : '';
}
