"""确定性 Mock Provider：不导入 SDK，不发起网络请求。"""

from __future__ import annotations

import json
from base64 import b64encode
from hashlib import sha256
from uuid import NAMESPACE_URL, uuid5

from video_agent_api.logging import log_event
from video_agent_api.ports.contracts import ModelSelection, PortResult


def build_mock_text_output(prompt: str) -> dict[str, object]:
    request = json.loads(prompt)
    if not isinstance(request, dict):
        raise ValueError("mock text prompt must be an object")
    snapshot = request.get("inputSnapshot", request)
    if not isinstance(snapshot, dict) or not isinstance(snapshot.get("creativeBrief"), dict):
        raise ValueError("mock text prompt is missing CreativeBrief")
    brief = snapshot["creativeBrief"]
    project_id = str(request.get("projectId") or brief.get("projectId") or "")
    run_id = str(request.get("runId") or sha256(prompt.encode()).hexdigest())
    if not project_id:
        raise ValueError("mock text prompt is missing project scope")
    candidates: list[dict[str, object]] = []

    def add(
        key: str,
        kind: str,
        scope_id: str,
        source_keys: list[str],
        payload: dict[str, object],
    ) -> None:
        candidates.append(
            {
                "key": key,
                "kind": kind,
                "scopeId": scope_id,
                "sourceKeys": source_keys,
                "payload": payload,
            }
        )

    add("story", "story_spec", project_id, [], {"logline": brief["subject"]})
    entry_ids = {
        entry_type: str(uuid5(NAMESPACE_URL, f"{run_id}:asset-bible:{entry_type}"))
        for entry_type in (
            "character",
            "look",
            "location",
            "scene_visual",
            "prop",
            "visual_style",
        )
    }
    entry_attributes: dict[str, dict[str, object]] = {
        "character": {"name": brief["characterPremise"]},
        "look": {
            "name": f"{brief['style']} character look",
            "characterEntryId": entry_ids["character"],
        },
        "location": {"name": brief["subject"]},
        "scene_visual": {
            "name": f"{brief['style']} scene visual",
            "locationEntryId": entry_ids["location"],
        },
        "prop": {"name": f"{brief['subject']} key prop"},
        "visual_style": {"name": brief["style"]},
    }
    for entry_type, stable_id in entry_ids.items():
        add(
            f"asset-bible:{entry_type}",
            "asset_bible_spec",
            project_id,
            ["story"],
            {
                "entryType": entry_type,
                "stableId": stable_id,
                "attributes": entry_attributes[entry_type],
            },
        )
    episode_count = int(brief["episodeCount"])
    scenes_per_episode = int(brief["scenesPerEpisode"])
    shots_per_scene = int(brief["shotsPerScene"])
    for episode_number in range(1, episode_count + 1):
        episode_id = str(uuid5(NAMESPACE_URL, f"{run_id}:episode:{episode_number}"))
        script_key = f"script:{episode_number}"
        episode_key = f"episode:{episode_number}"
        add(
            script_key,
            "script_spec",
            episode_id,
            ["story"],
            {
                "episodeNumber": episode_number,
                "durationSeconds": int(brief["episodeDurationSeconds"]),
            },
        )
        add(
            episode_key,
            "episode",
            episode_id,
            [script_key],
            {"episodeNumber": episode_number},
        )
        for scene_number in range(1, scenes_per_episode + 1):
            scene_id = str(
                uuid5(
                    NAMESPACE_URL,
                    f"{run_id}:episode:{episode_number}:scene:{scene_number}",
                )
            )
            scene_key = f"scene:{episode_number}:{scene_number}"
            add(
                scene_key,
                "scene",
                scene_id,
                [episode_key],
                {"episodeId": episode_id, "sceneNumber": scene_number},
            )
            for shot_number in range(1, shots_per_scene + 1):
                shot_id = str(
                    uuid5(
                        NAMESPACE_URL,
                        f"{run_id}:episode:{episode_number}:scene:{scene_number}:"
                        f"shot:{shot_number}",
                    )
                )
                shot_key = f"shot:{episode_number}:{scene_number}:{shot_number}"
                add(
                    shot_key,
                    "shot",
                    shot_id,
                    [scene_key],
                    {"sceneId": scene_id, "shotNumber": shot_number},
                )
                add(
                    f"shot-spec:{episode_number}:{scene_number}:{shot_number}",
                    "shot_spec",
                    shot_id,
                    [shot_key],
                    {
                        "durationFrames": int(brief["episodeDurationSeconds"])
                        * 30
                        // (scenes_per_episode * shots_per_scene),
                        "assetBibleRefs": list(entry_ids.values()),
                    },
                )
    return {"candidates": candidates}


class DeterministicMockProvider:
    """用稳定哈希生成 Port 成功结果，显式支持失败测试。"""

    def _result(
        self, operation: str, value: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        digest = sha256(
            f"{operation}|{value}|{selection.model_id}|{correlation_id}".encode()
        ).hexdigest()[:16]
        if value == "__mock_error__":
            error = RuntimeError(f"mock provider requested explicit error: {operation}")
            log_event(
                "provider.call",
                correlation_id=correlation_id,
                operation=operation,
                adapter="mock",
                result="error",
                error_type=type(error).__name__,
            )
            raise error
        result = PortResult(
            request_id=f"mock-{digest}",
            correlation_id=correlation_id,
            payload={"operation": operation, "result": digest, "model_id": selection.model_id},
        )
        log_event(
            "provider.call",
            correlation_id=correlation_id,
            operation=operation,
            adapter="mock",
            result="success",
        )
        return result

    def generate_text(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        result = self._result("text.generate", prompt, selection, correlation_id)
        try:
            result.payload["payload"] = build_mock_text_output(prompt)
        except (KeyError, TypeError, ValueError, json.JSONDecodeError):
            # Generic port smoke calls may not use the structured text contract.
            # TextGenerationService still rejects this payload before persistence.
            pass
        return result

    def generate_image(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        return self._image_result("image.generate", prompt, selection, correlation_id)

    def edit_image(self, prompt: str, selection: ModelSelection, correlation_id: str) -> PortResult:
        return self._image_result("image.edit", prompt, selection, correlation_id)

    def _image_result(
        self, operation: str, value: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        result = self._result(operation, value, selection, correlation_id)
        # A deterministic 1x1 PNG keeps the explicit Mock path useful without
        # implying that a live Provider succeeded or persisting media bytes in DB.
        pixel = (
            b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01"
            b"\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\x0dIDAT\x08\xd7"
            b"c\xf8\xcf\xc0\xf0\x1f\x00\x05\x00\x01\xff\x89\x99=\x1d\x00\x00\x00"
            b"\x00IEND\xaeB`\x82"
        )
        result.payload.update(
            {
                "base64": b64encode(pixel).decode("ascii"),
                "mimeType": "image/png",
                "sizeBytes": len(pixel),
                "width": 1,
                "height": 1,
            }
        )
        return result

    def submit_video(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        return self._result("video.submit", prompt, selection, correlation_id)

    def get_video_status(self, job_id: str, correlation_id: str) -> PortResult:
        return self._result(
            "video.status", job_id, ModelSelection("mock", "mock", "mock", "mock"), correlation_id
        )

    def cancel_video(self, job_id: str, correlation_id: str) -> PortResult:
        return self._result(
            "video.cancel", job_id, ModelSelection("mock", "mock", "mock", "mock"), correlation_id
        )

    def synthesize(self, text: str, selection: ModelSelection, correlation_id: str) -> PortResult:
        return self._result("tts.synthesize", text, selection, correlation_id)

    def transcribe(
        self, object_ref: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        return self._result("asr.transcribe", object_ref, selection, correlation_id)
