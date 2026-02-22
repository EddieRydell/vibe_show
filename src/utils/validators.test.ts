import { describe, it, expect } from "vitest";
import { parseChatEntries } from "./validators";

describe("parseChatEntries", () => {
  it("returns empty array for non-array input", () => {
    expect(parseChatEntries(null)).toEqual([]);
    expect(parseChatEntries(undefined)).toEqual([]);
    expect(parseChatEntries("string")).toEqual([]);
    expect(parseChatEntries(42)).toEqual([]);
    expect(parseChatEntries({})).toEqual([]);
  });

  it("returns empty array for empty array", () => {
    expect(parseChatEntries([])).toEqual([]);
  });

  it("parses valid chat entries", () => {
    const input = [
      { role: "user", text: "hello" },
      { role: "assistant", text: "hi there" },
    ];
    expect(parseChatEntries(input)).toEqual(input);
  });

  it("filters out entries missing role", () => {
    const input = [
      { role: "user", text: "hello" },
      { text: "no role" },
    ];
    expect(parseChatEntries(input)).toEqual([{ role: "user", text: "hello" }]);
  });

  it("filters out entries missing text", () => {
    const input = [
      { role: "user", text: "hello" },
      { role: "assistant" },
    ];
    expect(parseChatEntries(input)).toEqual([{ role: "user", text: "hello" }]);
  });

  it("filters out non-object entries", () => {
    const input = [
      { role: "user", text: "hello" },
      "string",
      42,
      null,
    ];
    expect(parseChatEntries(input)).toEqual([{ role: "user", text: "hello" }]);
  });

  it("accepts entries with extra fields", () => {
    const input = [{ role: "user", text: "hello", extra: "data" }];
    const result = parseChatEntries(input);
    expect(result).toHaveLength(1);
    expect(result[0].role).toBe("user");
    expect(result[0].text).toBe("hello");
  });
});
