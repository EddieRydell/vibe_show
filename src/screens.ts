export type AppScreen =
  | { kind: "loading" }
  | { kind: "first_launch" }
  | { kind: "home" }
  | { kind: "settings"; returnTo: AppScreen }
  | { kind: "profile"; slug: string }
  | { kind: "editor"; profileSlug: string; sequenceSlug: string };
