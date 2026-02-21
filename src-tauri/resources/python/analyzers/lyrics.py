"""Lyrics transcription using faster-whisper on the vocal stem."""

import logging

logger = logging.getLogger(__name__)


def analyze_lyrics(vocal_stem_path: str, use_gpu: bool = False, **kwargs) -> dict:
    """Transcribe lyrics with word-level timestamps.

    Returns a dict matching the LyricsAnalysis Rust struct.
    """
    from faster_whisper import WhisperModel

    logger.info("Running lyrics transcription on %s", vocal_stem_path)

    device = "cuda" if use_gpu else "cpu"
    compute_type = "float16" if use_gpu else "int8"

    model = WhisperModel("turbo", device=device, compute_type=compute_type)

    segments, info = model.transcribe(
        vocal_stem_path, word_timestamps=True, language=None
    )

    words = []
    full_text_parts = []

    for segment in segments:
        full_text_parts.append(segment.text)
        if segment.words:
            for word in segment.words:
                words.append(
                    {
                        "word": word.word.strip(),
                        "start": float(word.start),
                        "end": float(word.end),
                        "confidence": float(word.probability),
                    }
                )

    return {
        "words": words,
        "full_text": " ".join(full_text_parts).strip(),
        "language": info.language or "unknown",
    }
