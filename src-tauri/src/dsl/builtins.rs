use super::ast::TypeName;
use super::compiler::Op;

/// Built-in function: single source of truth for name, type signature, AND opcode.
/// Adding a builtin means adding ONE entry here — typeck and compiler both read from this.
#[derive(Debug, Clone)]
pub struct BuiltinFn {
    pub name: &'static str,
    pub params: &'static [TypeName],
    pub ret: TypeName,
    pub op: Op,
}

/// All built-in functions available in the DSL.
pub static BUILTINS: &[BuiltinFn] = &[
    // Math (1-arg)
    BuiltinFn { name: "sin",   params: &[TypeName::Float], ret: TypeName::Float, op: Op::Sin },
    BuiltinFn { name: "cos",   params: &[TypeName::Float], ret: TypeName::Float, op: Op::Cos },
    BuiltinFn { name: "tan",   params: &[TypeName::Float], ret: TypeName::Float, op: Op::Tan },
    BuiltinFn { name: "abs",   params: &[TypeName::Float], ret: TypeName::Float, op: Op::Abs },
    BuiltinFn { name: "floor", params: &[TypeName::Float], ret: TypeName::Float, op: Op::Floor },
    BuiltinFn { name: "ceil",  params: &[TypeName::Float], ret: TypeName::Float, op: Op::Ceil },
    BuiltinFn { name: "round", params: &[TypeName::Float], ret: TypeName::Float, op: Op::Round },
    BuiltinFn { name: "fract", params: &[TypeName::Float], ret: TypeName::Float, op: Op::Fract },
    BuiltinFn { name: "sqrt",  params: &[TypeName::Float], ret: TypeName::Float, op: Op::Sqrt },
    // Math (2-arg)
    BuiltinFn { name: "pow",   params: &[TypeName::Float, TypeName::Float], ret: TypeName::Float, op: Op::Pow },
    BuiltinFn { name: "min",   params: &[TypeName::Float, TypeName::Float], ret: TypeName::Float, op: Op::Min },
    BuiltinFn { name: "max",   params: &[TypeName::Float, TypeName::Float], ret: TypeName::Float, op: Op::Max },
    BuiltinFn { name: "step",  params: &[TypeName::Float, TypeName::Float], ret: TypeName::Float, op: Op::Step },
    BuiltinFn { name: "atan2", params: &[TypeName::Float, TypeName::Float], ret: TypeName::Float, op: Op::Atan2 },
    // Math (3-arg)
    BuiltinFn { name: "clamp",      params: &[TypeName::Float, TypeName::Float, TypeName::Float], ret: TypeName::Float, op: Op::Clamp },
    BuiltinFn { name: "mix",        params: &[TypeName::Float, TypeName::Float, TypeName::Float], ret: TypeName::Float, op: Op::Mix },
    BuiltinFn { name: "smoothstep", params: &[TypeName::Float, TypeName::Float, TypeName::Float], ret: TypeName::Float, op: Op::Smoothstep },
    // Color constructors
    BuiltinFn { name: "rgb",  params: &[TypeName::Float, TypeName::Float, TypeName::Float], ret: TypeName::Color, op: Op::Rgb },
    BuiltinFn { name: "hsv",  params: &[TypeName::Float, TypeName::Float, TypeName::Float], ret: TypeName::Color, op: Op::Hsv },
    BuiltinFn { name: "rgba", params: &[TypeName::Float, TypeName::Float, TypeName::Float, TypeName::Float], ret: TypeName::Color, op: Op::Rgba },
    // Vec2 constructor
    BuiltinFn { name: "vec2",     params: &[TypeName::Float, TypeName::Float], ret: TypeName::Vec2, op: Op::MakeVec2 },
    BuiltinFn { name: "distance", params: &[TypeName::Vec2, TypeName::Vec2], ret: TypeName::Float, op: Op::Distance },
    BuiltinFn { name: "length",   params: &[TypeName::Vec2], ret: TypeName::Float, op: Op::Length },
    // Random
    BuiltinFn { name: "hash", params: &[TypeName::Float, TypeName::Float], ret: TypeName::Float, op: Op::Hash },
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
    Pi,
    Tau,
}

pub static IMPLICIT_VARS: &[(&str, TypeName, BuiltinVar)] = &[
    ("t",     TypeName::Float, BuiltinVar::T),
    ("pixel", TypeName::Int,   BuiltinVar::Pixel),
    ("pixels",TypeName::Int,   BuiltinVar::Pixels),
    ("pos",   TypeName::Float, BuiltinVar::Pos),
    ("pos2d", TypeName::Vec2,  BuiltinVar::Pos2d),
    ("PI",    TypeName::Float, BuiltinVar::Pi),
    ("TAU",   TypeName::Float, BuiltinVar::Tau),
];

pub fn lookup_builtin(name: &str) -> Option<&'static BuiltinFn> {
    BUILTINS.iter().find(|b| b.name == name)
}

pub fn lookup_implicit(name: &str) -> Option<(&TypeName, BuiltinVar)> {
    IMPLICIT_VARS.iter()
        .find(|&&(n, _, _)| n == name)
        .map(|(_, ty, var)| (ty, *var))
}
