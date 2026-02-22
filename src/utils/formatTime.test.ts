import { describe, it, expect } from "vitest";
import { formatTimeTransport, formatTimeDuration } from "./formatTime";

describe("formatTimeTransport", () => {
  it("formats zero", () => {
    expect(formatTimeTransport(0)).toBe("0:00.00");
  });

  it("formats seconds with centiseconds", () => {
    expect(formatTimeTransport(5.25)).toBe("0:05.25");
  });

  it("formats minutes", () => {
    expect(formatTimeTransport(125.5)).toBe("2:05.50");
  });

  it("pads seconds to 2 digits", () => {
    expect(formatTimeTransport(3.1)).toBe("0:03.10");
  });
});

describe("formatTimeDuration", () => {
  it("formats short durations with 's' suffix", () => {
    expect(formatTimeDuration(2.5)).toBe("2.5s");
  });

  it("formats durations over a minute with m:ss.d", () => {
    expect(formatTimeDuration(90)).toBe("1:30.0");
  });

  it("formats zero", () => {
    expect(formatTimeDuration(0)).toBe("0.0s");
  });
});
