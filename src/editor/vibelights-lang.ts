import { LanguageSupport, StreamLanguage, StringStream } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import {
  autocompletion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";

// ── DSL Keywords & Builtins ────────────────────────────────────────

const KEYWORDS = new Set([
  "let", "fn", "if", "else", "param", "enum", "flags", "return",
]);

const TYPES = new Set([
  "float", "int", "bool", "color", "vec2", "gradient", "curve",
]);

const BUILTINS = new Set([
  "sin", "cos", "tan", "abs", "floor", "ceil", "round", "fract", "sqrt",
  "pow", "min", "max", "clamp", "mix", "smoothstep", "step", "atan2",
  "rgb", "hsv", "rgba", "hash", "distance", "length", "vec2",
]);

const IMPLICIT_VARS = new Set([
  "t", "pixel", "pixels", "pos", "pos2d", "PI", "TAU",
]);

// ── StreamLanguage tokenizer ───────────────────────────────────────

const vibelightsDSL = StreamLanguage.define({
  token(stream: StringStream): string | null {
    // Skip whitespace
    if (stream.eatSpace()) return null;

    // Comments
    if (stream.match("//")) {
      stream.skipToEnd();
      return "lineComment";
    }

    // Metadata
    if (stream.match(/@\w+/)) {
      return "meta";
    }

    // Color literals: #rrggbb
    if (stream.match(/#[0-9a-fA-F]{6}\b/)) {
      return "color";
    }

    // Numbers
    if (stream.match(/\d+\.\d*/) || stream.match(/\.\d+/) || stream.match(/\d+/)) {
      return "number";
    }

    // Strings
    if (stream.match(/"([^"\\]|\\.)*"/)) {
      return "string";
    }

    // Identifiers and keywords
    if (stream.match(/[a-zA-Z_]\w*/)) {
      const word = stream.current();
      if (KEYWORDS.has(word)) return "keyword";
      if (TYPES.has(word)) return "typeName";
      if (word === "true" || word === "false") return "bool";
      if (BUILTINS.has(word)) return "variableName.standard";
      if (IMPLICIT_VARS.has(word)) return "variableName.special";
      return "variableName";
    }

    // Operators
    if (stream.match(/[+\-*/%=<>!&|.,:;(){}[\]]/)) {
      return "operator";
    }

    stream.next();
    return null;
  },
});

// ── Autocompletion ─────────────────────────────────────────────────

function vibelightsCompletion(context: CompletionContext): CompletionResult | null {
  const word = context.matchBefore(/\w*/);
  if (!word || (word.from === word.to && !context.explicit)) return null;

  const options = [
    // Keywords
    ...["let", "fn", "if", "else", "param", "enum", "flags", "return"].map((label) => ({
      label,
      type: "keyword" as const,
    })),
    // Types
    ...["float", "int", "bool", "color", "vec2", "gradient", "curve"].map((label) => ({
      label,
      type: "type" as const,
      detail: "type",
    })),
    // Built-in functions
    ...Array.from(BUILTINS).map((label) => ({
      label,
      type: "function" as const,
      detail: "builtin",
    })),
    // Implicit variables
    { label: "t", type: "variable" as const, detail: "normalized time [0..1]" },
    { label: "pixel", type: "variable" as const, detail: "pixel index" },
    { label: "pixels", type: "variable" as const, detail: "total pixel count" },
    { label: "pos", type: "variable" as const, detail: "normalized position [0..1]" },
    { label: "pos2d", type: "variable" as const, detail: "2D position (x, y)" },
    { label: "PI", type: "constant" as const, detail: "3.14159..." },
    { label: "TAU", type: "constant" as const, detail: "6.28318..." },
    // Metadata
    { label: "@name", type: "keyword" as const, detail: "effect name" },
    { label: "@spatial", type: "keyword" as const, detail: "enable 2D layout" },
    // Snippets
    {
      label: "param-float",
      type: "text" as const,
      detail: "float parameter",
      apply: "param speed: float(0.1, 10.0) = 1.0",
    },
    {
      label: "param-color",
      type: "text" as const,
      detail: "color parameter",
      apply: "param color: color = #ffffff",
    },
    {
      label: "param-bool",
      type: "text" as const,
      detail: "bool parameter",
      apply: "param reverse: bool = false",
    },
    {
      label: "param-gradient",
      type: "text" as const,
      detail: "gradient parameter",
      apply: "param palette: gradient = #000000, #ffffff",
    },
  ];

  return {
    from: word.from,
    options,
  };
}

// ── LanguageSupport export ─────────────────────────────────────────

export function vibelightsLanguage(): LanguageSupport {
  return new LanguageSupport(vibelightsDSL, [
    autocompletion({ override: [vibelightsCompletion] }),
  ]);
}

export { tags };
