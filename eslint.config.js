import eslint from "@eslint/js";
import betterTailwindcss from "eslint-plugin-better-tailwindcss";
import tseslint from "typescript-eslint";

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: {
      "better-tailwindcss": betterTailwindcss,
    },
    settings: {
      "better-tailwindcss": {
        entryPoint: "src/index.css",
        detectComponentClasses: true,
      },
    },
    rules: {
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_" },
      ],
      // Allow non-null assertions — needed as escape hatch for noUncheckedIndexedAccess
      "@typescript-eslint/no-non-null-assertion": "off",
      // Allow arrow shorthand for void returns (onClick={() => fn()) is idiomatic React)
      "@typescript-eslint/no-confusing-void-expression": [
        "error",
        { ignoreArrowShorthand: true },
      ],
      // Allow numbers and booleans in template literals (they're always safe)
      "@typescript-eslint/restrict-template-expressions": [
        "error",
        { allowNumber: true, allowBoolean: true },
      ],
      // Prettier handles class ordering via prettier-plugin-tailwindcss
      "better-tailwindcss/enforce-consistent-class-order": "off",
      // SSOT: flag classes not in Tailwind config
      "better-tailwindcss/no-unknown-classes": "warn",
      // Catch contradicting classes like "p-2 p-4"
      "better-tailwindcss/no-conflicting-classes": "error",
      // Flag duplicate classes
      "better-tailwindcss/no-duplicate-classes": "warn",
      // Normalize to canonical form
      "better-tailwindcss/enforce-canonical-classes": "warn",
      // Ban arbitrary color values — use theme tokens instead
      "better-tailwindcss/no-restricted-classes": ["warn", {
        restrict: [
          {
            pattern: "^([a-zA-Z0-9:/_-]*:)?(bg|text|border|ring|outline|shadow|fill|stroke|accent|caret|decoration)-\\[#[0-9a-fA-F]+\\]$",
            message: "Use a theme token instead of an arbitrary color value.",
          },
          {
            pattern: "^([a-zA-Z0-9:/_-]*:)?(bg|text|border|ring|outline|shadow|fill|stroke|accent|caret|decoration)-\\[rgb",
            message: "Use a theme token instead of an arbitrary color value.",
          },
        ],
      }],
    },
  },
  {
    ignores: ["dist/", "src-tauri/"],
  }
);
