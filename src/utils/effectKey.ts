/**
 * Create a canonical effect selection key from track and effect indices.
 *
 * This is the single source of truth for effect key format: "trackIdx-effectIdx".
 */
export function makeEffectKey(trackIndex: number, effectIndex: number): string {
  return `${trackIndex}-${effectIndex}`;
}

/**
 * Parse an effect key into its track/effect indices.
 *
 * Keys must be in canonical "trackIdx-effectIdx" format.
 * Returns null if the key is malformed.
 */
export function parseEffectKey(
  key: string,
): { trackIndex: number; effectIndex: number } | null {
  if (!/^\d+-\d+$/.test(key)) return null;
  const parts = key.split("-");
  const trackIndex = parseInt(parts[0]!, 10);
  const effectIndex = parseInt(parts[1]!, 10);
  return { trackIndex, effectIndex };
}

/**
 * Deduplicate a set of effect keys into unique [trackIndex, effectIndex] tuples.
 */
export function deduplicateEffectKeys(
  keys: Iterable<string>,
): [number, number][] {
  const seen = new Set<string>();
  const result: [number, number][] = [];
  for (const key of keys) {
    const parsed = parseEffectKey(key);
    if (!parsed) continue;
    const canonical = makeEffectKey(parsed.trackIndex, parsed.effectIndex);
    if (!seen.has(canonical)) {
      seen.add(canonical);
      result.push([parsed.trackIndex, parsed.effectIndex]);
    }
  }
  return result;
}
