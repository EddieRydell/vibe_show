import { useCallback, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { EditorView } from "@codemirror/view";
import { cmd } from "../../commands";
import { vibelightsLanguage } from "../../editor/vibelights-lang";
import type { ScriptCompileResult } from "../../types";
import { useDebouncedEffect } from "../../hooks/useDebounce";

interface Props {
  scriptName: string | null;
  source: string;
  onSourceChange: (source: string) => void;
  onCompileResult: (result: ScriptCompileResult) => void;
  onSave: () => void;
}

const extensions = [vibelightsLanguage(), EditorView.lineWrapping];

export function ScriptEditor({
  scriptName,
  source,
  onSourceChange,
  onCompileResult,
  onSave,
}: Props) {
  const [compiling, setCompiling] = useState(false);

  // Auto-compile on source change (debounced)
  useDebouncedEffect(
    () => {
      if (!source.trim()) return;
      setCompiling(true);
      const compilePromise = scriptName
        ? cmd.compileGlobalScript(scriptName, source)
        : cmd.compileScriptPreview(source);

      compilePromise
        .then(onCompileResult)
        .catch(console.error)
        .finally(() => setCompiling(false));
    },
    400,
    [source, scriptName, onCompileResult],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "s" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        onSave();
      }
    },
    [onSave],
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col" onKeyDown={handleKeyDown}>
      {/* Compile status */}
      <div className="border-border bg-surface flex items-center gap-2 border-b px-3 py-1">
        <span className="text-text-2 text-[10px]">
          {compiling ? "Compiling..." : ""}
        </span>
      </div>

      {/* CodeMirror editor */}
      <div className="min-h-0 flex-1 overflow-hidden">
        <CodeMirror
          value={source}
          onChange={onSourceChange}
          extensions={extensions}
          theme="dark"
          height="100%"
          className="h-full text-xs [&_.cm-editor]:h-full [&_.cm-scroller]:overflow-auto!"
          basicSetup={{
            lineNumbers: true,
            foldGutter: false,
            highlightActiveLine: true,
            bracketMatching: true,
            closeBrackets: true,
            indentOnInput: true,
            tabSize: 2,
          }}
        />
      </div>
    </div>
  );
}
