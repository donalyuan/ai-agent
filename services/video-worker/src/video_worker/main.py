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


def create_app(
    process_next_image_task: Callable[[], bool] | None = None,
    enable_background_worker: bool | None = None,
) -> FastAPI:
    app = FastAPI(title="novex-video-worker")
    processor = process_next_image_task or default_process_next_image_task
    background_enabled = (
        enable_background_worker
        if enable_background_worker is not None
        else os.getenv("ASSET_GENERATION_WORKER_ENABLED", "false").lower() == "true"
    )

    @app.get("/health")
    def health() -> dict[str, str]:
        return {
            "service": "novex-video-worker",
            "status": "ok",
            "asset_generation_worker": "enabled" if background_enabled else "disabled",
        }

    @app.post("/asset-generation/process-next")
    def process_next() -> dict[str, bool]:
        return {"processed": bool(processor())}

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

    return app


app = create_app()
