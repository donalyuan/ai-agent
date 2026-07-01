from fastapi import FastAPI

app = FastAPI(title="video-agent-worker")


@app.get("/health")
def health() -> dict[str, str]:
    return {
        "service": "video-agent-worker",
        "status": "ok",
    }
