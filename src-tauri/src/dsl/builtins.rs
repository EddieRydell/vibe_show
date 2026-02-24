use super::ast::TypeName;
use super::compiler::Op;

/// Built-in function: single source of truth for name, type signature, AND opcode.
/// Adding a builtin means adding ONE entry here — typeck, compiler, and reference docs
/// all read from this.
#[derive(Debug, Clone)]
pub struct BuiltinFn {
    pub name: &'static str,
    pub params: &'static [(&'static str, TypeName)],
    pub ret: TypeName,
    pub op: Op,
    pub category: &'static str,
    pub description: &'static str,
}

/// All built-in functions available in the DSL.
pub static BUILTINS: &[BuiltinFn] = &[
    // ── Math (1-arg) ────────────────────────────────────────────
    BuiltinFn {
        name: "sin", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Sin, category: "math", description: "Sine",
    },
    BuiltinFn {
        name: "cos", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Cos, category: "math", description: "Cosine",
    },
    BuiltinFn {
        name: "tan", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Tan, category: "math", description: "Tangent",
    },
    BuiltinFn {
        name: "abs", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Abs, category: "math", description: "Absolute value",
    },
    BuiltinFn {
        name: "floor", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Floor, category: "math", description: "Round down",
    },
    BuiltinFn {
        name: "ceil", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Ceil, category: "math", description: "Round up",
    },
    BuiltinFn {
        name: "round", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Round, category: "math", description: "Round to nearest",
    },
    BuiltinFn {
        name: "fract", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Fract, category: "math", description: "Fractional part (x - floor(x))",
    },
    BuiltinFn {
        name: "sqrt", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Sqrt, category: "math", description: "Square root",
    },
    BuiltinFn {
        name: "sign", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Sign, category: "math", description: "Sign: -1.0, 0.0, or 1.0",
    },
    BuiltinFn {
        name: "exp", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Exp, category: "math", description: "e^x (exponential)",
    },
    BuiltinFn {
        name: "log", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Log, category: "math", description: "Natural logarithm (ln)",
    },
    // ── Math (2-arg) ────────────────────────────────────────────
    BuiltinFn {
        name: "pow", params: &[("base", TypeName::Float), ("exp", TypeName::Float)], ret: TypeName::Float,
        op: Op::Pow, category: "math", description: "Power",
    },
    BuiltinFn {
        name: "min", params: &[("a", TypeName::Float), ("b", TypeName::Float)], ret: TypeName::Float,
        op: Op::Min, category: "math", description: "Minimum",
    },
    BuiltinFn {
        name: "max", params: &[("a", TypeName::Float), ("b", TypeName::Float)], ret: TypeName::Float,
        op: Op::Max, category: "math", description: "Maximum",
    },
    BuiltinFn {
        name: "step", params: &[("edge", TypeName::Float), ("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Step, category: "math", description: "0 if x < edge, else 1",
    },
    BuiltinFn {
        name: "atan2", params: &[("y", TypeName::Float), ("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Atan2, category: "math", description: "Arctangent of y/x",
    },
    BuiltinFn {
        name: "mod", params: &[("a", TypeName::Float), ("b", TypeName::Float)], ret: TypeName::Float,
        op: Op::Modf, category: "math", description: "Modulo (same as a % b). Returns 0 if b is 0",
    },
    // ── Math (3-arg) ────────────────────────────────────────────
    BuiltinFn {
        name: "clamp", params: &[("x", TypeName::Float), ("min", TypeName::Float), ("max", TypeName::Float)], ret: TypeName::Float,
        op: Op::Clamp, category: "math", description: "Constrain x to [min, max]",
    },
    BuiltinFn {
        name: "mix", params: &[("a", TypeName::Float), ("b", TypeName::Float), ("t", TypeName::Float)], ret: TypeName::Float,
        op: Op::Mix, category: "math", description: "Linear interpolation: a + (b - a) * t",
    },
    BuiltinFn {
        name: "smoothstep", params: &[("e0", TypeName::Float), ("e1", TypeName::Float), ("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Smoothstep, category: "math", description: "Smooth Hermite interpolation. Requires e0 < e1; returns 0 if e0 >= e1",
    },
    // ── Color constructors ──────────────────────────────────────
    BuiltinFn {
        name: "rgb", params: &[("r", TypeName::Float), ("g", TypeName::Float), ("b", TypeName::Float)], ret: TypeName::Color,
        op: Op::Rgb, category: "color", description: "RGB color (0.0-1.0 range)",
    },
    BuiltinFn {
        name: "hsv", params: &[("h", TypeName::Float), ("s", TypeName::Float), ("v", TypeName::Float)], ret: TypeName::Color,
        op: Op::Hsv, category: "color", description: "HSV color (h: 0-360, s: 0-1, v: 0-1)",
    },
    BuiltinFn {
        name: "rgba", params: &[("r", TypeName::Float), ("g", TypeName::Float), ("b", TypeName::Float), ("a", TypeName::Float)], ret: TypeName::Color,
        op: Op::Rgba, category: "color", description: "RGBA color (0.0-1.0 range)",
    },
    // ── Vec2 ────────────────────────────────────────────────────
    BuiltinFn {
        name: "vec2", params: &[("x", TypeName::Float), ("y", TypeName::Float)], ret: TypeName::Vec2,
        op: Op::MakeVec2, category: "vec2", description: "Construct vec2",
    },
    BuiltinFn {
        name: "distance", params: &[("a", TypeName::Vec2), ("b", TypeName::Vec2)], ret: TypeName::Float,
        op: Op::Distance, category: "vec2", description: "Euclidean distance between two vec2",
    },
    BuiltinFn {
        name: "length", params: &[("v", TypeName::Vec2)], ret: TypeName::Float,
        op: Op::Length, category: "vec2", description: "Length of vec2",
    },
    BuiltinFn {
        name: "dot", params: &[("a", TypeName::Vec2), ("b", TypeName::Vec2)], ret: TypeName::Float,
        op: Op::Dot, category: "vec2", description: "Dot product of two vec2",
    },
    BuiltinFn {
        name: "normalize", params: &[("v", TypeName::Vec2)], ret: TypeName::Vec2,
        op: Op::Normalize, category: "vec2", description: "Normalize vec2 to unit length",
    },
    // ── Hash / Random ───────────────────────────────────────────
    BuiltinFn {
        name: "hash", params: &[("a", TypeName::Float), ("b", TypeName::Float)], ret: TypeName::Float,
        op: Op::Hash, category: "hash", description: "Deterministic pseudo-random [0, 1]. Same inputs always produce same output",
    },
    BuiltinFn {
        name: "hash3", params: &[("a", TypeName::Float), ("b", TypeName::Float), ("c", TypeName::Float)], ret: TypeName::Float,
        op: Op::Hash3, category: "hash", description: "Deterministic pseudo-random [0, 1] with 3 inputs",
    },
    BuiltinFn {
        name: "random", params: &[("seed", TypeName::Float)], ret: TypeName::Float,
        op: Op::Random, category: "hash", description: "Pseudo-random [0, 1] from seed",
    },
    BuiltinFn {
        name: "random_range", params: &[("seed", TypeName::Float), ("min", TypeName::Float), ("max", TypeName::Float)], ret: TypeName::Float,
        op: Op::RandomRange, category: "hash", description: "Pseudo-random in [min, max] from seed",
    },
    // ── Easing ──────────────────────────────────────────────────
    BuiltinFn {
        name: "ease_in", params: &[("t", TypeName::Float)], ret: TypeName::Float,
        op: Op::EaseIn, category: "easing", description: "Quadratic ease-in (t^2)",
    },
    BuiltinFn {
        name: "ease_out", params: &[("t", TypeName::Float)], ret: TypeName::Float,
        op: Op::EaseOut, category: "easing", description: "Quadratic ease-out",
    },
    BuiltinFn {
        name: "ease_in_out", params: &[("t", TypeName::Float)], ret: TypeName::Float,
        op: Op::EaseInOut, category: "easing", description: "Quadratic ease-in-out",
    },
    BuiltinFn {
        name: "ease_in_cubic", params: &[("t", TypeName::Float)], ret: TypeName::Float,
        op: Op::EaseInCubic, category: "easing", description: "Cubic ease-in (t^3)",
    },
    BuiltinFn {
        name: "ease_out_cubic", params: &[("t", TypeName::Float)], ret: TypeName::Float,
        op: Op::EaseOutCubic, category: "easing", description: "Cubic ease-out",
    },
    BuiltinFn {
        name: "ease_in_out_cubic", params: &[("t", TypeName::Float)], ret: TypeName::Float,
        op: Op::EaseInOutCubic, category: "easing", description: "Cubic ease-in-out",
    },
    // ── Noise ───────────────────────────────────────────────────
    BuiltinFn {
        name: "noise", params: &[("x", TypeName::Float)], ret: TypeName::Float,
        op: Op::Noise1, category: "noise", description: "1D Perlin noise. Returns [-1, 1]",
    },
    BuiltinFn {
        name: "noise2", params: &[("x", TypeName::Float), ("y", TypeName::Float)], ret: TypeName::Float,
        op: Op::Noise2, category: "noise", description: "2D Perlin noise. Returns [-1, 1]",
    },
    BuiltinFn {
        name: "noise3", params: &[("x", TypeName::Float), ("y", TypeName::Float), ("z", TypeName::Float)], ret: TypeName::Float,
        op: Op::Noise3, category: "noise", description: "3D Perlin noise. Returns [-1, 1]",
    },
    BuiltinFn {
        name: "fbm", params: &[("x", TypeName::Float), ("y", TypeName::Float), ("octaves", TypeName::Float)], ret: TypeName::Float,
        op: Op::Fbm, category: "noise", description: "Fractal Brownian motion (layered 2D noise)",
    },
    BuiltinFn {
        name: "worley2", params: &[("x", TypeName::Float), ("y", TypeName::Float)], ret: TypeName::Float,
        op: Op::Worley2, category: "noise", description: "2D Worley/cellular noise. Returns [0, 1]",
    },
];

/// Implicit builtin variables: single source of truth for name, type, AND var enum.
/// Used by both the type checker (for type resolution) and the compiler (for opcode emission).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinVar {
    T,
    Pixel,
    Pixels,
    Pos,
    Pos2d,
    AbsT,
    Pi,
    Tau,
}

pub static IMPLICIT_VARS: &[(&str, TypeName, BuiltinVar, &str)] = &[
    ("t",     TypeName::Float, BuiltinVar::T,     "Normalized time [0.0, 1.0] within effect duration"),
    ("pixel", TypeName::Float, BuiltinVar::Pixel, "Current pixel index (0-based)"),
    ("pixels",TypeName::Float, BuiltinVar::Pixels,"Total pixel count in the effect's target"),
    ("pos",   TypeName::Float, BuiltinVar::Pos,   "Normalized position: pixel / (pixels - 1), range [0.0, 1.0]"),
    ("pos2d", TypeName::Vec2,  BuiltinVar::Pos2d, "2D position (requires @spatial true)"),
    ("abs_t", TypeName::Float, BuiltinVar::AbsT,  "Absolute time in seconds (for motion path evaluation)"),
    ("PI",    TypeName::Float, BuiltinVar::Pi,    "3.14159..."),
    ("TAU",   TypeName::Float, BuiltinVar::Tau,   "6.28318... (2\u{03C0})"),
];

pub fn lookup_builtin(name: &str) -> Option<&'static BuiltinFn> {
    BUILTINS.iter().find(|b| b.name == name)
}

pub fn lookup_implicit(name: &str) -> Option<(&TypeName, BuiltinVar)> {
    IMPLICIT_VARS.iter()
        .find(|&&(n, _, _, _)| n == name)
        .map(|(_, ty, var, _)| (ty, *var))
}
