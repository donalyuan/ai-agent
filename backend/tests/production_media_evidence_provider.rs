use novex_api::application::production_media_evidence::LocalFfprobeMediaEvidenceProvider;
use novex_production_crew::{
    durable::media::{ComposeInput, FinalMediaAsset, RequiredTakeInventorySnapshot},
    orchestrator::application_port::{MediaEvidenceProvider, TemporaryMediaAccess},
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
use tokio::process::Command;
use uuid::Uuid;

async fn render_fixture(path: &Path, with_audio: bool) {
    let mut command = Command::new("ffmpeg");
    command.args([
        "-v",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "color=c=black:s=320x180:r=25:d=1",
    ]);
    if with_audio {
        command.args(["-f", "lavfi", "-i", "sine=frequency=440:duration=1"]);
    }
    command.args(["-c:v", "mpeg4", "-pix_fmt", "yuv420p"]);
    if with_audio {
        command.args(["-c:a", "aac", "-shortest"]);
    } else {
        command.arg("-an");
    }
    let output = command.arg(path).output().await.unwrap();
    assert!(
        output.status.success(),
        "ffmpeg fixture generation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn inventory_for(path: &Path) -> RequiredTakeInventorySnapshot {
    let sha256 = format!("{:x}", Sha256::digest(tokio::fs::read(path).await.unwrap()));
    let scene_id = Uuid::new_v4();
    RequiredTakeInventorySnapshot::build(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        1,
        0,
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        FinalMediaAsset {
            artifact_id: Uuid::new_v4(),
            sha256,
            mime_type: "video/mp4".into(),
            duration_ms: 1_000,
        },
        "a".repeat(64),
        vec![ComposeInput {
            generation_step_id: Uuid::new_v4(),
            generation_attempt_id: Uuid::new_v4(),
            output_artifact_id: Uuid::new_v4(),
            segment_key: "segment-1".into(),
            scene_ids: vec![scene_id],
            shot_contracts: vec![(scene_id, vec![Uuid::new_v4()])],
            consumed_by_final_compose: true,
            generation_succeeded: true,
        }],
    )
    .unwrap()
}

fn access(
    inventory: &RequiredTakeInventorySnapshot,
    path: impl Into<String>,
) -> TemporaryMediaAccess {
    TemporaryMediaAccess {
        asset_id: inventory.final_asset.artifact_id,
        access_url: path.into(),
        request_headers: BTreeMap::new(),
    }
}

#[tokio::test]
async fn local_ffprobe_provider_reads_managed_video_and_returns_only_redacted_evidence() {
    let root = std::env::temp_dir().join(format!("novex-media-evidence-{}", Uuid::new_v4()));
    let nested = root.join("managed");
    tokio::fs::create_dir_all(&nested).await.unwrap();
    let media = nested.join("final-secret-name.mp4");
    render_fixture(&media, true).await;
    let inventory = inventory_for(&media).await;
    let provider = LocalFfprobeMediaEvidenceProvider::new(&root);

    let result = provider
        .inspect_media(
            inventory.clone(),
            access(&inventory, "managed/final-secret-name.mp4"),
        )
        .await
        .unwrap();

    assert!(result
        .vision_capability_version
        .starts_with("ffprobe-vision@"));
    assert!(result
        .audio_capability_version
        .starts_with("ffprobe-audio@"));
    assert_eq!(
        result.redacted_analysis["final_media"]["video_stream_count"],
        1
    );
    assert_eq!(
        result.redacted_analysis["final_media"]["audio_stream_count"],
        1
    );
    assert_eq!(
        result.redacted_analysis["takes"][0]["take_id"],
        inventory.takes[0].take_id.to_string()
    );

    let encoded = serde_json::to_string(&result.redacted_analysis).unwrap();
    for forbidden in [
        root.to_string_lossy().as_ref(),
        "final-secret-name.mp4",
        "access_url",
        "request_headers",
        "authorization",
        "provider-secret",
        "http://",
        "https://",
        "file://",
    ] {
        assert!(!encoded
            .to_ascii_lowercase()
            .contains(&forbidden.to_ascii_lowercase()));
    }

    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn local_ffprobe_provider_rejects_media_without_audio_track() {
    let root = std::env::temp_dir().join(format!("novex-media-no-audio-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&root).await.unwrap();
    let media = root.join("silent.mp4");
    render_fixture(&media, false).await;
    let inventory = inventory_for(&media).await;
    let provider = LocalFfprobeMediaEvidenceProvider::new(&root);

    let error = provider
        .inspect_media(inventory.clone(), access(&inventory, "silent.mp4"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "evidence_blocker");
    assert!(error.to_string().contains("audio_stream_missing"));

    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn local_ffprobe_provider_rejects_remote_escape_and_credentials_before_probe() {
    let root = std::env::temp_dir().join(format!("novex-media-boundary-{}", Uuid::new_v4()));
    let outside = std::env::temp_dir().join(format!("novex-media-outside-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&root).await.unwrap();
    tokio::fs::create_dir_all(&outside).await.unwrap();
    let managed = root.join("managed.mp4");
    let escaped = outside.join("outside.mp4");
    render_fixture(&managed, true).await;
    tokio::fs::copy(&managed, &escaped).await.unwrap();
    let inventory = inventory_for(&managed).await;
    let provider = LocalFfprobeMediaEvidenceProvider::new(&root);

    for rejected in [
        "https://example.invalid/final.mp4?token=provider-secret".to_string(),
        format!(
            "../{}/outside.mp4",
            outside.file_name().unwrap().to_string_lossy()
        ),
        format!("file://{}", escaped.display()),
    ] {
        let error = provider
            .inspect_media(inventory.clone(), access(&inventory, rejected))
            .await
            .unwrap_err();
        assert_eq!(error.code(), "evidence_blocker");
        assert!(!error.to_string().contains("provider-secret"));
        assert!(!error
            .to_string()
            .contains(outside.to_string_lossy().as_ref()));
    }

    let mut credentialed = access(&inventory, "managed.mp4");
    credentialed
        .request_headers
        .insert("Authorization".into(), "Bearer provider-secret".into());
    let error = provider
        .inspect_media(inventory, credentialed)
        .await
        .unwrap_err();
    assert_eq!(error.code(), "evidence_blocker");
    assert!(!error.to_string().contains("provider-secret"));

    tokio::fs::remove_dir_all(root).await.unwrap();
    tokio::fs::remove_dir_all(outside).await.unwrap();
}
