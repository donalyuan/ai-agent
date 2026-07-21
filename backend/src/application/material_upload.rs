//! 用户上传素材的内容识别、媒体探测与本地文件存储。

use crate::repositories::MaterialType;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::{fs, process::Command};
use uuid::Uuid;

pub const MAX_UPLOAD_BYTES: usize = 500 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct DetectedMaterial {
    pub material_type: MaterialType,
    pub extension: String,
    pub mime_type: String,
    pub format: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MediaProbe {
    pub duration_sec: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UploadValidationError {
    EmptyFile,
    FileTooLarge,
    UnsupportedFileType,
    InvalidFileContent,
    InvalidUtf8Subtitle,
    InvalidProbeOutput,
}

impl std::fmt::Display for UploadValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::EmptyFile => "上传文件不能为空",
            Self::FileTooLarge => "上传文件不能超过 500 MiB",
            Self::UnsupportedFileType => "不支持该文件类型",
            Self::InvalidFileContent => "文件内容无效或与扩展名不匹配",
            Self::InvalidUtf8Subtitle => "字幕文件必须使用 UTF-8 编码",
            Self::InvalidProbeOutput => "无法读取音视频文件信息",
        })
    }
}

impl std::error::Error for UploadValidationError {}

pub fn inspect_upload(
    file_name: &str,
    _declared_content_type: Option<&str>,
    bytes: &[u8],
) -> Result<DetectedMaterial, UploadValidationError> {
    if bytes.is_empty() {
        return Err(UploadValidationError::EmptyFile);
    }
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(UploadValidationError::FileTooLarge);
    }

    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or(UploadValidationError::UnsupportedFileType)?;
    match extension.as_str() {
        "jpg" | "jpeg" | "png" | "webp" | "gif" => {
            let canonical_extension = if extension == "jpeg" {
                "jpg"
            } else {
                &extension
            };
            let image_format = match canonical_extension {
                "jpg" => image::ImageFormat::Jpeg,
                "png" => image::ImageFormat::Png,
                "webp" => image::ImageFormat::WebP,
                "gif" => image::ImageFormat::Gif,
                _ => unreachable!(),
            };
            let decoded = image::load_from_memory_with_format(bytes, image_format)
                .map_err(|_| UploadValidationError::InvalidFileContent)?;
            Ok(DetectedMaterial {
                material_type: MaterialType::Image,
                extension: canonical_extension.to_string(),
                mime_type: image_mime(canonical_extension).to_string(),
                format: canonical_extension.to_string(),
                width: Some(decoded.width()),
                height: Some(decoded.height()),
            })
        }
        "mp4" | "mov" | "webm" => Ok(DetectedMaterial {
            material_type: MaterialType::Video,
            extension: extension.clone(),
            mime_type: mime_guess::from_ext(&extension)
                .first_or_octet_stream()
                .to_string(),
            format: extension,
            width: None,
            height: None,
        }),
        "mp3" | "wav" | "m4a" | "ogg" => Ok(DetectedMaterial {
            material_type: MaterialType::Audio,
            extension: extension.clone(),
            mime_type: mime_guess::from_ext(&extension)
                .first_or_octet_stream()
                .to_string(),
            format: extension,
            width: None,
            height: None,
        }),
        "srt" | "vtt" | "ass" | "ssa" => {
            std::str::from_utf8(bytes).map_err(|_| UploadValidationError::InvalidUtf8Subtitle)?;
            Ok(DetectedMaterial {
                material_type: MaterialType::Subtitle,
                extension: extension.clone(),
                mime_type: subtitle_mime(&extension).to_string(),
                format: extension,
                width: None,
                height: None,
            })
        }
        _ => Err(UploadValidationError::UnsupportedFileType),
    }
}

fn image_mime(extension: &str) -> &'static str {
    match extension {
        "jpg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

fn subtitle_mime(extension: &str) -> &'static str {
    match extension {
        "vtt" => "text/vtt",
        "srt" => "application/x-subrip",
        "ass" | "ssa" => "text/x-ssa",
        _ => "text/plain",
    }
}

#[derive(Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    format: FfprobeFormat,
    #[serde(default)]
    streams: Vec<FfprobeStream>,
}

#[derive(Default, Deserialize)]
struct FfprobeFormat {
    format_name: Option<String>,
    duration: Option<String>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

pub fn parse_ffprobe_output(bytes: &[u8]) -> Result<MediaProbe, UploadValidationError> {
    let output: FfprobeOutput =
        serde_json::from_slice(bytes).map_err(|_| UploadValidationError::InvalidProbeOutput)?;
    let video = output
        .streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("video"));
    let duration_sec = output
        .format
        .duration
        .as_deref()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0);

    Ok(MediaProbe {
        duration_sec,
        width: video.and_then(|stream| stream.width),
        height: video.and_then(|stream| stream.height),
        format_name: output.format.format_name,
    })
}

pub fn media_format_matches_extension(extension: &str, format_name: Option<&str>) -> bool {
    let expected = match extension {
        "mp4" => "mp4",
        "mov" => "mov",
        "webm" => "webm",
        "mp3" => "mp3",
        "wav" => "wav",
        "m4a" => "m4a",
        "ogg" => "ogg",
        _ => return false,
    };
    format_name
        .map(|value| value.split(',').any(|format| format == expected))
        .unwrap_or(false)
}

pub async fn probe_media(path: &Path) -> Result<MediaProbe, UploadValidationError> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=format_name,duration:stream=codec_type,width,height",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .await
        .map_err(|_| UploadValidationError::InvalidProbeOutput)?;
    if !output.status.success() {
        return Err(UploadValidationError::InvalidFileContent);
    }
    let probe = parse_ffprobe_output(&output.stdout)?;
    if probe.duration_sec.is_none() {
        return Err(UploadValidationError::InvalidFileContent);
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or(UploadValidationError::InvalidFileContent)?;
    if !media_format_matches_extension(&extension, probe.format_name.as_deref()) {
        return Err(UploadValidationError::InvalidFileContent);
    }
    Ok(probe)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredMaterialFile {
    pub absolute_path: PathBuf,
    pub public_url: String,
}

#[derive(Clone, Debug)]
pub struct LocalMaterialStorage {
    root: PathBuf,
}

impl LocalMaterialStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub async fn store(
        &self,
        project_id: Uuid,
        upload_id: Uuid,
        extension: &str,
        bytes: &[u8],
    ) -> Result<StoredMaterialFile, std::io::Error> {
        let directory = self.root.join("uploads").join(project_id.to_string());
        fs::create_dir_all(&directory).await?;
        let file_name = format!("{upload_id}.{extension}");
        let absolute_path = directory.join(&file_name);
        fs::write(&absolute_path, bytes).await?;
        Ok(StoredMaterialFile {
            absolute_path,
            public_url: format!("/assets/uploads/{project_id}/{file_name}"),
        })
    }

    pub async fn store_generated(
        &self,
        project_id: Uuid,
        artifact_id: Uuid,
        extension: &str,
        bytes: &[u8],
    ) -> Result<StoredMaterialFile, std::io::Error> {
        let directory = self
            .root
            .join("generated")
            .join("artifacts")
            .join(project_id.to_string());
        fs::create_dir_all(&directory).await?;
        let file_name = format!("{artifact_id}.{extension}");
        let absolute_path = directory.join(&file_name);
        fs::write(&absolute_path, bytes).await?;
        Ok(StoredMaterialFile {
            absolute_path,
            public_url: format!("/assets/generated/artifacts/{project_id}/{file_name}"),
        })
    }

    pub async fn store_generated_thumbnail(
        &self,
        project_id: Uuid,
        artifact_id: Uuid,
        bytes: &[u8],
    ) -> Result<StoredMaterialFile, std::io::Error> {
        let directory = self
            .root
            .join("generated")
            .join("artifacts")
            .join(project_id.to_string());
        fs::create_dir_all(&directory).await?;
        let file_name = format!("{artifact_id}.jpg");
        let absolute_path = directory.join(&file_name);
        fs::write(&absolute_path, bytes).await?;
        Ok(StoredMaterialFile {
            absolute_path,
            public_url: format!("/assets/generated/artifacts/{project_id}/{file_name}"),
        })
    }

    pub async fn store_upload_thumbnail(
        &self,
        project_id: Uuid,
        upload_id: Uuid,
        bytes: &[u8],
    ) -> Result<StoredMaterialFile, std::io::Error> {
        let directory = self.root.join("uploads").join(project_id.to_string());
        fs::create_dir_all(&directory).await?;
        let file_name = format!("{upload_id}.jpg");
        let absolute_path = directory.join(&file_name);
        fs::write(&absolute_path, bytes).await?;
        Ok(StoredMaterialFile {
            absolute_path,
            public_url: format!("/assets/uploads/{project_id}/{file_name}"),
        })
    }

    pub async fn remove(&self, stored: &StoredMaterialFile) -> Result<(), std::io::Error> {
        match fs::remove_file(&stored.absolute_path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}
