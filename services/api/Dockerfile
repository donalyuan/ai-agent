FROM ghcr.io/astral-sh/uv:0.10.12 AS uv

FROM python:3.12.11-slim-bookworm
WORKDIR /workspace
# Media activities require an explicit, probeable renderer in the standard image.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ffmpeg \
    && rm -rf /var/lib/apt/lists/*
COPY --from=uv /uv /uvx /bin/
ENV PATH=/workspace/services/api/.venv/bin:$PATH
ENV PYTHONPATH=/workspace:/workspace/services/api/src

COPY services/api/pyproject.toml services/api/uv.lock /workspace/services/api/
RUN uv lock --project /workspace/services/api --check
RUN uv sync --project /workspace/services/api --frozen --no-dev --no-install-project

COPY services/api /workspace/services/api
COPY workers /workspace/workers
RUN uv sync --project /workspace/services/api --frozen --no-dev

EXPOSE 8000
CMD ["uvicorn", "video_agent_api.app:app", "--host", "0.0.0.0", "--port", "8000"]
