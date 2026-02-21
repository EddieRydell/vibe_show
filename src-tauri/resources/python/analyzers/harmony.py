"""Key detection and chord recognition."""

import logging

logger = logging.getLogger(__name__)


def analyze_harmony(audio_path: str, **kwargs) -> dict:
    """Detect musical key and chord progression.

    Returns a dict matching the HarmonyAnalysis Rust struct.
    """
    logger.info("Running harmony analysis on %s", audio_path)

    try:
        import essentia.standard as es

        audio = es.MonoLoader(filename=audio_path, sampleRate=44100)()

        # Key detection
        key_extractor = es.KeyExtractor()
        key, scale, key_strength = key_extractor(audio)
        key_str = f"{key} {scale}"
        key_confidence = float(key_strength)

    except (ImportError, Exception):
        logger.info("Using librosa fallback for key detection")
        import librosa
        import numpy as np

        y, sr = librosa.load(audio_path, sr=22050)
        chroma = librosa.feature.chroma_cqt(y=y, sr=sr)
        chroma_avg = np.mean(chroma, axis=1)

        key_names = [
            "C",
            "C#",
            "D",
            "D#",
            "E",
            "F",
            "F#",
            "G",
            "G#",
            "A",
            "A#",
            "B",
        ]
        key_idx = int(np.argmax(chroma_avg))
        key_str = f"{key_names[key_idx]} major"
        key_confidence = float(chroma_avg[key_idx] / np.sum(chroma_avg))

    # Chord detection using librosa chroma
    chords = _detect_chords(audio_path)

    return {
        "key": key_str,
        "key_confidence": round(key_confidence, 3),
        "chords": chords,
    }


def _detect_chords(audio_path: str) -> list:
    """Simple chord detection based on chroma features."""
    import librosa
    import numpy as np

    y, sr = librosa.load(audio_path, sr=22050)
    chroma = librosa.feature.chroma_cqt(y=y, sr=sr)

    hop_length = 512
    frame_duration = hop_length / sr

    # Simple chord templates (major and minor triads)
    chord_names = [
        "C",
        "C#",
        "D",
        "D#",
        "E",
        "F",
        "F#",
        "G",
        "G#",
        "A",
        "A#",
        "B",
    ]

    major_template = np.array([1, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0], dtype=float)
    minor_template = np.array([1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0], dtype=float)

    chords = []
    prev_chord = None
    chord_start = 0.0

    # Process in chunks for efficiency
    chunk_size = 8  # ~0.19 seconds per chord
    n_frames = chroma.shape[1]

    for i in range(0, n_frames, chunk_size):
        end_idx = min(i + chunk_size, n_frames)
        chunk = np.mean(chroma[:, i:end_idx], axis=1)
        chunk = chunk / (np.linalg.norm(chunk) + 1e-10)

        best_score = -1
        best_chord = "N"

        for root in range(12):
            rolled_major = np.roll(major_template, root)
            rolled_minor = np.roll(minor_template, root)

            score_maj = float(np.dot(chunk, rolled_major))
            score_min = float(np.dot(chunk, rolled_minor))

            if score_maj > best_score:
                best_score = score_maj
                best_chord = f"{chord_names[root]}maj"
            if score_min > best_score:
                best_score = score_min
                best_chord = f"{chord_names[root]}m"

        if best_score < 0.5:
            best_chord = "N"

        current_time = i * frame_duration

        if best_chord != prev_chord:
            if prev_chord is not None:
                chords.append(
                    {
                        "label": prev_chord,
                        "start": round(chord_start, 3),
                        "end": round(current_time, 3),
                    }
                )
            prev_chord = best_chord
            chord_start = current_time

    # Final chord
    if prev_chord is not None:
        chords.append(
            {
                "label": prev_chord,
                "start": round(chord_start, 3),
                "end": round(n_frames * frame_duration, 3),
            }
        )

    return chords
