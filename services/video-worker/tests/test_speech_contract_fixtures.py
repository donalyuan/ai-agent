import json
from pathlib import Path


FIXTURES = Path(__file__).parent / "fixtures" / "speech"


def load_json(name: str) -> dict:
    return json.loads((FIXTURES / name).read_text(encoding="utf-8"))


def test_official_contract_sources_match_current_volcengine_protocols() -> None:
    sources = load_json("official_contract_sources.json")

    assert sources["list_speakers"]["version"] == "2025-05-20"
    assert sources["list_speakers"]["request_fields"] == [
        "ResourceIDs",
        "VoiceTypes",
        "Page",
        "Limit",
    ]
    assert sources["tts"]["required_headers"] == [
        "X-Api-Key",
        "X-Api-Resource-Id",
        "X-Api-Request-Id",
    ]
    assert sources["tts"]["language_field"] == "explicit_language"
    assert sources["tts"]["undefined_fields"] == ["language", "emotion"]
    assert sources["tts"]["timestamp_path"] == "sentence.words"
    assert sources["asr"]["resource_id"] == "volc.seedasr.auc"
    assert sources["asr"]["required_headers"] == [
        "X-Api-Key",
        "X-Api-Resource-Id",
        "X-Api-Request-Id",
    ]


def test_contract_fixtures_have_real_timestamps_and_no_credentials() -> None:
    speakers = load_json("list_speakers_page_1.json")
    asr = load_json("asr_result.json")
    tts_line = (FIXTURES / "tts_stream.ndjson").read_text(encoding="utf-8").strip()
    tts = json.loads(tts_line)

    assert speakers["Result"]["Speakers"][0]["ResourceID"] == "seed-tts-2.0"
    assert tts["sentence"]["words"][0]["endTime"] > 0
    assert asr["result"]["utterances"][0]["words"][0]["end_time"] > 0

    serialized = json.dumps(
        {"speakers": speakers, "tts": tts, "asr": asr},
        ensure_ascii=True,
    ).lower()
    for forbidden in ("x-api-key", "authorization", "secret-key", "access-key"):
        assert forbidden not in serialized
