import { useEffect, useRef, useState } from "react";
import type { EffectInfo, EffectKind } from "../types";
import { cmd } from "../commands";
import { useToast } from "../hooks/useToast";

interface EffectPickerProps {
  position: { x: number; y: number };
  onSelect: (kind: EffectKind) => void;
  onCancel: () => void;
}

export function EffectPicker({ position, onSelect, onCancel }: EffectPickerProps) {
  const [effects, setEffects] = useState<EffectInfo[]>([]);
  const [scripts, setScripts] = useState<string[]>([]);
  const ref = useRef<HTMLDivElement>(null);
  const { showError } = useToast();

  useEffect(() => {
    cmd.listEffects().then(setEffects).catch(showError);
    cmd.listGlobalScripts().then((pairs) => setScripts(pairs.map(([n]) => n))).catch(showError);
  }, [showError]);

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

  const effectKindKey = (kind: EffectKind): string =>
    typeof kind === "string" ? kind : `Script:${kind.Script}`;

  return (
    <div
      ref={ref}
      role="menu"
      aria-label="Add effect"
      className="border-border bg-surface fixed z-50 rounded-md border py-1 shadow-lg"
      style={{ left: position.x, top: position.y }}
    >
      <div className="text-text-2 px-3 py-1 text-[10px] tracking-wider uppercase">
        Add Effect
      </div>
      {effects.map((effect) => (
        <button
          key={effectKindKey(effect.kind)}
          role="menuitem"
          className="text-text hover:bg-primary/15 hover:text-primary flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs transition-colors"
          onClick={() => onSelect(effect.kind)}
        >
          {effect.name}
        </button>
      ))}
      {scripts.length > 0 && (
        <>
          <div className="bg-border mx-2 my-1 h-px" />
          <div className="text-text-2 px-3 py-1 text-[10px] tracking-wider uppercase">
            Scripts
          </div>
          {scripts.map((name) => (
            <button
              key={`script:${name}`}
              role="menuitem"
              className="text-text hover:bg-primary/15 hover:text-primary flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs transition-colors"
              onClick={() => onSelect({ Script: name })}
            >
              {name}
            </button>
          ))}
        </>
      )}
    </div>
  );
}
