import unicodedata

import pytest

from video_worker.generated_image_filename import generated_image_filename


def test_generated_image_filename_keeps_chinese_and_formats_positions():
    assert generated_image_filename(
        "别硬扛，用Debug解决烦心事",
        1,
        1,
        ".jpg",
    ) == "别硬扛，用Debug解决烦心事-镜头01-第01张.jpg"


def test_generated_image_filename_normalizes_and_removes_invalid_characters():
    result = generated_image_filename(
        '  Cafe\u0301/A\\B<C>D:E"F|G?H*I\x00.  ',
        2,
        3,
        ".png",
    )

    assert result == "CaféABCDEFGHI-镜头02-第03张.png"
    assert unicodedata.is_normalized("NFC", result)


@pytest.mark.parametrize("title", ["", "   ", "...", "<>:\"/\\|?*\x00"])
def test_generated_image_filename_falls_back_for_empty_cleaned_title(title: str):
    assert generated_image_filename(title, 2, 3, ".png") == (
        "未命名脚本-镜头02-第03张.png"
    )


def test_generated_image_filename_truncates_at_utf8_codepoint_boundary():
    result = generated_image_filename("测" * 200, 20, 4, ".webp")

    assert len(result.encode("utf-8")) <= 255
    assert result.endswith("-镜头20-第04张.webp")
    assert result.encode("utf-8").decode("utf-8") == result


@pytest.mark.parametrize("extension", [".gif", "png", "../x.png"])
def test_generated_image_filename_rejects_unsupported_extension(extension: str):
    with pytest.raises(ValueError, match="unsupported generated image extension"):
        generated_image_filename("脚本", 1, 1, extension)
