FROM mwader/static-ffmpeg:7.1 AS ffmpeg

FROM python:3.12-slim

WORKDIR /app

COPY pyproject.toml ./
COPY src ./src
COPY --from=ffmpeg /ffmpeg /usr/local/bin/ffmpeg
COPY --from=ffmpeg /ffprobe /usr/local/bin/ffprobe
RUN pip install --no-cache-dir -e ".[dev]"

EXPOSE 8081

CMD ["uvicorn", "video_worker.main:app", "--host", "0.0.0.0", "--port", "8081"]
