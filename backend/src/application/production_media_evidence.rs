//! 使用本地 ffprobe 读取自管成片，并输出版本化、脱敏的媒体能力证据。

use async_trait::async_trait;
use novex_production_crew::{
    durable::media::RequiredTakeInventorySnapshot,
    orchestrator::application_port::{
        MediaEvidenceAnalysis, MediaEvidenceProvider, TemporaryMediaAccess,
    },
    ProductionError, ProductionResult,
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    path::{Component, Path, PathBuf},
    process::Stdio,
};
use tokio::{io::AsyncReadExt, process::Command};
use url::Url;

const FFPROBE_BINARY: &str = "/usr/bin/ffprobe";

#[derive(Clone, Debug)]
pub struct LocalFfprobeMediaEvidenceProvider {
    storage_root: PathBuf,
}

impl LocalFfprobeMediaEvidenceProvider {
    pub fn new(storage_root: impl Into<PathBuf>) -> Self {
        Self {
            storage_root: storage_root.into(),
        }
    }

    async fn resolve_managed_path(
        &self,
        access: &TemporaryMediaAccess,
    ) -> ProductionResult<PathBuf> {
        if !access.request_headers.is_empty() {
            return Err(evidence_blocker("temporary_media_headers_not_allowed"));
        }
        let raw = access.access_url.trim();
        if raw.is_empty() {
            return Err(evidence_blocker("temporary_media_access_missing"));
        }

        let requested = match Url::parse(raw) {
            Ok(url) => {
                if url.scheme() != "file"
                    || url.host().is_some()
                    || url.query().is_some()
                    || url.fragment().is_some()
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || url.port().is_some()
                {
                    return Err(evidence_blocker("temporary_media_access_not_local"));
                }
                url.to_file_path()
                    .map_err(|_| evidence_blocker("temporary_media_access_invalid"))?
            }
            Err(_) => {
                let path = Path::new(raw);
                if path.components().any(|component| {
                    matches!(component, Component::ParentDir | Component::Prefix(_))
                }) {
                    return Err(evidence_blocker("temporary_media_path_escape"));
                }
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    self.storage_root.join(path)
                }
            }
        };

        let root = tokio::fs::canonicalize(&self.storage_root)
            .await
            .map_err(|_| ProductionError::CapabilityMismatch {
                reason: "managed_media_root_unavailable".into(),
            })?;
        let resolved = tokio::fs::canonicalize(requested)
            .await
            .map_err(|_| evidence_blocker("managed_media_not_found"))?;
        if !resolved.starts_with(&root) {
            return Err(evidence_blocker("temporary_media_path_escape"));
        }
        let metadata = tokio::fs::metadata(&resolved)
            .await
            .map_err(|_| evidence_blocker("managed_media_not_found"))?;
        if !metadata.is_file() {
            return Err(evidence_blocker("managed_media_not_file"));
        }
        Ok(resolved)
    }

    async fn capability_version(&self) -> ProductionResult<String> {
        let output = Command::new(FFPROBE_BINARY)
            .arg("-version")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .await
            .map_err(|_| ProductionError::CapabilityMismatch {
                reason: "ffprobe_unavailable".into(),
            })?;
        if !output.status.success() {
            return Err(ProductionError::CapabilityMismatch {
                reason: "ffprobe_unavailable".into(),
            });
        }
        let version = std::str::from_utf8(&output.stdout)
            .ok()
            .and_then(|stdout| stdout.lines().next())
            .and_then(|line| line.split_whitespace().nth(2))
            .map(|raw| {
                raw.chars()
                    .take_while(|character| character.is_ascii_digit() || *character == '.')
                    .collect::<String>()
            })
            .and_then(|numeric| {
                let mut parts = numeric.split('.');
                let major = parts.next()?.parse::<u32>().ok()?;
                let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
                Some(format!("{major}.{minor}"))
            })
            .ok_or_else(|| ProductionError::CapabilityMismatch {
                reason: "ffprobe_version_unavailable".into(),
            })?;
        Ok(version)
    }

    async fn probe(&self, path: &Path) -> ProductionResult<FfprobeOutput> {
        let output = Command::new(FFPROBE_BINARY)
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=format_name,duration:stream=codec_type,codec_name,width,height,sample_rate,channels",
                "-of",
                "json",
            ])
            .arg(path)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .await
            .map_err(|_| ProductionError::CapabilityMismatch {
                reason: "ffprobe_unavailable".into(),
            })?;
        if !output.status.success() {
            return Err(evidence_blocker("ffprobe_media_unreadable"));
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|_| evidence_blocker("ffprobe_output_invalid"))
    }
}

#[async_trait]
impl MediaEvidenceProvider for LocalFfprobeMediaEvidenceProvider {
    async fn inspect_media(
        &self,
        inventory: RequiredTakeInventorySnapshot,
        access: TemporaryMediaAccess,
    ) -> ProductionResult<MediaEvidenceAnalysis> {
        inventory.validate()?;
        if access.asset_id != inventory.final_asset.artifact_id {
            return Err(evidence_blocker("temporary_media_asset_mismatch"));
        }
        let path = self.resolve_managed_path(&access).await?;
        let capability_version = self.capability_version().await?;
        let actual_sha256 = sha256_file(&path).await?;
        if actual_sha256 != inventory.final_asset.sha256 {
            return Err(evidence_blocker("managed_media_hash_mismatch"));
        }
        let probe = self.probe(&path).await?;
        validate_container_mime(
            &inventory.final_asset.mime_type,
            probe.format.format_name.as_deref(),
        )?;

        let duration_ms = parse_duration_ms(probe.format.duration.as_deref())
            .ok_or_else(|| evidence_blocker("media_duration_missing"))?;
        let expected_duration_ms = inventory.final_asset.duration_ms;
        let tolerance_ms = 1_000_u64.max(expected_duration_ms / 20);
        if duration_ms.abs_diff(expected_duration_ms) > tolerance_ms {
            return Err(evidence_blocker("media_duration_mismatch"));
        }

        let video_streams = probe
            .streams
            .iter()
            .filter(|stream| stream.codec_type.as_deref() == Some("video"))
            .map(|stream| {
                let codec_name = non_empty(stream.codec_name.as_deref())
                    .ok_or_else(|| evidence_blocker("video_stream_metadata_invalid"))?;
                let width = stream
                    .width
                    .filter(|value| *value > 0)
                    .ok_or_else(|| evidence_blocker("video_stream_metadata_invalid"))?;
                let height = stream
                    .height
                    .filter(|value| *value > 0)
                    .ok_or_else(|| evidence_blocker("video_stream_metadata_invalid"))?;
                Ok(json!({
                    "codec_name": codec_name,
                    "width": width,
                    "height": height,
                }))
            })
            .collect::<ProductionResult<Vec<_>>>()?;
        if video_streams.is_empty() {
            return Err(evidence_blocker("video_stream_missing"));
        }

        let audio_streams = probe
            .streams
            .iter()
            .filter(|stream| stream.codec_type.as_deref() == Some("audio"))
            .map(|stream| {
                let codec_name = non_empty(stream.codec_name.as_deref())
                    .ok_or_else(|| evidence_blocker("audio_stream_metadata_invalid"))?;
                let sample_rate_hz = stream
                    .sample_rate
                    .as_deref()
                    .and_then(|value| value.parse::<u32>().ok())
                    .filter(|value| *value > 0)
                    .ok_or_else(|| evidence_blocker("audio_stream_metadata_invalid"))?;
                let channels = stream
                    .channels
                    .filter(|value| *value > 0)
                    .ok_or_else(|| evidence_blocker("audio_stream_metadata_invalid"))?;
                Ok(json!({
                    "codec_name": codec_name,
                    "sample_rate_hz": sample_rate_hz,
                    "channels": channels,
                }))
            })
            .collect::<ProductionResult<Vec<_>>>()?;
        if audio_streams.is_empty() {
            return Err(evidence_blocker("audio_stream_missing"));
        }

        let takes = inventory
            .takes
            .iter()
            .map(|take| {
                let shot_contract_count = take.scene_shot_map.values().map(Vec::len).sum::<usize>();
                json!({
                    "take_id": take.take_id,
                    "result": "covered_by_final_compose",
                    "scene_count": take.scene_ids.len(),
                    "shot_contract_count": shot_contract_count,
                })
            })
            .collect::<Vec<_>>();
        Ok(MediaEvidenceAnalysis {
            vision_capability_version: format!("ffprobe-vision@{capability_version}"),
            audio_capability_version: format!("ffprobe-audio@{capability_version}"),
            redacted_analysis: json!({
                "schema_version": "local_ffprobe_media_evidence@1",
                "final_media": {
                    "result": "readable",
                    "duration_ms": duration_ms,
                    "video_stream_count": video_streams.len(),
                    "audio_stream_count": audio_streams.len(),
                    "video_streams": video_streams,
                    "audio_streams": audio_streams,
                },
                "takes": takes,
            }),
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    format: FfprobeFormat,
    #[serde(default)]
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeFormat {
    format_name: Option<String>,
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    sample_rate: Option<String>,
    channels: Option<u32>,
}

async fn sha256_file(path: &Path) -> ProductionResult<String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|_| evidence_blocker("managed_media_not_found"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|_| evidence_blocker("managed_media_unreadable"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_container_mime(mime_type: &str, format_name: Option<&str>) -> ProductionResult<()> {
    let expected = match mime_type {
        "video/mp4" => "mp4",
        "video/quicktime" => "mov",
        "video/webm" => "webm",
        "video/x-matroska" => "matroska",
        "video/x-msvideo" => "avi",
        "video/mpeg" => "mpeg",
        "video/ogg" => "ogg",
        _ => {
            return Err(ProductionError::CapabilityMismatch {
                reason: "ffprobe_mime_unsupported".into(),
            })
        }
    };
    let matches =
        format_name.is_some_and(|formats| formats.split(',').any(|format| format == expected));
    if !matches {
        return Err(evidence_blocker("media_container_mime_mismatch"));
    }
    Ok(())
}

fn parse_duration_ms(duration: Option<&str>) -> Option<u64> {
    duration?
        .parse::<f64>()
        .ok()
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .map(|seconds| (seconds * 1_000.0).round() as u64)
        .filter(|milliseconds| *milliseconds > 0)
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.trim().is_empty())
}

fn evidence_blocker(reason: &'static str) -> ProductionError {
    ProductionError::EvidenceBlocker {
        reason: reason.into(),
        details: json!({"blocker": reason}),
    }
}
