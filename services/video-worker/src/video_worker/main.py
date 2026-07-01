from fastapi import FastAPI

app = FastAPI(title="novex-video-worker")


@app.get("/health")
def health() -> dict[str, str]:
    return {
        "service": "novex-video-worker",
        "status": "ok",
    }
