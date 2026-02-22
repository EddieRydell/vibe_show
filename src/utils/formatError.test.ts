import { describe, it, expect } from "vitest";
import { formatTauriError } from "./formatError";

describe("formatTauriError", () => {
  it("returns string errors as-is", () => {
    expect(formatTauriError("something went wrong")).toBe("something went wrong");
  });

  it("extracts Error.message", () => {
    expect(formatTauriError(new Error("test error"))).toBe("test error");
  });

  it("extracts AppError with detail.message", () => {
    const err = { code: "PythonError", detail: { message: "pip failed" } };
    expect(formatTauriError(err)).toBe("pip failed");
  });

  it("extracts AppError with detail.what (NotFound)", () => {
    const err = { code: "NotFound", detail: { what: "sequence" } };
    expect(formatTauriError(err)).toBe("sequence not found");
  });

  it("extracts AppError with detail.model (ModelNotInstalled)", () => {
    const err = { code: "ModelNotInstalled", detail: { model: "whisper-large" } };
    expect(formatTauriError(err)).toBe("Required model not installed: whisper-large");
  });

  it("formats bare code errors (no detail)", () => {
    const err = { code: "PythonNotReady" };
    expect(formatTauriError(err)).toBe("Python Not Ready");
  });

  it("falls back to JSON for unknown objects", () => {
    const err = { foo: "bar" };
    expect(formatTauriError(err)).toBe('{"foo":"bar"}');
  });

  it("returns 'Unknown error' for null/undefined", () => {
    expect(formatTauriError(null)).toBe("Unknown error");
    expect(formatTauriError(undefined)).toBe("Unknown error");
  });

  it("extracts direct message field", () => {
    const err = { message: "direct message" };
    expect(formatTauriError(err)).toBe("direct message");
  });
});
