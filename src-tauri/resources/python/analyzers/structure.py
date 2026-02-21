"""Song structure analysis using allin1."""

import logging

logger = logging.getLogger(__name__)


def analyze_structure(audio_path: str, models_dir: str = None, **kwargs) -> dict:
    """Detect song sections (verse, chorus, bridge, etc.).

    Returns a dict matching the StructureAnalysis Rust struct.
    """
    import allin1

    logger.info("Running structure analysis on %s", audio_path)

    result = allin1.analyze(audio_path)

    sections = []
    for segment in result.segments:
        sections.append(
            {
                "label": segment.label,
                "start": float(segment.start),
                "end": float(segment.end),
                "confidence": 0.8,  # allin1 doesn't expose per-segment confidence
            }
        )

    return {"sections": sections}
