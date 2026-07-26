import { redactUnknown } from "./redaction.js";

export type RuntimeErrorCode =
  | "bad_request"
  | "config_invalid"
  | "context_atomic_group_invalid"
  | "context_budget_exceeded"
  | "context_conflict"
  | "context_content_hash_mismatch"
  | "context_finalize_mismatch"
  | "context_schema_invalid"
  | "definition_contract_error"
  | "definition_rebind_required"
  | "internal_error"
  | "audit_persistence_failed"
  | "audit_terminal_conflict"
  | "model_capability_mismatch"
  | "model_incompatible"
  | "model_not_found"
  | "tokenizer_profile_unavailable"
  | "not_found"
  | "required_context_unavailable"
  | "session_busy"
  | "session_not_found"
  | "session_not_running"
  | "session_migration_required"
  | "model_rebind_required"
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
