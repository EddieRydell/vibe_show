"""
VibeLights Audio Analysis Sidecar — FastAPI application.

This sidecar is managed by the VibeLights Tauri app. It runs on localhost
and provides audio analysis endpoints. Communication uses JSON over HTTP
with SSE for progress streaming.

Usage:
    python sidecar.py --port 9123 --models-dir /path/to/models
"""

import argparse
import asyncio
import logging
import os
import signal
import sys
from pathlib import Path

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
from pydantic import BaseModel
from sse_starlette.sse import EventSourceResponse

logging.basicConfig(
    level=logging.INFO,
    format="[%(levelname)s] %(name)s: %(message)s",
    stream=sys.stderr,
)
logger = logging.getLogger("vibelights-sidecar")

app = FastAPI(title="VibeLights Analysis Sidecar")

# Global config set at startup
_models_dir: Path = Path(".")
_shutdown_event = asyncio.Event()


class AnalyzeRequest(BaseModel):
    audio_path: str
    output_dir: str
    features: dict
    models_dir: str
    gpu: bool = False


class HealthResponse(BaseModel):
    status: str = "ok"
    version: str = "0.1.0"


# ── Endpoints ──────────────────────────────────────────────────────


@app.get("/health")
async def health() -> HealthResponse:
    return HealthResponse()


@app.post("/analyze")
async def analyze(request: AnalyzeRequest):
    """Run audio analysis with SSE progress streaming."""

    async def event_generator():
        from analyzers.pipeline import run_pipeline

        try:
            async for event in run_pipeline(
                audio_path=Path(request.audio_path),
                output_dir=Path(request.output_dir),
                features=request.features,
                models_dir=Path(request.models_dir),
                use_gpu=request.gpu,
            ):
                yield event
        except Exception as e:
            logger.exception("Analysis failed")
            yield {
                "event": "error",
                "data": f'{{"error": "{str(e)}"}}',
            }

    return EventSourceResponse(event_generator())


@app.get("/models")
async def list_models():
    """List installed model directories."""
    models = []
    if _models_dir.exists():
        for entry in _models_dir.iterdir():
            if entry.is_dir():
                models.append(entry.name)
    return {"models": models}


@app.post("/shutdown")
async def shutdown():
    """Graceful shutdown."""
    logger.info("Shutdown requested")
    _shutdown_event.set()
    # Schedule actual shutdown after response is sent
    asyncio.get_event_loop().call_later(0.5, _force_exit)
    return {"status": "shutting_down"}


def _force_exit():
    os.kill(os.getpid(), signal.SIGTERM)


# ── Main ───────────────────────────────────────────────────────────


def main():
    parser = argparse.ArgumentParser(description="VibeLights Analysis Sidecar")
    parser.add_argument("--port", type=int, default=9100, help="Port to listen on")
    parser.add_argument(
        "--models-dir",
        type=str,
        default="./models",
        help="Directory for ML model weights",
    )
    args = parser.parse_args()

    global _models_dir
    _models_dir = Path(args.models_dir)
    _models_dir.mkdir(parents=True, exist_ok=True)

    logger.info(f"Starting sidecar on port {args.port}")
    logger.info(f"Models directory: {_models_dir}")

    import uvicorn

    uvicorn.run(
        app,
        host="127.0.0.1",
        port=args.port,
        log_level="warning",
    )


if __name__ == "__main__":
    main()
