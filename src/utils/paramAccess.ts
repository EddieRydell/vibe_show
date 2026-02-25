import type { ParamSchema, ParamValue } from "../types";

/**
 * Generic accessor for a param value variant.
 * Returns the inner value of the specified variant, or the fallback if the key
 * is missing or holds a different variant.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function getParam(params: Record<string, ParamValue>, key: string, variant: string, fallback: any): any {
  const v = params[key];
  if (v && variant in v) return (v as Record<string, unknown>)[variant];
  return fallback;
}

/**
 * Generic accessor for a param schema default value variant.
 * Returns the inner value of the specified variant, or the fallback if the
 * default holds a different variant.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function getDefault(schema: ParamSchema, variant: string, fallback: any): any {
  const d = schema.default;
  if (variant in d) return (d as Record<string, unknown>)[variant];
  return fallback;
}
