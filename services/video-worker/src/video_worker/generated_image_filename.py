from __future__ import annotations

import unicodedata


DEFAULT_SCRIPT_TITLE = "未命名脚本"
MAX_FILENAME_BYTES = 255
SUPPORTED_EXTENSIONS = frozenset({".png", ".jpg", ".webp"})
WINDOWS_INVALID_FILENAME_CHARS = frozenset('<>:"/\\|?*')


def generated_image_filename(
    script_title: str,
    scene_sequence: int,
    candidate_index: int,
    extension: str,
) -> str:
    if scene_sequence < 1 or candidate_index < 1:
        raise ValueError("scene sequence and candidate index must be positive")
    if extension not in SUPPORTED_EXTENSIONS:
        raise ValueError(f"unsupported generated image extension: {extension}")

    suffix = f"-镜头{scene_sequence:02d}-第{candidate_index:02d}张{extension}"
    title = _clean_script_title(script_title)
    title_budget = MAX_FILENAME_BYTES - len(suffix.encode("utf-8"))
    if title_budget <= 0:
        raise ValueError("generated image suffix exceeds filename byte limit")
    title = _truncate_utf8(title, title_budget).rstrip(". ") or DEFAULT_SCRIPT_TITLE
    return f"{title}{suffix}"


def _clean_script_title(value: str) -> str:
    normalized = unicodedata.normalize("NFC", value)
    cleaned = "".join(
        character
        for character in normalized
        if character not in WINDOWS_INVALID_FILENAME_CHARS
        and unicodedata.category(character) != "Cc"
    )
    return cleaned.strip().rstrip(". ") or DEFAULT_SCRIPT_TITLE


def _truncate_utf8(value: str, max_bytes: int) -> str:
    encoded = value.encode("utf-8")
    if len(encoded) <= max_bytes:
        return value
    return encoded[:max_bytes].decode("utf-8", errors="ignore")
