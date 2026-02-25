import { useCallback, useEffect, useRef, type DependencyList } from "react";

/**
 * Runs a side effect after deps stabilize for `delayMs`.
 * Useful for auto-compile, auto-save, or similar patterns.
 */
export function useDebouncedEffect(
  effect: () => void,
  delayMs: number,
  deps: DependencyList,
): void {
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(effect, delayMs);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, deps);
}

/**
 * Returns a debounced version of `fn` that coalesces rapid calls.
 * Useful for onChange handlers that trigger expensive IPC.
 */
export function useDebouncedCallback<Args extends unknown[]>(
  fn: (...args: Args) => void,
  delayMs: number,
): (...args: Args) => void {
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const fnRef = useRef(fn);
  fnRef.current = fn;

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  return useCallback(
    (...args: Args) => {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => fnRef.current(...args), delayMs);
    },
    [delayMs],
  );
}
