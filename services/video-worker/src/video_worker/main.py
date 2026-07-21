import asyncio
import os
from pathlib import Path
from typing import Callable

from fastapi import FastAPI

from video_worker.asset_generation import (
    LocalAssetStorage,
    PostgresAssetGenerationStore,
    image_provider_from_model,
    run_next_image_task,
)
from video_worker.model_registry import PostgresModelRegistry
from video_worker.speech_generation import (
    LocalSpeechStorage,
    PostgresSpeechStore,
    run_next_audio_inspection,
    run_next_speech_task,
    run_next_tos_cleanup,
)
from video_worker.tos_tool_check import (
    PostgresTosCheckStore,
    run_next_tos_connection_check,
)
from video_worker.voice_catalog import PostgresVoiceCatalogStore, run_next_voice_catalog_sync
from video_worker.work_generation import (
    default_process_next_work_generation as process_work_generation,
    validate_work_generation_mode,
)


def default_process_next_image_task() -> bool:
    database_url = os.getenv(
        "DATABASE_URL",
        "postgres://postgres:postgres@biga-postgres:5432/video_agent",
    )
    store = PostgresAssetGenerationStore(database_url)
    model_registry = PostgresModelRegistry(database_url)
    storage = LocalAssetStorage(
        Path(os.getenv("ASSET_STORAGE_ROOT", "/app/storage/assets")),
        public_prefix=os.getenv("ASSET_PUBLIC_PREFIX", "/assets"),
    )
    return run_next_image_task(
        store,
        model_registry,
        image_provider_from_model,
        storage,
    )


def default_process_next_voice_catalog_sync() -> bool:
    database_url = os.getenv(
        "DATABASE_URL",
        "postgres://postgres:postgres@biga-postgres:5432/video_agent",
    )
    return run_next_voice_catalog_sync(PostgresVoiceCatalogStore(database_url))


def default_process_next_speech_work() -> bool:
    database_url = os.getenv(
        "DATABASE_URL",
        "postgres://postgres:postgres@biga-postgres:5432/video_agent",
    )
    store = PostgresSpeechStore(database_url)
    registry = PostgresModelRegistry(database_url)
    storage = LocalSpeechStorage(
        Path(os.getenv("ASSET_STORAGE_ROOT", "/app/storage/assets")),
        public_prefix=os.getenv("ASSET_PUBLIC_PREFIX", "/assets"),
    )
    store.recover_stale_work(
        int(os.getenv("SPEECH_WORKER_LEASE_SECONDS", "600"))
    )
    processed = run_next_tos_cleanup(store, registry)
    processed = run_next_audio_inspection(store, storage) or processed
    processed = run_next_speech_task(store, registry, storage) or processed
    return processed


def default_process_next_tos_tool_work() -> bool:
    database_url = os.getenv(
        "DATABASE_URL",
        "postgres://postgres:postgres@biga-postgres:5432/video_agent",
    )
    store = PostgresTosCheckStore(database_url)
    store.recover_stale_checks(
        int(os.getenv("TOS_TOOL_WORKER_LEASE_SECONDS", "600"))
    )
    return run_next_tos_connection_check(store)


def default_process_next_work_generation() -> bool:
    return process_work_generation()


def create_app(
    process_next_image_task: Callable[[], bool] | None = None,
    process_next_voice_catalog: Callable[[], bool] | None = None,
    process_next_speech_work: Callable[[], bool] | None = None,
    process_next_tos_tool_work: Callable[[], bool] | None = None,
    process_next_work_generation: Callable[[], bool] | None = None,
    enable_background_worker: bool | None = None,
    enable_voice_catalog_worker: bool | None = None,
    enable_speech_worker: bool | None = None,
    enable_tos_tool_worker: bool | None = None,
    enable_work_generation_worker: bool | None = None,
) -> FastAPI:
    app = FastAPI(title="novex-video-worker")
    processor = process_next_image_task or default_process_next_image_task
    voice_catalog_processor = (
        process_next_voice_catalog or default_process_next_voice_catalog_sync
    )
    speech_processor = process_next_speech_work or default_process_next_speech_work
    tos_tool_processor = process_next_tos_tool_work or default_process_next_tos_tool_work
    work_generation_processor = process_next_work_generation or default_process_next_work_generation
    background_enabled = (
        enable_background_worker
        if enable_background_worker is not None
        else os.getenv("ASSET_GENERATION_WORKER_ENABLED", "false").lower() == "true"
    )
    voice_catalog_background_enabled = (
        enable_voice_catalog_worker
        if enable_voice_catalog_worker is not None
        else os.getenv("VOICE_CATALOG_WORKER_ENABLED", "false").lower() == "true"
    )
    speech_background_enabled = (
        enable_speech_worker
        if enable_speech_worker is not None
        else os.getenv("SPEECH_GENERATION_WORKER_ENABLED", "false").lower() == "true"
    )
    tos_tool_background_enabled = (
        enable_tos_tool_worker
        if enable_tos_tool_worker is not None
        else os.getenv("TOS_TOOL_WORKER_ENABLED", "false").lower() == "true"
    )
    work_generation_background_enabled = (
        enable_work_generation_worker
        if enable_work_generation_worker is not None
        else os.getenv("WORK_GENERATION_WORKER_ENABLED", "false").lower() == "true"
    )
    if process_next_work_generation is None:
        validate_work_generation_mode(
            fake_enabled=os.getenv("WORK_GENERATION_FAKE_PROVIDER_ENABLED", "false").lower() == "true",
            real_enabled=os.getenv("WORK_GENERATION_REAL_PROVIDER_ENABLED", "false").lower() == "true",
            worker_enabled=work_generation_background_enabled,
        )

    @app.get("/health")
    def health() -> dict[str, str]:
        return {
            "service": "novex-video-worker",
            "status": "ok",
            "asset_generation_worker": "enabled" if background_enabled else "disabled",
            "voice_catalog_worker": (
                "enabled" if voice_catalog_background_enabled else "disabled"
            ),
            "speech_generation_worker": (
                "enabled" if speech_background_enabled else "disabled"
            ),
            "tos_tool_worker": "enabled" if tos_tool_background_enabled else "disabled",
            "work_generation_worker": (
                "enabled" if work_generation_background_enabled else "disabled"
            ),
        }

    @app.post("/asset-generation/process-next")
    def process_next() -> dict[str, bool]:
        return {"processed": bool(processor())}

    @app.post("/speech/voice-catalog/process-next")
    def process_next_voice_catalog_sync() -> dict[str, bool]:
        return {"processed": bool(voice_catalog_processor())}

    @app.post("/speech/process-next")
    def process_next_speech() -> dict[str, bool]:
        return {"processed": bool(speech_processor())}

    @app.post("/tools/tos-staging/process-next")
    def process_next_tos_tool() -> dict[str, bool]:
        return {"processed": bool(tos_tool_processor())}

    @app.post("/work-generation/process-next")
    def process_next_work_generation_task() -> dict[str, bool]:
        return {"processed": bool(work_generation_processor())}

    if background_enabled:
        interval_seconds = float(os.getenv("ASSET_GENERATION_POLL_SECONDS", "5"))

        @app.on_event("startup")
        async def start_asset_generation_worker() -> None:
            async def loop() -> None:
                while True:
                    try:
                        await asyncio.to_thread(processor)
                    except Exception as error:  # pragma: no cover - logged in runtime.
                        print(f"asset generation worker error: {error}")
                    await asyncio.sleep(interval_seconds)

            app.state.asset_generation_worker_task = asyncio.create_task(loop())

        @app.on_event("shutdown")
        async def stop_asset_generation_worker() -> None:
            task = getattr(app.state, "asset_generation_worker_task", None)
            if task is not None:
                task.cancel()

    if voice_catalog_background_enabled:
        voice_catalog_interval_seconds = float(
            os.getenv("VOICE_CATALOG_POLL_SECONDS", "5")
        )

        @app.on_event("startup")
        async def start_voice_catalog_worker() -> None:
            async def loop() -> None:
                while True:
                    try:
                        await asyncio.to_thread(voice_catalog_processor)
                    except Exception as error:  # pragma: no cover - logged in runtime.
                        print(
                            "voice catalog worker error: "
                            f"{error.__class__.__name__}"
                        )
                    await asyncio.sleep(voice_catalog_interval_seconds)

            app.state.voice_catalog_worker_task = asyncio.create_task(loop())

        @app.on_event("shutdown")
        async def stop_voice_catalog_worker() -> None:
            task = getattr(app.state, "voice_catalog_worker_task", None)
            if task is not None:
                task.cancel()

    if speech_background_enabled:
        speech_interval_seconds = float(os.getenv("SPEECH_GENERATION_POLL_SECONDS", "5"))

        @app.on_event("startup")
        async def start_speech_generation_worker() -> None:
            async def loop() -> None:
                while True:
                    try:
                        await asyncio.to_thread(speech_processor)
                    except Exception as error:  # pragma: no cover - logged in runtime.
                        print(f"speech generation worker error: {error}")
                    await asyncio.sleep(speech_interval_seconds)

            app.state.speech_generation_worker_task = asyncio.create_task(loop())

        @app.on_event("shutdown")
        async def stop_speech_generation_worker() -> None:
            task = getattr(app.state, "speech_generation_worker_task", None)
            if task is not None:
                task.cancel()

    if tos_tool_background_enabled:
        tos_tool_interval_seconds = float(os.getenv("TOS_TOOL_WORKER_POLL_SECONDS", "5"))

        @app.on_event("startup")
        async def start_tos_tool_worker() -> None:
            async def loop() -> None:
                while True:
                    try:
                        await asyncio.to_thread(tos_tool_processor)
                    except Exception as error:  # pragma: no cover - logged in runtime.
                        print(f"TOS tool worker error: {error}")
                    await asyncio.sleep(tos_tool_interval_seconds)

            app.state.tos_tool_worker_task = asyncio.create_task(loop())

        @app.on_event("shutdown")
        async def stop_tos_tool_worker() -> None:
            task = getattr(app.state, "tos_tool_worker_task", None)
            if task is not None:
                task.cancel()

    if work_generation_background_enabled:
        work_generation_interval_seconds = float(
            os.getenv("WORK_GENERATION_POLL_SECONDS", "5")
        )

        @app.on_event("startup")
        async def start_work_generation_worker() -> None:
            async def loop() -> None:
                while True:
                    try:
                        await asyncio.to_thread(work_generation_processor)
                    except Exception as error:  # pragma: no cover - logged in runtime.
                        print(f"work generation worker error: {error}")
                    await asyncio.sleep(work_generation_interval_seconds)

            app.state.work_generation_worker_task = asyncio.create_task(loop())

        @app.on_event("shutdown")
        async def stop_work_generation_worker() -> None:
            task = getattr(app.state, "work_generation_worker_task", None)
            if task is not None:
                task.cancel()

    return app


app = create_app()
