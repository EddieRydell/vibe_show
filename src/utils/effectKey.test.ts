import { describe, it, expect } from "vitest";
import { parseEffectKey, deduplicateEffectKeys } from "./effectKey";

describe("parseEffectKey", () => {
  it("parses fixture-prefixed key", () => {
    expect(parseEffectKey("5:2-3")).toEqual({ trackIndex: 2, effectIndex: 3 });
  });

  it("parses legacy key without fixture prefix", () => {
    expect(parseEffectKey("1-7")).toEqual({ trackIndex: 1, effectIndex: 7 });
  });

  it("returns null for malformed keys", () => {
    expect(parseEffectKey("")).toBeNull();
    expect(parseEffectKey("abc")).toBeNull();
    expect(parseEffectKey("1-2-3")).toBeNull();
    expect(parseEffectKey("a-b")).toBeNull();
  });
});

describe("deduplicateEffectKeys", () => {
  it("deduplicates fixture-specific keys to logical pairs", () => {
    const keys = ["1:0-0", "2:0-0", "3:0-0", "1:1-2"];
    const result = deduplicateEffectKeys(keys);
    expect(result).toEqual([
      [0, 0],
      [1, 2],
    ]);
  });

  it("handles empty input", () => {
    expect(deduplicateEffectKeys([])).toEqual([]);
  });

  it("skips malformed keys", () => {
    const keys = ["1:0-0", "bad", "1:1-1"];
    const result = deduplicateEffectKeys(keys);
    expect(result).toEqual([
      [0, 0],
      [1, 1],
    ]);
  });
});
