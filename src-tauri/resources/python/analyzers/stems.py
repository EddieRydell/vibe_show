"""Source separation using Demucs."""

import logging
from pathlib import Path

logger = logging.getLogger(__name__)


def analyze_stems(
    audio_path: str, output_dir: str, use_gpu: bool = False, **kwargs
) -> dict:
    """Separate audio into vocal, drums, bass, and other stems.

    Returns a dict matching the StemAnalysis Rust struct (relative paths).
    """
    import subprocess
    import sys

    logger.info("Running source separation on %s", audio_path)

    out = Path(output_dir)
    out.mkdir(parents=True, exist_ok=True)

    # Run demucs as a subprocess for isolation
    cmd = [
        sys.executable,
        "-m",
        "demucs",
        "--two-stems=vocals" if False else "-n",
        "htdemucs",
        "-o",
        str(out),
        str(audio_path),
    ]
    if not use_gpu:
        cmd.insert(3, "--device")
        cmd.insert(4, "cpu")

    # Build the command properly
    cmd = [
        sys.executable,
        "-m",
        "demucs",
        "-n",
        "htdemucs",
        "-o",
        str(out),
    ]
    if not use_gpu:
        cmd.extend(["--device", "cpu"])
    cmd.append(str(audio_path))

    logger.info("Running: %s", " ".join(cmd))

    result = subprocess.run(cmd, capture_output=True, text=True, timeout=3600)

    if result.returncode != 0:
        raise RuntimeError(f"Demucs failed: {result.stderr}")

    # Demucs outputs to {output_dir}/htdemucs/{stem_name}/{source}.wav
    audio_name = Path(audio_path).stem
    stems_base = out / "htdemucs" / audio_name

    stem_files = {}
    for stem in ["vocals", "drums", "bass", "other"]:
        stem_path = stems_base / f"{stem}.wav"
        if stem_path.exists():
            stem_files[stem] = str(stem_path)
        else:
            logger.warning("Expected stem not found: %s", stem_path)
            stem_files[stem] = ""

    return {
        "vocals": stem_files.get("vocals", ""),
        "drums": stem_files.get("drums", ""),
        "bass": stem_files.get("bass", ""),
        "other": stem_files.get("other", ""),
    }
