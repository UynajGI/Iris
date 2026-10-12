/** Convert only bounded inline JPEG evidence to a local blob allowed by the desktop CSP. */
export function nativeDetailBlob(value: string): Blob {
  const prefix = 'data:image/jpeg;base64,';
  if (!value.startsWith(prefix) || value.length > 16 * 1024 * 1024)
    throw new Error('Invalid native detail image');
  const binary = atob(value.slice(prefix.length));
  if (binary.length < 2 || binary.charCodeAt(0) !== 255 || binary.charCodeAt(1) !== 216)
    throw new Error('Native detail is not JPEG');
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; ++i) bytes[i] = binary.charCodeAt(i);
  return new Blob([bytes], { type: 'image/jpeg' });
}
