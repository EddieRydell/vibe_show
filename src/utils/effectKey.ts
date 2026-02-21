/**
 * Parse a visual effect key into its logical track/effect indices.
 *
 * Keys have the format "fixtureId:trackIdx-effectIdx" or legacy "trackIdx-effectIdx".
 * Returns null if the key is malformed.
 */
export function parseEffectKey(
  key: string,
): { trackIndex: number; effectIndex: number } | null {
  const logical = key.includes(":") ? key.split(":")[1] : key;
  const parts = logical.split("-");
  if (parts.length !== 2) return null;
  const trackIndex = parseInt(parts[0], 10);
  const effectIndex = parseInt(parts[1], 10);
  if (isNaN(trackIndex) || isNaN(effectIndex)) return null;
  return { trackIndex, effectIndex };
}

/**
 * Deduplicate a set of visual effect keys into unique [trackIndex, effectIndex] tuples.
 *
 * Multiple visual keys (fixture-specific) may map to the same logical effect.
 * This extracts the unique logical pairs.
 */
export function deduplicateEffectKeys(
  keys: Iterable<string>,
): [number, number][] {
  const seen = new Set<string>();
  const result: [number, number][] = [];
  for (const key of keys) {
    const parsed = parseEffectKey(key);
    if (!parsed) continue;
    const logical = `${parsed.trackIndex}-${parsed.effectIndex}`;
    if (!seen.has(logical)) {
      seen.add(logical);
      result.push([parsed.trackIndex, parsed.effectIndex]);
    }
  }
  return result;
}
