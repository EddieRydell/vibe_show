use super::ast::{BinOp, ParamType, Span, UnaryOp};
use super::builtins::{self, BuiltinVar};
use super::error::CompileError;
use super::typeck::{TypedExpr, TypedExprKind, TypedScript, TypedStmt, TypedStmtKind};

/// A compiled DSL script ready for VM execution.
#[derive(Debug, Clone)]
pub struct CompiledScript {
    pub name: String,
    pub spatial: bool,
    pub ops: Vec<Op>,
    pub constants: Vec<f64>,
    /// Param metadata for the runtime to map ParamValue → f64/gradient/curve.
    pub params: Vec<CompiledParam>,
    /// Number of local variable slots needed.
    pub local_count: u16,
    /// Enum definitions: name → variant names (for runtime variant resolution).
    pub enums: Vec<EnumDef>,
    /// Flags definitions: name → flag names (for runtime bitmask resolution).
    pub flags: Vec<FlagsDef>,
}

/// Enum type definition carried into the compiled script for runtime resolution.
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<String>,
}

/// Flags type definition carried into the compiled script for runtime resolution.
#[derive(Debug, Clone)]
pub struct FlagsDef {
    pub name: String,
    pub flags: Vec<String>,
}

/// Compiled param info (index matches TypedParam order).
#[derive(Debug, Clone)]
pub struct CompiledParam {
    pub name: String,
    pub ty: ParamType,
}

/// Bytecode operations for the stack-based VM.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    /// Push a constant from the constant pool.
    PushConst(u16),
    /// Push a parameter value (resolved at runtime).
    PushParam(u16),
    /// Load a local variable onto the stack.
    LoadLocal(u16),
    /// Store top of stack into a local variable slot.
    StoreLocal(u16),
    /// Pop top of stack.
    Pop,

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,

    // Comparison
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,

    // Logic
    And,
    Or,
    Not,

    // Math (1-arg)
    Sin,
    Cos,
    Tan,
    Abs,
    Floor,
    Ceil,
    Round,
    Fract,
    Sqrt,

    // Math (2-arg)
    Pow,
    Min,
    Max,
    Step,
    Atan2,

    // Math (3-arg)
    Clamp,
    Mix,
    Smoothstep,

    // Color
    /// Pop r, g, b → push Color
    Rgb,
    /// Pop h, s, v → push Color
    Hsv,
    /// Pop r, g, b, a → push Color
    Rgba,
    /// Pop color, pop float → push scaled color
    ColorScale,
    /// Pop color → push float (r channel)
    ColorR,
    /// Pop color → push float (g channel)
    ColorG,
    /// Pop color → push float (b channel)
    ColorB,
    /// Pop color → push float (a channel)
    ColorA,

    // Vec2
    /// Pop x, y → push Vec2
    MakeVec2,
    /// Pop Vec2 → push float (x)
    Vec2X,
    /// Pop Vec2 → push float (y)
    Vec2Y,
    /// Pop Vec2, Vec2 → push float distance
    Distance,
    /// Pop Vec2 → push float length
    Length,

    // Gradient/Curve/Color param evaluation
    /// Pop float t → push Color from gradient param
    EvalGradient(u16),
    /// Pop float t → push float from curve param
    EvalCurve(u16),
    /// Push Color from a color param
    LoadColor(u16),

    // Hash
    /// Pop a, b → push float
    Hash,

    // Enum/Flags
    /// Pop int → compare with variant index → push bool
    EnumEq(u16),
    /// Pop int → test bitmask → push bool
    FlagTest(u32),

    // Control flow
    /// Jump forward if top of stack is false (pop condition).
    JumpIfFalse(u16),
    /// Unconditional jump.
    Jump(u16),

    // Type conversion
    /// Pop int → push float
    IntToFloat,

    // Builtin variables
    PushT,
    PushPixel,
    PushPixels,
    PushPos,
    PushPos2d,

    /// Halt execution, top of stack is the return color.
    Return,
}

pub fn compile(typed: &TypedScript) -> Result<CompiledScript, CompileError> {
    let mut compiler = Compiler::new();

    // Register params
    for p in &typed.params {
        compiler.params.push(CompiledParam {
            name: p.name.clone(),
            ty: p.ty.clone(),
        });
    }

    // Compile body
    for stmt in &typed.body {
        compiler.compile_stmt(stmt)?;
    }

    compiler.emit(Op::Return);

    Ok(CompiledScript {
        name: typed.name.clone(),
        spatial: typed.spatial,
        ops: compiler.ops,
        constants: compiler.constants,
        params: compiler.params,
        local_count: compiler.local_count,
        enums: typed.enums.iter().map(|td| EnumDef {
            name: td.name.clone(),
            variants: td.variants.clone(),
        }).collect(),
        flags: typed.flags.iter().map(|td| FlagsDef {
            name: td.name.clone(),
            flags: td.variants.clone(),
        }).collect(),
    })
}

struct Compiler {
    ops: Vec<Op>,
    constants: Vec<f64>,
    params: Vec<CompiledParam>,
    local_count: u16,
}

impl Compiler {
    fn new() -> Self {
        Self {
            ops: Vec::new(),
            constants: Vec::new(),
            params: Vec::new(),
            local_count: 0,
        }
    }

    fn emit(&mut self, op: Op) {
        self.ops.push(op);
    }

    fn emit_const(&mut self, value: f64) {
        let idx = self.add_constant(value);
        self.emit(Op::PushConst(idx));
    }

    fn add_constant(&mut self, value: f64) -> u16 {
        // Check if constant already exists (exact bit equality)
        for (i, &c) in self.constants.iter().enumerate() {
            if c.to_bits() == value.to_bits() {
                return i as u16;
            }
        }
        let idx = self.constants.len() as u16;
        self.constants.push(value);
        idx
    }

    fn current_offset(&self) -> usize {
        self.ops.len()
    }

    fn patch_jump(&mut self, idx: usize) {
        let target = self.ops.len() as u16;
        match &mut self.ops[idx] {
            Op::JumpIfFalse(ref mut dest) | Op::Jump(ref mut dest) => *dest = target,
            _ => {}
        }
    }

    fn compile_stmt(&mut self, stmt: &TypedStmt) -> Result<(), CompileError> {
        match &stmt.kind {
            TypedStmtKind::Let { value, local_index, .. } => {
                self.compile_expr(value)?;
                let idx = *local_index;
                if idx >= self.local_count {
                    self.local_count = idx + 1;
                }
                self.emit(Op::StoreLocal(idx));
                Ok(())
            }
            TypedStmtKind::Expr(expr) => {
                self.compile_expr(expr)?;
                Ok(())
            }
        }
    }

    fn compile_expr(&mut self, expr: &TypedExpr) -> Result<(), CompileError> {
        match &expr.kind {
            TypedExprKind::FloatLit(v) => {
                self.emit_const(*v);
            }
            TypedExprKind::IntLit(v) => {
                self.emit_const(f64::from(*v));
            }
            TypedExprKind::BoolLit(v) => {
                self.emit_const(if *v { 1.0 } else { 0.0 });
            }
            TypedExprKind::ColorLit { r, g, b } => {
                self.emit_const(f64::from(*r) / 255.0);
                self.emit_const(f64::from(*g) / 255.0);
                self.emit_const(f64::from(*b) / 255.0);
                self.emit(Op::Rgb);
            }
            TypedExprKind::LoadLocal(idx) => {
                self.emit(Op::LoadLocal(*idx));
            }
            TypedExprKind::LoadParam(idx) => {
                self.emit(Op::PushParam(*idx));
            }
            TypedExprKind::LoadColor(idx) => {
                self.emit(Op::LoadColor(*idx));
            }
            TypedExprKind::LoadBuiltin(var) => {
                self.emit(match var {
                    BuiltinVar::T => Op::PushT,
                    BuiltinVar::Pixel => Op::PushPixel,
                    BuiltinVar::Pixels => Op::PushPixels,
                    BuiltinVar::Pos => Op::PushPos,
                    BuiltinVar::Pos2d => Op::PushPos2d,
                    BuiltinVar::Pi => {
                        self.emit_const(std::f64::consts::PI);
                        return Ok(());
                    }
                    BuiltinVar::Tau => {
                        self.emit_const(std::f64::consts::TAU);
                        return Ok(());
                    }
                });
            }
            TypedExprKind::BinOp { op, left, right } => {
                self.compile_expr(left)?;
                self.compile_expr(right)?;
                self.emit(match op {
                    BinOp::Add => Op::Add,
                    BinOp::Sub => Op::Sub,
                    BinOp::Mul => Op::Mul,
                    BinOp::Div => Op::Div,
                    BinOp::Mod => Op::Mod,
                    BinOp::Lt => Op::Lt,
                    BinOp::Gt => Op::Gt,
                    BinOp::Le => Op::Le,
                    BinOp::Ge => Op::Ge,
                    BinOp::Eq => Op::Eq,
                    BinOp::Ne => Op::Ne,
                    BinOp::And => Op::And,
                    BinOp::Or => Op::Or,
                    BinOp::BitOr => {
                        return Err(CompileError::compiler(
                            "BitOr should not appear in compiled code",
                            expr.span,
                        ));
                    }
                });
            }
            TypedExprKind::UnaryOp { op, operand } => {
                self.compile_expr(operand)?;
                self.emit(match op {
                    UnaryOp::Neg => Op::Neg,
                    UnaryOp::Not => Op::Not,
                });
            }
            TypedExprKind::BuiltinCall { name, args } => {
                for arg in args {
                    self.compile_expr(arg)?;
                }
                self.emit(Self::builtin_op(name, expr.span)?);
            }
            TypedExprKind::EvalGradient { param_index, arg } => {
                self.compile_expr(arg)?;
                self.emit(Op::EvalGradient(*param_index));
            }
            TypedExprKind::EvalCurve { param_index, arg } => {
                self.compile_expr(arg)?;
                self.emit(Op::EvalCurve(*param_index));
            }
            TypedExprKind::ColorScale { color, factor } => {
                self.compile_expr(color)?;
                self.compile_expr(factor)?;
                self.emit(Op::ColorScale);
            }
            TypedExprKind::Field { object, field } => {
                self.compile_expr(object)?;
                self.emit(match field.as_str() {
                    "r" => Op::ColorR,
                    "g" => Op::ColorG,
                    "b" => Op::ColorB,
                    "a" => Op::ColorA,
                    "x" => Op::Vec2X,
                    "y" => Op::Vec2Y,
                    _ => {
                        return Err(CompileError::compiler(
                            format!("Unknown field '{field}' in compiler"),
                            expr.span,
                        ));
                    }
                });
            }
            TypedExprKind::MakeVec2 { x, y } => {
                self.compile_expr(x)?;
                self.compile_expr(y)?;
                self.emit(Op::MakeVec2);
            }
            TypedExprKind::If { condition, then_body, else_body } => {
                self.compile_expr(condition)?;
                let jump_else = self.current_offset();
                self.emit(Op::JumpIfFalse(0)); // placeholder

                // Compile then branch
                for (i, stmt) in then_body.iter().enumerate() {
                    // For non-last statements that are expressions, pop the value
                    if i > 0 {
                        if let TypedStmtKind::Expr(_) = &then_body[i - 1].kind {
                            // Previous was an expr stmt in the middle — but
                            // actually let stmts don't push values, expr stmts do.
                            // We only want to pop intermediate expr results.
                        }
                    }
                    self.compile_stmt(stmt)?;
                }

                if let Some(else_stmts) = else_body {
                    let jump_end = self.current_offset();
                    self.emit(Op::Jump(0)); // placeholder
                    self.patch_jump(jump_else);

                    // Compile else branch
                    for stmt in else_stmts {
                        self.compile_stmt(stmt)?;
                    }
                    self.patch_jump(jump_end);
                } else {
                    self.patch_jump(jump_else);
                }
            }
            TypedExprKind::EnumEq { param_index, variant_index } => {
                self.emit(Op::PushParam(*param_index));
                self.emit(Op::EnumEq(*variant_index));
            }
            TypedExprKind::FlagTest { param_index, bit_mask } => {
                self.emit(Op::PushParam(*param_index));
                self.emit(Op::FlagTest(*bit_mask));
            }
            TypedExprKind::IntToFloat(inner) => {
                self.compile_expr(inner)?;
                self.emit(Op::IntToFloat);
            }
        }
        Ok(())
    }

    fn builtin_op(name: &str, span: Span) -> Result<Op, CompileError> {
        builtins::lookup_builtin(name)
            .map(|b| b.op)
            .ok_or_else(|| CompileError::compiler(
                format!("Unknown builtin function '{name}' in compiler"),
                span,
            ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dsl::lexer::lex;
    use crate::dsl::parser::parse;
    use crate::dsl::typeck::type_check;

    fn compile_src(src: &str) -> CompiledScript {
        let tokens = lex(src).unwrap();
        let script = parse(tokens).unwrap();
        let typed = type_check(&script).unwrap();
        compile(&typed).unwrap()
    }

    #[test]
    fn solid_red() {
        let compiled = compile_src("rgb(1.0, 0.0, 0.0)");
        assert!(compiled.ops.contains(&Op::Rgb));
        assert!(compiled.ops.contains(&Op::Return));
    }

    #[test]
    fn uses_time() {
        let compiled = compile_src("let x = sin(t * 3.14)\nrgb(x, x, x)");
        assert!(compiled.ops.contains(&Op::PushT));
        assert!(compiled.ops.contains(&Op::Sin));
        assert!(compiled.ops.contains(&Op::Mul));
        assert!(compiled.ops.contains(&Op::StoreLocal(0)));
        assert!(compiled.ops.contains(&Op::LoadLocal(0)));
    }

    #[test]
    fn if_else_branches() {
        let compiled = compile_src("if t > 0.5 {\nrgb(1.0, 0.0, 0.0)\n} else {\nrgb(0.0, 0.0, 1.0)\n}");
        // Should have JumpIfFalse and Jump instructions
        let has_jump_if = compiled.ops.iter().any(|op| matches!(op, Op::JumpIfFalse(_)));
        let has_jump = compiled.ops.iter().any(|op| matches!(op, Op::Jump(_)));
        assert!(has_jump_if);
        assert!(has_jump);
    }

    #[test]
    fn constant_dedup() {
        let compiled = compile_src("rgb(1.0, 1.0, 1.0)");
        // 1.0 should appear only once in the constant pool
        let ones = compiled.constants.iter().filter(|&&c| c == 1.0).count();
        assert_eq!(ones, 1, "Duplicate constants should be deduplicated");
    }

    #[test]
    fn param_push() {
        let compiled = compile_src("param speed: float(0.0, 10.0) = 1.0\nlet x = t * speed\nrgb(x, x, x)");
        assert!(compiled.ops.contains(&Op::PushParam(0)));
        assert_eq!(compiled.params.len(), 1);
        assert_eq!(compiled.params[0].name, "speed");
    }

    #[test]
    fn color_literal() {
        let compiled = compile_src("#ff0000");
        // Should push r=1.0, g=0.0, b=0.0, then Rgb
        assert!(compiled.ops.contains(&Op::Rgb));
        assert!(compiled.constants.iter().any(|&c| (c - 1.0).abs() < f64::EPSILON));
    }

    #[test]
    fn local_count_tracks_lets() {
        let compiled = compile_src("let a = 1.0\nlet b = 2.0\nlet c = a + b\nrgb(c, c, c)");
        assert!(compiled.local_count >= 3);
    }

    #[test]
    fn pi_and_tau_as_constants() {
        let compiled = compile_src("let x = sin(t * PI)\nrgb(x, x, x)");
        // PI should be in the constant pool
        assert!(compiled.constants.iter().any(|&c| (c - std::f64::consts::PI).abs() < f64::EPSILON));
    }

    #[test]
    fn hsv_function() {
        let compiled = compile_src("hsv(t * 360.0, 1.0, 1.0)");
        assert!(compiled.ops.contains(&Op::Hsv));
        assert!(compiled.ops.contains(&Op::PushT));
    }

    #[test]
    fn enum_comparison_bytecode() {
        let compiled = compile_src("enum Mode { A, B }\nparam mode: Mode = A\nif mode == Mode.A {\nrgb(1.0, 0.0, 0.0)\n} else {\nrgb(0.0, 1.0, 0.0)\n}");
        // Enum comparison: PushParam(mode) + PushConst(variant_index) + Eq
        assert!(compiled.ops.contains(&Op::PushParam(0)));
        assert!(compiled.ops.contains(&Op::Eq));
    }
}
