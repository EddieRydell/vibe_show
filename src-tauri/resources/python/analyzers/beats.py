"""Beat and tempo analysis using madmom."""

import logging

import numpy as np

logger = logging.getLogger(__name__)


def analyze_beats(audio_path: str, **kwargs) -> dict:
    """Detect beats, downbeats, tempo, and time signature.

    Returns a dict matching the BeatAnalysis Rust struct.
    """
    import madmom

    logger.info("Running beat analysis on %s", audio_path)

    # Beat detection using RNNBeatProcessor + DBNBeatTrackingProcessor
    proc = madmom.features.beats.RNNBeatProcessor()(audio_path)
    beat_proc = madmom.features.beats.DBNBeatTrackingProcessor(fps=100)
    beats = beat_proc(proc)

    # Downbeat detection
    try:
        down_proc = madmom.features.downbeats.RNNDownBeatProcessor()(audio_path)
        dbn_down = madmom.features.downbeats.DBNDownBeatTrackingProcessor(
            beats_per_bar=[3, 4], fps=100
        )
        downbeat_result = dbn_down(down_proc)
        # downbeat_result is (time, beat_position) — downbeats have position 1
        downbeats = [
            float(row[0]) for row in downbeat_result if int(row[1]) == 1
        ]
        # Infer time signature from most common beats-per-bar
        if len(downbeats) >= 2:
            bar_lengths = []
            for i in range(len(downbeats) - 1):
                count = sum(
                    1 for b in beats if downbeats[i] <= b < downbeats[i + 1]
                )
                bar_lengths.append(count)
            time_signature = int(np.median(bar_lengths)) if bar_lengths else 4
        else:
            time_signature = 4
    except Exception:
        logger.warning("Downbeat detection failed, using beat positions")
        downbeats = []
        time_signature = 4

    # Tempo estimation
    try:
        tempo_proc = madmom.features.tempo.TempoEstimationProcessor(fps=100)
        tempi = tempo_proc(proc)
        # tempi is array of (tempo, strength) pairs
        if len(tempi) > 0:
            tempo = float(tempi[0][0])
            tempo_confidence = float(tempi[0][1])
        else:
            tempo = 120.0
            tempo_confidence = 0.0
    except Exception:
        tempo = 120.0
        tempo_confidence = 0.0

    # Beat confidences from the activation function
    beat_confidences = []
    for beat_time in beats:
        # Find nearest activation value
        idx = int(beat_time * 100)
        if 0 <= idx < len(proc):
            beat_confidences.append(float(proc[idx]))
        else:
            beat_confidences.append(0.5)

    return {
        "beats": [float(b) for b in beats],
        "downbeats": downbeats,
        "tempo": tempo,
        "time_signature": time_signature,
        "beat_confidences": beat_confidences,
        "tempo_confidence": tempo_confidence,
    }
