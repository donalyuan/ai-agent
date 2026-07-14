use novex_api::application::material_upload::{
    inspect_upload, media_format_matches_extension, parse_ffprobe_output, LocalMaterialStorage,
    UploadValidationError,
};
use novex_api::repositories::MaterialType;
use std::fs;
use uuid::Uuid;

const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

#[test]
fn inspects_png_dimensions_and_system_metadata() {
    let detected = inspect_upload("cover.png", Some("image/png"), PNG_1X1).unwrap();

    assert_eq!(detected.material_type, MaterialType::Image);
    assert_eq!(detected.extension, "png");
    assert_eq!(detected.mime_type, "image/png");
    assert_eq!(detected.width, Some(1));
    assert_eq!(detected.height, Some(1));
    assert_eq!(detected.format, "png");
}

#[test]
fn rejects_truncated_image_even_when_dimensions_header_is_present() {
    let error = inspect_upload("cover.png", Some("image/png"), &PNG_1X1[..24]).unwrap_err();

    assert_eq!(error, UploadValidationError::InvalidFileContent);
}

#[test]
fn accepts_utf8_subtitle_and_rejects_unsupported_content() {
    let subtitle = inspect_upload(
        "demo.vtt",
        Some("text/vtt"),
        b"WEBVTT\n\n00:00.000 --> 00:01.000\nhello\n",
    )
    .unwrap();
    assert_eq!(subtitle.material_type, MaterialType::Subtitle);
    assert_eq!(subtitle.format, "vtt");

    let error = inspect_upload("payload.exe", None, b"MZ").unwrap_err();
    assert_eq!(error, UploadValidationError::UnsupportedFileType);
}

#[test]
fn parses_ffprobe_duration_and_video_dimensions() {
    let probe = parse_ffprobe_output(
        br#"{
          "format": {"format_name": "mov,mp4,m4a", "duration": "12.345"},
          "streams": [{"codec_type": "video", "width": 1920, "height": 1080}]
        }"#,
    )
    .unwrap();

    assert_eq!(probe.duration_sec, Some(12.345));
    assert_eq!(probe.width, Some(1920));
    assert_eq!(probe.height, Some(1080));
}

#[test]
fn matches_ffprobe_container_to_uploaded_extension() {
    assert!(media_format_matches_extension(
        "mp4",
        Some("mov,mp4,m4a,3gp,3g2,mj2")
    ));
    assert!(media_format_matches_extension(
        "m4a",
        Some("mov,mp4,m4a,3gp,3g2,mj2")
    ));
    assert!(media_format_matches_extension(
        "webm",
        Some("matroska,webm")
    ));
    assert!(!media_format_matches_extension(
        "mp4",
        Some("matroska,webm")
    ));
    assert!(!media_format_matches_extension("mp3", None));
}

#[test]
fn uses_canonical_mime_instead_of_untrusted_upload_declaration() {
    let detected = inspect_upload("clip.mp3", Some("audio/x-custom"), b"probe later").unwrap();

    assert_eq!(detected.mime_type, "audio/mpeg");
}

#[tokio::test]
async fn stores_and_removes_uploaded_file_under_project_directory() {
    let root = std::env::temp_dir().join(format!("novex-material-upload-{}", Uuid::new_v4()));
    let storage = LocalMaterialStorage::new(root.clone());
    let project_id = Uuid::new_v4();
    let upload_id = Uuid::new_v4();

    let stored = storage
        .store(project_id, upload_id, "png", PNG_1X1)
        .await
        .unwrap();

    assert!(stored.absolute_path.exists());
    assert_eq!(
        stored.public_url,
        format!("/assets/uploads/{project_id}/{upload_id}.png")
    );

    storage.remove(&stored).await.unwrap();
    assert!(!stored.absolute_path.exists());
    let _ = fs::remove_dir_all(root);
}
