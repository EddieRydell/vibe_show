"""
Pipeline orchestrator — runs analysis features in dependency order.

Dependency graph:
  1. Stems (Demucs) — first, since lyrics/drums/vocals depend on it
  2. Beats (madmom) — independent
  3. Structure (allin1) — independent
  4. Lyrics (faster-whisper on vocal stem) — depends on stems
  5. Mood (Essentia) — independent
  6. Key/Chords (Essentia/madmom) — independent
  7. Low-level (librosa) — independent
  8. Pitch (Basic Pitch) — independent
  9. Drum onsets (librosa on drum stem) — depends on stems
  10. Vocal presence (energy on vocal stem) — depends on stems

Each step yields SSE events for progress streaming.
"""

import json
import logging
import traceback
from pathlib import Path

logger = logging.getLogger(__name__)


async def run_pipeline(
    audio_path: Path,
    output_dir: Path,
    features: dict,
    models_dir: Path,
    use_gpu: bool = False,
):
    """
    Generator that yields SSE event dicts.
    Progress events have {"phase": str, "progress": float, "detail": str|None}.
    The final event is the complete AudioAnalysis JSON.
    """
    import asyncio

    result = {
        "features": features,
        "beats": None,
        "structure": None,
        "stems": None,
        "lyrics": None,
        "mood": None,
        "harmony": None,
        "low_level": None,
        "pitch": None,
        "drums": None,
        "vocal_presence": None,
    }

    audio_str = str(audio_path)
    output_str = str(output_dir)

    total_features = sum(1 for v in features.values() if v)
    completed = 0

    def progress_event(phase: str, detail: str = None):
        nonlocal completed
        pct = completed / max(total_features, 1)
        return {
            "event": "progress",
            "data": json.dumps(
                {"phase": phase, "progress": round(pct, 3), "detail": detail}
            ),
        }

    # ── Phase 1: Stems (must run first if needed) ──────────────────

    stems_data = None
    if features.get("stems", False):
        yield progress_event("Source separation (Demucs)...", "Separating audio into stems")
        try:
            from analyzers.stems import analyze_stems

            stems_data = await asyncio.to_thread(
                analyze_stems,
                audio_path=audio_str,
                output_dir=output_str,
                use_gpu=use_gpu,
            )
            result["stems"] = stems_data
        except Exception as e:
            logger.error("Stems analysis failed: %s\n%s", e, traceback.format_exc())
        completed += 1

    # ── Phase 2: Independent features (can run in parallel in future) ──

    if features.get("beats", False):
        yield progress_event("Beat detection (madmom)...", "Detecting beats and tempo")
        try:
            from analyzers.beats import analyze_beats

            result["beats"] = await asyncio.to_thread(
                analyze_beats, audio_path=audio_str
            )
        except Exception as e:
            logger.error("Beat analysis failed: %s\n%s", e, traceback.format_exc())
        completed += 1

    if features.get("structure", False):
        yield progress_event("Structure analysis (allin1)...", "Detecting song sections")
        try:
            from analyzers.structure import analyze_structure

            result["structure"] = await asyncio.to_thread(
                analyze_structure,
                audio_path=audio_str,
                models_dir=str(models_dir),
            )
        except Exception as e:
            logger.error("Structure analysis failed: %s\n%s", e, traceback.format_exc())
        completed += 1

    if features.get("mood", False):
        yield progress_event("Mood analysis...", "Classifying mood and energy")
        try:
            from analyzers.mood import analyze_mood

            result["mood"] = await asyncio.to_thread(
                analyze_mood, audio_path=audio_str
            )
        except Exception as e:
            logger.error("Mood analysis failed: %s\n%s", e, traceback.format_exc())
        completed += 1

    if features.get("harmony", False):
        yield progress_event("Harmony analysis...", "Detecting key and chords")
        try:
            from analyzers.harmony import analyze_harmony

            result["harmony"] = await asyncio.to_thread(
                analyze_harmony, audio_path=audio_str
            )
        except Exception as e:
            logger.error("Harmony analysis failed: %s\n%s", e, traceback.format_exc())
        completed += 1

    if features.get("low_level", False):
        yield progress_event("Feature extraction...", "Extracting audio features")
        try:
            from analyzers.features import analyze_features

            result["low_level"] = await asyncio.to_thread(
                analyze_features, audio_path=audio_str
            )
        except Exception as e:
            logger.error("Feature extraction failed: %s\n%s", e, traceback.format_exc())
        completed += 1

    if features.get("pitch", False):
        yield progress_event("Pitch detection (Basic Pitch)...", "Detecting notes")
        try:
            from analyzers.pitch import analyze_pitch

            result["pitch"] = await asyncio.to_thread(
                analyze_pitch, audio_path=audio_str
            )
        except Exception as e:
            logger.error("Pitch analysis failed: %s\n%s", e, traceback.format_exc())
        completed += 1

    # ── Phase 3: Stem-dependent features ───────────────────────────

    if features.get("lyrics", False) and stems_data and stems_data.get("vocals"):
        yield progress_event("Lyrics transcription (Whisper)...", "Transcribing vocals")
        try:
            from analyzers.lyrics import analyze_lyrics

            result["lyrics"] = await asyncio.to_thread(
                analyze_lyrics,
                vocal_stem_path=stems_data["vocals"],
                use_gpu=use_gpu,
            )
        except Exception as e:
            logger.error("Lyrics analysis failed: %s\n%s", e, traceback.format_exc())
        completed += 1
    elif features.get("lyrics", False):
        # Run on original audio if no stems
        yield progress_event("Lyrics transcription (Whisper)...", "Transcribing audio")
        try:
            from analyzers.lyrics import analyze_lyrics

            result["lyrics"] = await asyncio.to_thread(
                analyze_lyrics,
                vocal_stem_path=audio_str,
                use_gpu=use_gpu,
            )
        except Exception as e:
            logger.error("Lyrics analysis failed: %s\n%s", e, traceback.format_exc())
        completed += 1

    if features.get("drums", False) and stems_data and stems_data.get("drums"):
        yield progress_event("Drum onset detection...", "Detecting drum hits")
        try:
            from analyzers.drums import analyze_drums

            result["drums"] = await asyncio.to_thread(
                analyze_drums, drum_stem_path=stems_data["drums"]
            )
        except Exception as e:
            logger.error("Drum analysis failed: %s\n%s", e, traceback.format_exc())
        completed += 1
    elif features.get("drums", False):
        completed += 1

    if features.get("vocal_presence", False) and stems_data and stems_data.get("vocals"):
        yield progress_event("Vocal presence detection...", "Detecting vocal regions")
        try:
            from analyzers.vocals import analyze_vocals

            result["vocal_presence"] = await asyncio.to_thread(
                analyze_vocals, vocal_stem_path=stems_data["vocals"]
            )
        except Exception as e:
            logger.error(
                "Vocal presence failed: %s\n%s", e, traceback.format_exc()
            )
        completed += 1
    elif features.get("vocal_presence", False):
        completed += 1

    # ── Final result ───────────────────────────────────────────────

    yield {
        "event": "progress",
        "data": json.dumps(
            {"phase": "Complete", "progress": 1.0, "detail": "Analysis finished"}
        ),
    }

    yield {"event": "result", "data": json.dumps(result)}
