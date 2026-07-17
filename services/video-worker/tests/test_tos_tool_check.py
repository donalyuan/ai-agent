from video_worker.model_registry import SpeechStagingRuntimeConfig
from video_worker.tos_tool_check import run_next_tos_connection_check


def locked_config() -> SpeechStagingRuntimeConfig:
    return SpeechStagingRuntimeConfig(
        config_id="66666666-6666-4666-8666-666666666666",
        version=5,
        storage_provider="volcengine_tos",
        endpoint="https://tos-cn-beijing.volces.com",
        region="cn-beijing",
        bucket="private-bucket",
        object_prefix="novex/asr",
        access_key="tos-ak",
        secret_key="tos-sk",
        signed_url_ttl_seconds=600,
        max_file_bytes=1048576,
        max_audio_duration_seconds=3600,
    )


class MemoryStore:
    def __init__(self, config):
        self.config = config
        self.completed = None

    def claim_next_check(self):
        config, self.config = self.config, None
        return config

    def complete_check(self, config_id, version, *, succeeded, error_summary):
        self.completed = (config_id, version, succeeded, error_summary)


def test_connection_check_records_real_checker_success() -> None:
    config = locked_config()
    store = MemoryStore(config)
    calls = []

    class Checker:
        def check(self):
            calls.append("checked")

    assert run_next_tos_connection_check(
        store, checker_factory=lambda received: Checker()
    )
    assert calls == ["checked"]
    assert store.completed == (config.config_id, config.version, True, None)


def test_connection_check_failure_redacts_exception_message() -> None:
    config = locked_config()
    store = MemoryStore(config)

    class SecretFailure(RuntimeError):
        pass

    class Checker:
        def check(self):
            raise SecretFailure("secret-key-should-not-be-stored")

    assert run_next_tos_connection_check(
        store, checker_factory=lambda received: Checker()
    )
    assert store.completed[2] is False
    assert store.completed[3] == "TOS Bucket 能力检查失败: SecretFailure"
    assert "secret-key" not in store.completed[3]
