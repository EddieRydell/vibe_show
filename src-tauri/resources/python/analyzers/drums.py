"""Drum onset detection from the drum stem."""

import logging

logger = logging.getLogger(__name__)


def analyze_drums(drum_stem_path: str, **kwargs) -> dict:
    """Detect onset times and strengths from the drum stem.

    Returns a dict matching the DrumAnalysis Rust struct.
    """
    import librosa
    import numpy as np

    logger.info("Running drum onset detection on %s", drum_stem_path)

    y, sr = librosa.load(drum_stem_path, sr=22050)

    # Onset detection
    onset_env = librosa.onset.onset_strength(y=y, sr=sr)
    onset_frames = librosa.onset.onset_detect(
        y=y, sr=sr, onset_envelope=onset_env, backtrack=False
    )
    onset_times = librosa.frames_to_time(onset_frames, sr=sr)

    # Get strengths at onset positions
    strengths = []
    max_env = float(np.max(onset_env)) if len(onset_env) > 0 else 1.0
    for frame in onset_frames:
        if frame < len(onset_env):
            strengths.append(round(float(onset_env[frame]) / max_env, 3))
        else:
            strengths.append(0.5)

    return {
        "onsets": [round(float(t), 4) for t in onset_times],
        "strengths": strengths,
    }
