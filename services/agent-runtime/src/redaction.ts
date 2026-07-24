const SENSITIVE_FIELD = /^(api[-_]?key|api[-_]?secret|authorization|proxy[-_]?authorization|credential|password|secret|access[-_]?token|refresh[-_]?token|id[-_]?token|token|cookie|set[-_]?cookie)$/i;
const BEARER_VALUE = /Bearer\s+[A-Za-z0-9._~+\/-]+=*/gi;
const SENSITIVE_QUERY = /([?&](?:api[-_]?key|api[-_]?secret|access[-_]?token|token)=)[^&#\s]*/gi;
const URL_PASSWORD = /([a-z][a-z0-9+.-]*:\/\/[^\s:/]+:)[^\s@/]+@/gi;

export const REDACTED = "[REDACTED]";

function redactString(value: string, knownSecrets: readonly string[]): string {
  let redacted = value
    .replace(BEARER_VALUE, `Bearer ${REDACTED}`)
    .replace(SENSITIVE_QUERY, `$1${REDACTED}`)
    .replace(URL_PASSWORD, `$1${REDACTED}@`);
  for (const secret of knownSecrets) {
    if (secret.length > 0) redacted = redacted.replaceAll(secret, REDACTED);
  }
  return redacted;
}

export function redactUrl(value: string): string {
  try {
    const url = new URL(value);
    if (url.password) url.password = REDACTED;
    for (const key of [...url.searchParams.keys()]) {
      if (SENSITIVE_FIELD.test(key)) url.searchParams.set(key, REDACTED);
    }
    return url.toString().replace(/\/$/, value.endsWith("/") ? "/" : "");
  } catch {
    return redactString(value, []);
  }
}

export function redactUnknown(value: unknown, knownSecrets: readonly string[] = []): unknown {
  if (typeof value === "string") return redactString(value, knownSecrets);
  if (Array.isArray(value)) return value.map((item) => redactUnknown(item, knownSecrets));
  if (value instanceof Error) {
    return { name: value.name, message: redactString(value.message, knownSecrets) };
  }
  if (value === null || typeof value !== "object") return value;

  return Object.fromEntries(
    Object.entries(value).map(([key, item]) => [
      key,
      SENSITIVE_FIELD.test(key) ? REDACTED : redactUnknown(item, knownSecrets),
    ]),
  );
}

export function safeJson(value: unknown, knownSecrets: readonly string[] = []): string {
  return JSON.stringify(redactUnknown(value, knownSecrets));
}
