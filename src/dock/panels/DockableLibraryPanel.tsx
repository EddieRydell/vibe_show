import { memo } from "react";
import { LibraryPanel } from "../../components/LibraryPanel";
import { useEditorStore } from "../contexts/EditorContext";

export const DockableLibraryPanel = memo(function DockableLibraryPanel() {
  const commitChange = useEditorStore((s) => s.commitChange);
  const refreshKey = useEditorStore((s) => s.refreshKey);
  return (
    <LibraryPanel
      onLibraryChange={() => commitChange()}
      showVersion={refreshKey}
    />
  );
}, () => true);
