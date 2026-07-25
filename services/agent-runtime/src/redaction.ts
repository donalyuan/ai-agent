const SENSITIVE_FIELD = /^(api[-_]?key|api[-_]?secret|authorization|proxy[-_]?authorization|credential|credentials|password|secret|signature|sig|access[-_]?token|refresh[-_]?token|id[-_]?token|token|cookie|set[-_]?cookie|x-amz-signature|headers|raw[-_]?headers|request[-_]?headers)$/i;
const BEARER_VALUE = /Bearer\s+[A-Za-z0-9._~+\/-]+=*/gi;
const SENSITIVE_HEADER = /(?:authorization|proxy-authorization|cookie|set-cookie)\s*:\s*[^\r\n]*/gi;
const SENSITIVE_QUERY = /([?&](?:api[-_]?key|api[-_]?secret|access[-_]?token|token|signature|sig|x-amz-signature)=)[^&#\s]*/gi;
const URL_PASSWORD = /([a-z][a-z0-9+.-]*:\/\/[^\s:/]+:)[^\s@/]+@/gi;
const CANARY_SECRET = /NOVEX_CANARY_SECRET_DO_NOT_PERSIST_[A-Za-z0-9_-]+/g;

export const REDACTED = "[REDACTED]";
export const MODEL_CALL_SCHEMA_VERSION = "1";

function redactString(value: string, knownSecrets: readonly string[]): string {
  let redacted = value
    .replace(BEARER_VALUE, `Bearer ${REDACTED}`)
    .replace(SENSITIVE_HEADER, REDACTED)
    .replace(SENSITIVE_QUERY, `$1${REDACTED}`)
    .replace(URL_PASSWORD, `$1${REDACTED}@`)
    .replace(CANARY_SECRET, REDACTED);
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
  try {
    return redactRuntimeValue(value, knownSecrets, new WeakSet<object>());
  } catch {
    return REDACTED;
  }
}

function redactRuntimeValue(value: unknown, knownSecrets: readonly string[], seen: WeakSet<object>): unknown {
  if (typeof value === "string") return redactString(value, knownSecrets);
  if (Array.isArray(value)) return value.map((item) => redactRuntimeValue(item, knownSecrets, seen));
  if (value instanceof Error) return { name: value.name, message: redactString(value.message, knownSecrets) };
  if (value === null || typeof value !== "object") return value;
  if (seen.has(value)) return REDACTED;
  seen.add(value);
  try {
    const object = value as Record<string, unknown>;
    if (object.secret === true) return REDACTED;
    return Object.fromEntries(Object.entries(object).map(([key, item]) => [
      key,
      SENSITIVE_FIELD.test(key) ? REDACTED : redactRuntimeValue(item, knownSecrets, seen),
    ]));
  } finally {
    seen.delete(value);
  }
}

/** Validates JSON audit input and applies the same irreversible redaction at every persistence boundary. */
export function redactForAudit(value: unknown, knownSecrets: readonly string[] = []): unknown {
  return redactAuditValue(value, knownSecrets, new WeakSet<object>());
}

function redactAuditValue(value: unknown, knownSecrets: readonly string[], seen: WeakSet<object>): unknown {
  if (typeof value === "string") return redactString(value, knownSecrets);
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error("audit value contains a non-finite number");
    return value;
  }
  if (typeof value === "boolean" || value === null) return value;
  if (value instanceof Error) return { name: value.name, message: redactString(value.message, knownSecrets) };
  if (typeof value !== "object") throw new Error("audit value is not JSON serializable");
  if (seen.has(value)) throw new Error("audit value contains a cycle");
  seen.add(value);
  try {
    if (Array.isArray(value)) {
      return value.map((item) => item === undefined ? null : redactAuditValue(item, knownSecrets, seen));
    }
    if (Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null) {
      throw new Error("audit value contains a non-plain object");
    }
    const object = value as Record<string, unknown>;
    if (object.secret === true) return REDACTED;
    return Object.fromEntries(Object.entries(object)
      .filter(([, item]) => item !== undefined)
      .map(([key, item]) => [
        key,
        SENSITIVE_FIELD.test(key) ? REDACTED : redactAuditValue(item, knownSecrets, seen),
      ]));
  } finally {
    seen.delete(value);
  }
}

export function safeJson(value: unknown, knownSecrets: readonly string[] = []): string {
  return JSON.stringify(redactUnknown(value, knownSecrets));
}
