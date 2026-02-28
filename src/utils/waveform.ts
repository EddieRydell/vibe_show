/** Shared waveform downsampling used by both the editor store and analysis screen. */

export const WAVEFORM_PEAKS = 2000;

export function downsampleToPeaks(channelData: Float32Array, targetPeaks: number): Float32Array {
  const peaks = new Float32Array(targetPeaks);
  const samplesPerPeak = Math.floor(channelData.length / targetPeaks);
  if (samplesPerPeak === 0) {
    for (let i = 0; i < Math.min(channelData.length, targetPeaks); i++) {
      peaks[i] = Math.abs(channelData[i]!);
    }
    return peaks;
  }
  for (let i = 0; i < targetPeaks; i++) {
    let max = 0;
    const start = i * samplesPerPeak;
    const end = Math.min(start + samplesPerPeak, channelData.length);
    for (let j = start; j < end; j++) {
      const abs = Math.abs(channelData[j]!);
      if (abs > max) max = abs;
    }
    peaks[i] = max;
  }
  return peaks;
}
