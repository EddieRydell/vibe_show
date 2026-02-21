"""Low-level audio feature extraction using librosa."""

import logging

import numpy as np

logger = logging.getLogger(__name__)


def analyze_features(audio_path: str, **kwargs) -> dict:
    """Extract RMS, spectral centroid, onset strength, and chromagram.

    Returns a dict matching the LowLevelFeatures Rust struct.
    """
    import librosa

    logger.info("Running low-level feature extraction on %s", audio_path)

    y, sr = librosa.load(audio_path, sr=22050)
    hop_length = 512

    # RMS energy
    rms = librosa.feature.rms(y=y, hop_length=hop_length)[0]

    # Spectral centroid
    centroid = librosa.feature.spectral_centroid(y=y, sr=sr, hop_length=hop_length)[0]

    # Onset strength
    onset_env = librosa.onset.onset_strength(y=y, sr=sr, hop_length=hop_length)

    # Chromagram (12 x T)
    chroma = librosa.feature.chroma_cqt(y=y, sr=sr, hop_length=hop_length)

    time_step = hop_length / sr

    return {
        "rms": [round(float(x), 6) for x in rms],
        "spectral_centroid": [round(float(x), 2) for x in centroid],
        "onset_strength": [round(float(x), 6) for x in onset_env],
        "time_step": round(time_step, 8),
        "chromagram": [round(float(x), 4) for x in chroma.flatten("C")],
        "chromagram_length": int(chroma.shape[1]),
    }
