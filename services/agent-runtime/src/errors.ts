import { redactUnknown } from "./redaction.js";

export type RuntimeErrorCode =
  | "bad_request"
  | "config_invalid"
  | "internal_error"
  | "model_incompatible"
  | "model_not_found"
  | "not_found"
  | "session_busy"
  | "session_not_found"
  | "session_not_running"
  | "storage_unavailable";

export class RuntimeError extends Error {
  constructor(
    public readonly code: RuntimeErrorCode,
    public readonly status: number,
    message: string,
    options?: ErrorOptions,
  ) {
    super(message, options);
    this.name = "RuntimeError";
  }
}

export interface PublicError {
  error: {
    code: RuntimeErrorCode;
    message: string;
  };
}

export function normalizeError(error: unknown): RuntimeError {
  if (error instanceof RuntimeError) return error;
  const redacted = redactUnknown(error instanceof Error ? error.message : String(error));
  return new RuntimeError("internal_error", 500, String(redacted));
}

export function publicError(error: unknown): PublicError {
  const normalized = normalizeError(error);
  return { error: { code: normalized.code, message: normalized.message } };
}
