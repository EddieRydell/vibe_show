export type AppScreen =
  | { kind: "loading" }
  | { kind: "first_launch" }
  | { kind: "home" }
  | { kind: "settings"; returnTo: AppScreen }
  | { kind: "editor"; setupSlug: string; sequenceSlug: string }
  | { kind: "script"; scriptName: string | null; returnTo: AppScreen }
  | { kind: "analysis"; setupSlug: string; filename: string; returnTo: AppScreen };
