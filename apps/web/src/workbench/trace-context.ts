const hex = (bytes: Uint8Array) =>
  Array.from(bytes, (value) => value.toString(16).padStart(2, "0")).join("");

function randomHex(bytes: number): string {
  const value = new Uint8Array(bytes);
  globalThis.crypto.getRandomValues(value);
  return hex(value);
}

export function createTraceparent(): string {
  return `00-${randomHex(16)}-${randomHex(8)}-01`;
}

export function traceHeaders(): HeadersInit {
  return { traceparent: createTraceparent() };
}
