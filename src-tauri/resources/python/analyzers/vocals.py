"""Vocal presence detection from the vocal stem."""

import logging

logger = logging.getLogger(__name__)


def analyze_vocals(vocal_stem_path: str, **kwargs) -> dict:
    """Detect regions where vocals are present based on stem energy.

    Returns a dict matching the VocalPresence Rust struct.
    """
    import librosa
    import numpy as np

    logger.info("Running vocal presence detection on %s", vocal_stem_path)

    y, sr = librosa.load(vocal_stem_path, sr=22050)
    hop_length = 512

    # Compute RMS energy
    rms = librosa.feature.rms(y=y, hop_length=hop_length)[0]
    times = librosa.frames_to_time(range(len(rms)), sr=sr, hop_length=hop_length)

    # Dynamic threshold: use median + a fraction of the range
    rms_median = float(np.median(rms))
    rms_max = float(np.max(rms))
    threshold = rms_median + 0.15 * (rms_max - rms_median)

    # Find segments where RMS exceeds threshold
    segments = []
    in_segment = False
    seg_start = 0.0

    for i, (t, r) in enumerate(zip(times, rms)):
        if r > threshold and not in_segment:
            in_segment = True
            seg_start = float(t)
        elif r <= threshold and in_segment:
            in_segment = False
            seg_end = float(t)
            # Only keep segments longer than 0.3 seconds
            if seg_end - seg_start > 0.3:
                segments.append(
                    {
                        "start": round(seg_start, 3),
                        "end": round(seg_end, 3),
                    }
                )

    # Close final segment
    if in_segment:
        seg_end = float(times[-1]) if len(times) > 0 else 0.0
        if seg_end - seg_start > 0.3:
            segments.append(
                {
                    "start": round(seg_start, 3),
                    "end": round(seg_end, 3),
                }
            )

    # Merge segments that are very close (< 0.5 seconds gap)
    merged = []
    for seg in segments:
        if merged and seg["start"] - merged[-1]["end"] < 0.5:
            merged[-1]["end"] = seg["end"]
        else:
            merged.append(seg)

    return {"segments": merged}
