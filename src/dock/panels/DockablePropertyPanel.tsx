import { memo } from "react";
import { PropertyPanel } from "../../components/PropertyPanel";
import { useEditorStore } from "../contexts/EditorContext";

export const DockablePropertyPanel = memo(function DockablePropertyPanel() {
  const singleSelected = useEditorStore((s) => s.singleSelected);
  const sequenceIndex = useEditorStore((s) => s.sequenceIndex);
  const refreshKey = useEditorStore((s) => s.refreshKey);
  const handleParamChange = useEditorStore((s) => s.handleParamChange);
  return (
    <PropertyPanel
      selectedEffect={singleSelected}
      sequenceIndex={sequenceIndex}
      showVersion={refreshKey}
      onParamChange={handleParamChange}
    />
  );
}, () => true);
