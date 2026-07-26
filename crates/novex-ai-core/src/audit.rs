use serde_json::{Map, Value};
use std::fmt;
use url::Url;

pub const MODEL_CALL_SCHEMA_VERSION: &str = "1";
pub const GOVERNED_MODEL_CALL_SCHEMA_VERSION: &str = "2";
pub const AUDIT_REDACTED: &str = "[REDACTED]";

/// Redacts a JSON audit value without changing non-sensitive business text.
pub fn redact_audit_value(value: &Value, known_secrets: &[String]) -> Value {
    redact_value(value, known_secrets)
}

pub fn validate_audit_payload(value: &Value) -> Result<(), AuditValidationError> {
    match value {
        Value::String(value) => {
            if is_base64_payload(value) {
                return Err(AuditValidationError(
                    "audit payload contains base64 data".into(),
                ));
            }
            if is_temporary_signed_url(value) {
                return Err(AuditValidationError(
                    "audit payload contains a temporary signed URL".into(),
                ));
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_audit_payload(value)?;
            }
        }
        Value::Object(object) => {
            for value in object.values() {
                validate_audit_payload(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn validate_asset_references(value: &Value) -> Result<(), AuditValidationError> {
    let references = value
        .as_array()
        .ok_or_else(|| AuditValidationError("asset_references must be an array".into()))?;
    for reference in references {
        let object = reference
            .as_object()
            .ok_or_else(|| AuditValidationError("asset reference must be an object".into()))?;
        if object.keys().any(|key| {
            !matches!(
                key.as_str(),
                "asset_id" | "version" | "sha256" | "mime" | "metadata"
            )
        }) {
            return Err(AuditValidationError(
                "asset reference contains an unknown field".into(),
            ));
        }
        let required = ["asset_id", "version", "sha256", "mime"];
        if required.iter().any(|key| {
            object
                .get(*key)
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        }) {
            return Err(AuditValidationError(
                "asset reference is missing a required field".into(),
            ));
        }
        let digest = object["sha256"].as_str().unwrap_or_default();
        let mime = object["mime"].as_str().unwrap_or_default();
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || !matches!(mime.split_once('/'), Some(("image" | "audio" | "video", subtype)) if !subtype.is_empty())
            || object
                .get("metadata")
                .is_some_and(|value| !value.is_object())
        {
            return Err(AuditValidationError(
                "asset reference format is invalid".into(),
            ));
        }
        validate_audit_payload(reference)?;
    }
    Ok(())
}

fn redact_value(value: &Value, known_secrets: &[String]) -> Value {
    match value {
        Value::String(value) => Value::String(redact_string(value, known_secrets)),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| redact_value(value, known_secrets))
                .collect(),
        ),
        Value::Object(object) if object.get("secret") == Some(&Value::Bool(true)) => {
            Value::String(AUDIT_REDACTED.into())
        }
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let value = if sensitive_key(key) {
                        Value::String(AUDIT_REDACTED.into())
                    } else {
                        redact_value(value, known_secrets)
                    };
                    (key.clone(), value)
                })
                .collect::<Map<_, _>>(),
        ),
        value => value.clone(),
    }
}

fn sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "authorization"
            | "proxyauthorization"
            | "cookie"
            | "setcookie"
            | "credential"
            | "credentials"
            | "password"
            | "secret"
            | "signature"
            | "sig"
            | "token"
            | "headers"
            | "rawheaders"
            | "requestheaders"
    ) || normalized.contains("apikey")
        || normalized.contains("apisecret")
        || normalized.ends_with("token")
        || normalized.ends_with("secretkey")
        || normalized == "xamzsignature"
}

fn redact_string(value: &str, known_secrets: &[String]) -> String {
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("bearer ")
        || lower.contains("authorization:")
        || lower.contains("proxy-authorization:")
        || lower.contains("cookie:")
        || lower.contains("set-cookie:")
    {
        return AUDIT_REDACTED.into();
    }
    let mut redacted = redact_url(value).unwrap_or_else(|| value.to_string());
    redacted = redact_canaries(&redacted);
    for secret in known_secrets.iter().filter(|secret| !secret.is_empty()) {
        redacted = redacted.replace(secret, AUDIT_REDACTED);
    }
    redacted
}

fn redact_url(value: &str) -> Option<String> {
    let mut url = Url::parse(value).ok()?;
    if !url.username().is_empty() {
        let _ = url.set_username(AUDIT_REDACTED);
    }
    if url.password().is_some() {
        let _ = url.set_password(Some(AUDIT_REDACTED));
    }
    let pairs = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    if !pairs.is_empty() {
        url.query_pairs_mut()
            .clear()
            .extend_pairs(pairs.iter().map(|(key, value)| {
                (
                    key,
                    if sensitive_key(key) {
                        AUDIT_REDACTED
                    } else {
                        value
                    },
                )
            }));
    }
    Some(url.into())
}

fn redact_canaries(value: &str) -> String {
    const PREFIX: &str = "NOVEX_CANARY_SECRET_DO_NOT_PERSIST_";
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(index) = remaining.find(PREFIX) {
        output.push_str(&remaining[..index]);
        output.push_str(AUDIT_REDACTED);
        let suffix = &remaining[index + PREFIX.len()..];
        let end = suffix
            .find(|character: char| {
                !character.is_ascii_alphanumeric() && character != '_' && character != '-'
            })
            .unwrap_or(suffix.len());
        remaining = &suffix[end..];
    }
    output.push_str(remaining);
    output
}

fn is_base64_payload(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.to_ascii_lowercase().starts_with("data:")
        && trimmed[..trimmed.find(',').unwrap_or(trimmed.len())]
            .to_ascii_lowercase()
            .contains(";base64")
    {
        return true;
    }
    trimmed.len() > 4096
        && trimmed.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'\r' | b'\n')
        })
}

fn is_temporary_signed_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    url.query_pairs().any(|(key, _)| {
        matches!(
            key.to_ascii_lowercase().replace(['-', '_'], "").as_str(),
            "xamzsignature"
                | "xamzcredential"
                | "xamzexpires"
                | "xtossignature"
                | "xtoscredential"
                | "xtosexpires"
                | "signature"
        )
    })
}

#[derive(Debug)]
pub struct AuditValidationError(pub String);

impl fmt::Display for AuditValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AuditValidationError {}
