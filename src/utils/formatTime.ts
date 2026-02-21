/** Transport display: M:SS.CC (minutes:seconds.centiseconds) */
export function formatTimeTransport(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  const ms = Math.floor((seconds % 1) * 100);
  return `${m}:${s.toString().padStart(2, "0")}.${ms.toString().padStart(2, "0")}`;
}

/** Duration display: M:S.Ds with "s" suffix for property panel */
export function formatTimeDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = (seconds % 60).toFixed(1);
  return m > 0 ? `${m}:${s.padStart(4, "0")}` : `${s}s`;
}
