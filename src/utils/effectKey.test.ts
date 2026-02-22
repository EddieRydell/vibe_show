import { describe, it, expect } from "vitest";
import {
  makeEffectKey,
  parseEffectKey,
  deduplicateEffectKeys,
} from "./effectKey";

describe("makeEffectKey", () => {
  it("creates canonical trackIndex-effectIndex format", () => {
    expect(makeEffectKey(0, 0)).toBe("0-0");
    expect(makeEffectKey(2, 3)).toBe("2-3");
    expect(makeEffectKey(10, 5)).toBe("10-5");
  });

  it("round-trips through parseEffectKey", () => {
    const key = makeEffectKey(4, 7);
    const parsed = parseEffectKey(key);
    expect(parsed).toEqual({ trackIndex: 4, effectIndex: 7 });
  });
});

describe("parseEffectKey", () => {
  it("parses canonical key", () => {
    expect(parseEffectKey("2-3")).toEqual({ trackIndex: 2, effectIndex: 3 });
  });

  it("parses zero indices", () => {
    expect(parseEffectKey("0-0")).toEqual({ trackIndex: 0, effectIndex: 0 });
  });

  it("returns null for malformed keys", () => {
    expect(parseEffectKey("")).toBeNull();
    expect(parseEffectKey("abc")).toBeNull();
    expect(parseEffectKey("1-2-3")).toBeNull();
    expect(parseEffectKey("a-b")).toBeNull();
  });

  it("returns null for keys with colon prefix (no longer supported)", () => {
    expect(parseEffectKey("5:2-3")).toBeNull();
  });
});

describe("deduplicateEffectKeys", () => {
  it("deduplicates identical keys", () => {
    const keys = ["0-0", "0-0", "1-2"];
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
    const keys = ["0-0", "bad", "1-1"];
    const result = deduplicateEffectKeys(keys);
    expect(result).toEqual([
      [0, 0],
      [1, 1],
    ]);
  });
});
