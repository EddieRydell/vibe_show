"""Polyphonic pitch detection using Basic Pitch."""

import logging

logger = logging.getLogger(__name__)


def analyze_pitch(audio_path: str, **kwargs) -> dict:
    """Detect polyphonic notes (MIDI events) in the audio.

    Returns a dict matching the PitchAnalysis Rust struct.
    """
    from basic_pitch.inference import predict

    logger.info("Running pitch detection on %s", audio_path)

    _, midi_data, _ = predict(audio_path)

    notes = []
    for instrument in midi_data.instruments:
        for note in instrument.notes:
            notes.append(
                {
                    "midi_note": note.pitch,
                    "start": round(float(note.start), 4),
                    "end": round(float(note.end), 4),
                    "velocity": round(note.velocity / 127.0, 3),
                }
            )

    # Sort by start time
    notes.sort(key=lambda n: n["start"])

    return {"notes": notes}
