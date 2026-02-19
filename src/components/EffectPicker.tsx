import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { EffectInfo, EffectKind } from "../types";

interface EffectPickerProps {
  position: { x: number; y: number };
  onSelect: (kind: EffectKind) => void;
  onCancel: () => void;
}

export function EffectPicker({ position, onSelect, onCancel }: EffectPickerProps) {
  const [effects, setEffects] = useState<EffectInfo[]>([]);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<EffectInfo[]>("list_effects").then(setEffects).catch(console.error);
  }, []);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onCancel();
      }
    }
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") onCancel();
    }
    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleKey);
    };
  }, [onCancel]);

  return (
    <div
      ref={ref}
      className="border-border bg-surface fixed z-50 rounded-md border py-1 shadow-lg"
      style={{ left: position.x, top: position.y }}
    >
      <div className="text-text-2 px-3 py-1 text-[10px] tracking-wider uppercase">
        Add Effect
      </div>
      {effects.map((effect) => (
        <button
          key={effect.kind}
          className="text-text hover:bg-primary/15 hover:text-primary flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs transition-colors"
          onClick={() => onSelect(effect.kind)}
        >
          {effect.name}
        </button>
      ))}
    </div>
  );
}
