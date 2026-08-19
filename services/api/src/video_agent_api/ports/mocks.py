"""确定性 Mock Provider：不导入 SDK，不发起网络请求。"""

from __future__ import annotations

from hashlib import sha256

from video_agent_api.logging import log_event
from video_agent_api.ports.contracts import ModelSelection, PortResult


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
        return self._result("text.generate", prompt, selection, correlation_id)

    def generate_image(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        return self._result("image.generate", prompt, selection, correlation_id)

    def edit_image(self, prompt: str, selection: ModelSelection, correlation_id: str) -> PortResult:
        return self._result("image.edit", prompt, selection, correlation_id)

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
