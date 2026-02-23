/** Map song section labels to semi-transparent background colors. */
const SECTION_COLORS: Record<string, string> = {
  intro: "rgba(100, 149, 237, 0.12)",
  verse: "rgba(60, 179, 113, 0.12)",
  chorus: "rgba(255, 165, 0, 0.15)",
  bridge: "rgba(186, 85, 211, 0.12)",
  outro: "rgba(100, 149, 237, 0.12)",
  solo: "rgba(255, 99, 71, 0.12)",
  inst: "rgba(255, 215, 0, 0.12)",
};

export function sectionColor(label: string): string {
  const key = label.toLowerCase().replace(/[0-9]/g, "").trim();
  return SECTION_COLORS[key] ?? "rgba(128, 128, 128, 0.08)";
}
