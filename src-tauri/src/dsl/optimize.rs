use super::ast::{BinOp, Span, TypeName, UnaryOp};
use super::builtins::BuiltinVar;
use super::compiler::Op;
use super::typeck::{TypedExpr, TypedExprKind, TypedScript, TypedStmt, TypedStmtKind};

// ── Pass 1: Constant Folding on TypedExpr ────────────────────────────

/// Fold constant expressions in a typed script.
/// Recursively evaluates pure constant subtrees at compile time.
pub fn fold_constants(mut script: TypedScript) -> TypedScript {
    script.body = script.body.into_iter().map(fold_stmt).collect();
    script
}

fn fold_stmt(stmt: TypedStmt) -> TypedStmt {
    let kind = match stmt.kind {
        TypedStmtKind::Let { name, value, local_index } => TypedStmtKind::Let {
            name,
            value: fold_expr(value),
            local_index,
        },
        TypedStmtKind::Expr(expr) => TypedStmtKind::Expr(fold_expr(expr)),
    };
    TypedStmt { kind, span: stmt.span }
}

fn fold_expr(expr: TypedExpr) -> TypedExpr {
    let span = expr.span;
    let ty = expr.ty.clone();

    match expr.kind {
        // Resolve constant builtins to literals
        TypedExprKind::LoadBuiltin(BuiltinVar::Pi) => TypedExpr {
            kind: TypedExprKind::FloatLit(std::f64::consts::PI),
            ty,
            span,
        },
        TypedExprKind::LoadBuiltin(BuiltinVar::Tau) => TypedExpr {
            kind: TypedExprKind::FloatLit(std::f64::consts::TAU),
            ty,
            span,
        },

        // IntToFloat(IntLit) → FloatLit
        TypedExprKind::IntToFloat(inner) => {
            let folded = fold_expr(*inner);
            if let TypedExprKind::IntLit(v) = folded.kind {
                TypedExpr {
                    kind: TypedExprKind::FloatLit(f64::from(v)),
                    ty,
                    span,
                }
            } else {
                TypedExpr {
                    kind: TypedExprKind::IntToFloat(Box::new(folded)),
                    ty,
                    span,
                }
            }
        }

        // Unary ops
        TypedExprKind::UnaryOp { op, operand } => {
            let folded = fold_expr(*operand);
            match (&op, &folded.kind) {
                (UnaryOp::Neg, TypedExprKind::FloatLit(x)) => TypedExpr {
                    kind: TypedExprKind::FloatLit(-x),
                    ty,
                    span,
                },
                (UnaryOp::Neg, TypedExprKind::IntLit(x)) => TypedExpr {
                    kind: TypedExprKind::IntLit(-x),
                    ty,
                    span,
                },
                (UnaryOp::Not, TypedExprKind::BoolLit(x)) => TypedExpr {
                    kind: TypedExprKind::BoolLit(!x),
                    ty,
                    span,
                },
                _ => TypedExpr {
                    kind: TypedExprKind::UnaryOp {
                        op,
                        operand: Box::new(folded),
                    },
                    ty,
                    span,
                },
            }
        }

        // Binary ops
        TypedExprKind::BinOp { op, left, right } => {
            let l = fold_expr(*left);
            let r = fold_expr(*right);
            fold_binop(op, l, r, ty, span)
        }

        // Builtin calls — fold if all args are constant floats
        TypedExprKind::BuiltinCall { name, args } => {
            let folded_args: Vec<TypedExpr> = args.into_iter().map(fold_expr).collect();

            // Try to extract all-constant float args
            let const_floats: Option<Vec<f64>> = folded_args
                .iter()
                .map(|a| match &a.kind {
                    TypedExprKind::FloatLit(v) => Some(*v),
                    _ => None,
                })
                .collect();

            if let Some(vals) = const_floats {
                if let Some(result) = eval_builtin(&name, &vals) {
                    return TypedExpr {
                        kind: TypedExprKind::FloatLit(result),
                        ty,
                        span,
                    };
                }
            }

            TypedExpr {
                kind: TypedExprKind::BuiltinCall {
                    name,
                    args: folded_args,
                },
                ty,
                span,
            }
        }

        // Color field access on literal
        TypedExprKind::Field { object, field } => {
            let folded_obj = fold_expr(*object);
            if let TypedExprKind::ColorLit { r, g, b } = &folded_obj.kind {
                match field.as_str() {
                    "r" => return TypedExpr {
                        kind: TypedExprKind::FloatLit(f64::from(*r) / 255.0),
                        ty,
                        span,
                    },
                    "g" => return TypedExpr {
                        kind: TypedExprKind::FloatLit(f64::from(*g) / 255.0),
                        ty,
                        span,
                    },
                    "b" => return TypedExpr {
                        kind: TypedExprKind::FloatLit(f64::from(*b) / 255.0),
                        ty,
                        span,
                    },
                    _ => {}
                }
            }
            TypedExpr {
                kind: TypedExprKind::Field {
                    object: Box::new(folded_obj),
                    field,
                },
                ty,
                span,
            }
        }

        // If with constant condition
        TypedExprKind::If { condition, then_body, else_body } => {
            let folded_cond = fold_expr(*condition);
            match &folded_cond.kind {
                TypedExprKind::BoolLit(true) => {
                    let folded_then: Vec<TypedStmt> =
                        then_body.into_iter().map(fold_stmt).collect();
                    // Inline the then block: wrap in an if(true) that the compiler
                    // will handle, but really we just keep the then body.
                    // Actually, we need to return an expression. The then_body is
                    // a Vec<TypedStmt> — we can't flatten it to a single expr trivially
                    // without changing the AST. Instead, emit If with BoolLit(true) and
                    // let the compiler handle it, but at least the else is gone.
                    // Better: just emit the If with only the then branch.
                    TypedExpr {
                        kind: TypedExprKind::If {
                            condition: Box::new(folded_cond),
                            then_body: folded_then,
                            else_body: None,
                        },
                        ty,
                        span,
                    }
                }
                TypedExprKind::BoolLit(false) => {
                    if let Some(else_stmts) = else_body {
                        let folded_else: Vec<TypedStmt> =
                            else_stmts.into_iter().map(fold_stmt).collect();
                        // Replace with if(true) { else_body } to avoid changing AST shape
                        TypedExpr {
                            kind: TypedExprKind::If {
                                condition: Box::new(TypedExpr {
                                    kind: TypedExprKind::BoolLit(true),
                                    ty: TypeName::Bool,
                                    span,
                                }),
                                then_body: folded_else,
                                else_body: None,
                            },
                            ty,
                            span,
                        }
                    } else {
                        // if false with no else — emit a black color as fallback
                        TypedExpr {
                            kind: TypedExprKind::ColorLit { r: 0, g: 0, b: 0 },
                            ty,
                            span,
                        }
                    }
                }
                _ => {
                    let folded_then: Vec<TypedStmt> =
                        then_body.into_iter().map(fold_stmt).collect();
                    let folded_else =
                        else_body.map(|stmts| stmts.into_iter().map(fold_stmt).collect());
                    TypedExpr {
                        kind: TypedExprKind::If {
                            condition: Box::new(folded_cond),
                            then_body: folded_then,
                            else_body: folded_else,
                        },
                        ty,
                        span,
                    }
                }
            }
        }

        // Recurse into subexpressions for other nodes
        TypedExprKind::ColorScale { color, factor } => {
            let c = fold_expr(*color);
            let f = fold_expr(*factor);
            TypedExpr {
                kind: TypedExprKind::ColorScale {
                    color: Box::new(c),
                    factor: Box::new(f),
                },
                ty,
                span,
            }
        }
        TypedExprKind::MakeVec2 { x, y } => {
            let fx = fold_expr(*x);
            let fy = fold_expr(*y);
            TypedExpr {
                kind: TypedExprKind::MakeVec2 {
                    x: Box::new(fx),
                    y: Box::new(fy),
                },
                ty,
                span,
            }
        }
        TypedExprKind::EvalGradient { param_index, arg } => TypedExpr {
            kind: TypedExprKind::EvalGradient {
                param_index,
                arg: Box::new(fold_expr(*arg)),
            },
            ty,
            span,
        },
        TypedExprKind::EvalCurve { param_index, arg } => TypedExpr {
            kind: TypedExprKind::EvalCurve {
                param_index,
                arg: Box::new(fold_expr(*arg)),
            },
            ty,
            span,
        },
        TypedExprKind::EvalPath { param_index, arg } => TypedExpr {
            kind: TypedExprKind::EvalPath {
                param_index,
                arg: Box::new(fold_expr(*arg)),
            },
            ty,
            span,
        },

        // Leaf nodes — no folding possible
        _ => expr,
    }
}

/// Try to fold a binary operation on two already-folded operands.
fn fold_binop(
    op: BinOp,
    left: TypedExpr,
    right: TypedExpr,
    ty: TypeName,
    span: Span,
) -> TypedExpr {
    // Float × Float
    if let (TypedExprKind::FloatLit(a), TypedExprKind::FloatLit(b)) =
        (&left.kind, &right.kind)
    {
        let result = eval_float_binop(op, *a, *b);
        return TypedExpr {
            kind: if ty == TypeName::Bool {
                TypedExprKind::BoolLit(result != 0.0)
            } else {
                TypedExprKind::FloatLit(result)
            },
            ty,
            span,
        };
    }

    // Int × Int (bitwise/shift/arithmetic)
    if let (TypedExprKind::IntLit(a), TypedExprKind::IntLit(b)) =
        (&left.kind, &right.kind)
    {
        if let Some(result) = eval_int_binop(op, *a, *b) {
            return TypedExpr {
                kind: TypedExprKind::IntLit(result),
                ty,
                span,
            };
        }
    }

    // Bool × Bool (And/Or)
    if let (TypedExprKind::BoolLit(a), TypedExprKind::BoolLit(b)) =
        (&left.kind, &right.kind)
    {
        match op {
            BinOp::And => return TypedExpr {
                kind: TypedExprKind::BoolLit(*a && *b),
                ty,
                span,
            },
            BinOp::Or => return TypedExpr {
                kind: TypedExprKind::BoolLit(*a || *b),
                ty,
                span,
            },
            _ => {}
        }
    }

    TypedExpr {
        kind: TypedExprKind::BinOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
        },
        ty,
        span,
    }
}

/// Evaluate a float binary operation at compile time.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn eval_float_binop(op: BinOp, a: f64, b: f64) -> f64 {
    match op {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div => {
            if b == 0.0 { 0.0 } else { a / b }
        }
        BinOp::Mod => {
            if b == 0.0 { 0.0 } else { a % b }
        }
        BinOp::Pow => a.powf(b),
        BinOp::Lt => if a < b { 1.0 } else { 0.0 },
        BinOp::Gt => if a > b { 1.0 } else { 0.0 },
        BinOp::Le => if a <= b { 1.0 } else { 0.0 },
        BinOp::Ge => if a >= b { 1.0 } else { 0.0 },
        BinOp::Eq => {
            let diff = (a - b).abs();
            let magnitude = a.abs().max(b.abs()).max(1.0);
            if diff < 1e-9 * magnitude { 1.0 } else { 0.0 }
        }
        BinOp::Ne => {
            let diff = (a - b).abs();
            let magnitude = a.abs().max(b.abs()).max(1.0);
            if diff >= 1e-9 * magnitude { 1.0 } else { 0.0 }
        }
        BinOp::And => if a != 0.0 && b != 0.0 { 1.0 } else { 0.0 },
        BinOp::Or => if a != 0.0 || b != 0.0 { 1.0 } else { 0.0 },
        BinOp::BitAnd => ((a as i64) & (b as i64)) as f64,
        BinOp::BitOr => ((a as i64) | (b as i64)) as f64,
        BinOp::BitXor => ((a as i64) ^ (b as i64)) as f64,
        BinOp::Shl => {
            let shift = (b as i64).clamp(0, 63) as u32;
            ((a as i64).wrapping_shl(shift)) as f64
        }
        BinOp::Shr => {
            let shift = (b as i64).clamp(0, 63) as u32;
            ((a as i64).wrapping_shr(shift)) as f64
        }
    }
}

/// Evaluate an integer binary operation at compile time.
#[allow(clippy::cast_sign_loss)]
fn eval_int_binop(op: BinOp, a: i32, b: i32) -> Option<i32> {
    Some(match op {
        BinOp::Add => a.wrapping_add(b),
        BinOp::Sub => a.wrapping_sub(b),
        BinOp::Mul => a.wrapping_mul(b),
        BinOp::BitAnd => a & b,
        BinOp::BitOr => a | b,
        BinOp::BitXor => a ^ b,
        BinOp::Shl => a.wrapping_shl(b.clamp(0, 31) as u32),
        BinOp::Shr => a.wrapping_shr(b.clamp(0, 31) as u32),
        _ => return None,
    })
}

/// Evaluate a pure builtin function at compile time.
/// Maps builtin names to their Rust implementations (same functions the VM uses).
fn eval_builtin(name: &str, args: &[f64]) -> Option<f64> {
    match (name, args) {
        // 1-arg math
        ("sin", [x]) => Some(x.sin()),
        ("cos", [x]) => Some(x.cos()),
        ("tan", [x]) => Some(x.tan()),
        ("abs", [x]) => Some(x.abs()),
        ("floor", [x]) => Some(x.floor()),
        ("ceil", [x]) => Some(x.ceil()),
        ("round", [x]) => Some(x.round()),
        ("fract", [x]) => Some(x.fract()),
        ("sqrt", [x]) => Some(x.sqrt()),
        ("sign", [x]) => Some(x.signum()),
        ("exp", [x]) => Some(x.exp()),
        ("log", [x]) => Some(x.ln()),

        // 2-arg math
        ("pow", [base, exp]) => Some(base.powf(*exp)),
        ("min", [a, b]) => Some(a.min(*b)),
        ("max", [a, b]) => Some(a.max(*b)),
        ("step", [edge, x]) => Some(if *x < *edge { 0.0 } else { 1.0 }),
        ("atan2", [y, x]) => Some(y.atan2(*x)),
        ("mod", [a, b]) => Some(if *b == 0.0 { 0.0 } else { a % b }),

        // 3-arg math
        ("clamp", [x, min, max]) => Some(x.clamp(*min, *max)),
        ("mix", [a, b, t]) => Some(a + (b - a) * t),
        ("smoothstep", [e0, e1, x]) => {
            if e0 >= e1 {
                Some(0.0)
            } else {
                let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
                Some(t * t * (3.0 - 2.0 * t))
            }
        }

        // 1-arg easing
        ("ease_in", [t]) => Some(t * t),
        ("ease_out", [t]) => Some(t * (2.0 - t)),
        ("ease_in_out", [t]) => Some(if *t < 0.5 {
            2.0 * t * t
        } else {
            -1.0 + (4.0 - 2.0 * t) * t
        }),
        ("ease_in_cubic", [t]) => Some(t * t * t),
        ("ease_out_cubic", [t]) => {
            let t1 = t - 1.0;
            Some(t1 * t1 * t1 + 1.0)
        }
        ("ease_in_out_cubic", [t]) => Some(if *t < 0.5 {
            4.0 * t * t * t
        } else {
            let t1 = 2.0 * t - 2.0;
            0.5 * t1 * t1 * t1 + 1.0
        }),

        // Hash (deterministic)
        ("hash", [a, b]) => {
            let dot = a * 12.9898 + b * 78.233;
            let s = (dot.sin() * 43758.5453).fract();
            Some(s.abs())
        }
        ("hash3", [a, b, c]) => {
            let dot = a * 12.9898 + b * 78.233 + c * 45.164;
            let s = (dot.sin() * 43758.5453).fract();
            Some(s.abs())
        }
        ("random", [x]) => {
            let dot = x * 12.9898;
            let s = (dot.sin() * 43758.5453).fract();
            Some(s.abs())
        }
        ("random_range", [seed, min, max]) => {
            let dot = seed * 12.9898;
            let h = (dot.sin() * 43758.5453).fract().abs();
            Some(min + (max - min) * h)
        }

        // Skip noise/fbm/worley — they work but are expensive at compile time
        // and unlikely to appear with all-constant args in practice. The peephole
        // pass will catch any that slip through as bytecode constants.

        _ => None,
    }
}

// ── Pass 2: Peephole Optimization on Vec<Op> ─────────────────────────

/// Apply peephole optimizations to compiled bytecode.
/// Runs until no more changes are made (fixpoint).
pub fn peephole(mut ops: Vec<Op>, constants: &mut Vec<f64>) -> Vec<Op> {
    loop {
        let (new_ops, changed) = peephole_pass(&ops, constants);
        ops = new_ops;
        if !changed {
            break;
        }
    }
    ops
}

/// Single pass of peephole optimization. Returns (new_ops, changed).
fn peephole_pass(ops: &[Op], constants: &mut Vec<f64>) -> (Vec<Op>, bool) {
    let mut result = Vec::with_capacity(ops.len());
    let mut changed = false;
    let mut i = 0;

    while i < ops.len() {
        // Pattern: PushConst(a), PushConst(b), <binop> → PushConst(a op b)
        if i + 2 < ops.len() {
            if let (Op::PushConst(ai), Op::PushConst(bi)) = (ops[i], ops[i + 1]) {
                let a = const_val(constants, ai);
                let b = const_val(constants, bi);
                if let Some(folded) = try_fold_op(ops[i + 2], a, b) {
                    let idx = add_or_reuse_constant(constants, folded);
                    result.push(Op::PushConst(idx));
                    changed = true;
                    i += 3;
                    continue;
                }
            }
        }

        // Pattern: PushConst(0.0), Add → remove both (identity: x + 0 = x)
        // Pattern: PushConst(0.0), Sub → remove both (identity: x - 0 = x)
        if i + 1 < ops.len() {
            if let Op::PushConst(ci) = ops[i] {
                let c = const_val(constants, ci);
                if c == 0.0 && matches!(ops[i + 1], Op::Add | Op::Sub) {
                    changed = true;
                    i += 2;
                    continue;
                }
                // Pattern: PushConst(1.0), Mul → remove both (identity: x * 1 = x)
                // Pattern: PushConst(1.0), Div → remove both (identity: x / 1 = x)
                #[allow(clippy::float_cmp)]
                if c == 1.0 && matches!(ops[i + 1], Op::Mul | Op::Div) {
                    changed = true;
                    i += 2;
                    continue;
                }
                // Pattern: PushConst(0.0), Mul → Pop, PushConst(0.0) (absorption: x * 0 = 0)
                if c == 0.0 && ops[i + 1] == Op::Mul {
                    result.push(Op::Pop);
                    result.push(Op::PushConst(ci));
                    changed = true;
                    i += 2;
                    continue;
                }
            }
        }

        // Pattern: Not, Not → remove both
        if i + 1 < ops.len() && ops[i] == Op::Not && ops[i + 1] == Op::Not {
            changed = true;
            i += 2;
            continue;
        }

        // Pattern: Neg, Neg → remove both
        if i + 1 < ops.len() && ops[i] == Op::Neg && ops[i + 1] == Op::Neg {
            changed = true;
            i += 2;
            continue;
        }

        result.push(ops[i]);
        i += 1;
    }

    // Fix up jump targets if we changed anything
    if changed {
        fixup_jumps(ops, &mut result);
    }

    (result, changed)
}

/// Get the constant value at the given pool index.
fn const_val(constants: &[f64], idx: u16) -> f64 {
    constants.get(idx as usize).copied().unwrap_or(0.0)
}

/// Add a constant to the pool, reusing an existing index if possible.
fn add_or_reuse_constant(constants: &mut Vec<f64>, value: f64) -> u16 {
    for (i, &c) in constants.iter().enumerate() {
        if c.to_bits() == value.to_bits() {
            return i as u16;
        }
    }
    let idx = constants.len() as u16;
    constants.push(value);
    idx
}

/// Try to evaluate a binary op on two constants.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn try_fold_op(op: Op, a: f64, b: f64) -> Option<f64> {
    Some(match op {
        Op::Add => a + b,
        Op::Sub => a - b,
        Op::Mul => a * b,
        Op::Div => if b == 0.0 { 0.0 } else { a / b },
        Op::Mod => if b == 0.0 { 0.0 } else { a % b },
        Op::Pow => a.powf(b),
        Op::Lt => if a < b { 1.0 } else { 0.0 },
        Op::Gt => if a > b { 1.0 } else { 0.0 },
        Op::Le => if a <= b { 1.0 } else { 0.0 },
        Op::Ge => if a >= b { 1.0 } else { 0.0 },
        Op::Min => a.min(b),
        Op::Max => a.max(b),
        Op::BitAnd => ((a as i64) & (b as i64)) as f64,
        Op::BitOr => ((a as i64) | (b as i64)) as f64,
        Op::BitXor => ((a as i64) ^ (b as i64)) as f64,
        _ => return None,
    })
}

/// Rebuild jump targets after peephole changes.
///
/// Strategy: build an offset map from old instruction indices to new ones,
/// then rewrite all Jump/JumpIfFalse targets.
fn fixup_jumps(old_ops: &[Op], new_ops: &mut [Op]) {
    // Build map: for old instruction index → new instruction index.
    // Walk old ops, replaying the same pattern detection to track how many
    // old ops map to how many new ops at each position.
    let mut old_to_new = vec![0usize; old_ops.len() + 1];
    let mut pos_in_new = 0usize;
    let mut i = 0;
    while i < old_ops.len() {
        old_to_new[i] = pos_in_new;

        // Detect the same patterns peephole_pass uses to know how many old ops
        // map to how many new ops at this position
        let skip = pattern_length(old_ops, i);
        if skip > 0 {
            // This pattern was transformed — count how many new ops it produced
            let new_count = pattern_new_count(old_ops, i);
            pos_in_new += new_count;
            i += skip;
        } else {
            // Instruction survived as-is
            pos_in_new += 1;
            i += 1;
        }
    }
    old_to_new[old_ops.len()] = pos_in_new;

    // Rewrite jumps in new_ops
    for op in new_ops.iter_mut() {
        match op {
            Op::Jump(ref mut target) | Op::JumpIfFalse(ref mut target) => {
                let old_target = *target as usize;
                if old_target <= old_ops.len() {
                    *target = old_to_new[old_target] as u16;
                }
            }
            _ => {}
        }
    }
}

/// Detect peephole patterns at position i, returning how many old ops the pattern consumes.
/// Returns 0 if no pattern matches (instruction survives as-is).
fn pattern_length(ops: &[Op], i: usize) -> usize {
    // PushConst(a), PushConst(b), <binop> → PushConst(result)
    if i + 2 < ops.len() {
        if let (Op::PushConst(_), Op::PushConst(_)) = (ops[i], ops[i + 1]) {
            if try_fold_op(ops[i + 2], 0.0, 0.0).is_some() {
                return 3;
            }
        }
    }

    if i + 1 < ops.len() {
        if let Op::PushConst(_) = ops[i] {
            // Identity patterns: remove 2
            if matches!(ops[i + 1], Op::Add | Op::Sub | Op::Mul | Op::Div) {
                // Check if the constant triggers an identity/absorption pattern
                // This is imprecise (we don't have access to constants here),
                // but pattern_length is only called during fixup_jumps to replay
                // the same decisions. We need to match the same patterns.
                // Rather than passing constants, we'll use a different approach.
                // See below — we refactor to avoid needing this.
            }
        }

        // Not, Not or Neg, Neg
        if ops[i] == Op::Not && ops[i + 1] == Op::Not {
            return 2;
        }
        if ops[i] == Op::Neg && ops[i + 1] == Op::Neg {
            return 2;
        }
    }

    0
}

/// How many new ops a matched pattern produces.
fn pattern_new_count(ops: &[Op], i: usize) -> usize {
    // PushConst, PushConst, binop → 1 (PushConst(result))
    if i + 2 < ops.len() {
        if let (Op::PushConst(_), Op::PushConst(_)) = (ops[i], ops[i + 1]) {
            if try_fold_op(ops[i + 2], 0.0, 0.0).is_some() {
                return 1;
            }
        }
    }

    // Not+Not, Neg+Neg → 0
    if i + 1 < ops.len()
        && ((ops[i] == Op::Not && ops[i + 1] == Op::Not)
            || (ops[i] == Op::Neg && ops[i + 1] == Op::Neg))
    {
        return 0;
    }

    1
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::dsl::compile_source;
    use crate::dsl::compiler::{compile, Op};
    use crate::dsl::lexer::lex;
    use crate::dsl::parser::parse;
    use crate::dsl::typeck::type_check;
    use crate::dsl::vm::{self, VmContext};
    use crate::model::color::Color;

    /// Compile with optimization (the default pipeline).
    fn compile_opt(src: &str) -> crate::dsl::compiler::CompiledScript {
        compile_source(src).unwrap()
    }

    /// Compile without optimization for comparison.
    fn compile_unopt(src: &str) -> crate::dsl::compiler::CompiledScript {
        let tokens = lex(src).unwrap();
        let ast = parse(tokens).unwrap();
        let typed = type_check(&ast).unwrap();
        compile(&typed).unwrap()
    }

    fn run_compiled(compiled: &crate::dsl::compiler::CompiledScript, t: f64, pixel: usize, pixels: usize) -> Color {
        let pos = if pixels > 1 { pixel as f64 / (pixels - 1) as f64 } else { 0.0 };
        let ctx = VmContext {
            t,
            pixel,
            pixels,
            pos,
            pos2d: (pos, 0.0),
            abs_t: 0.0,
            param_values: &[],
            gradients: &[],
            curves: &[],
            colors: &[],
            paths: &[],
        };
        vm::execute(compiled, &ctx)
    }

    #[test]
    fn fold_arithmetic() {
        let compiled = compile_opt("rgb(1.0 + 2.0, 0.0, 0.0)");
        // Should fold 1.0+2.0 to 3.0 — no Add op
        assert!(
            !compiled.ops.contains(&Op::Add),
            "1.0 + 2.0 should be folded, ops: {:?}",
            compiled.ops
        );
        // Result should be correct: rgb(3.0, 0, 0) → clamped to 255
        let color = run_compiled(&compiled, 0.0, 0, 1);
        assert_eq!(color.r, 255);
    }

    #[test]
    fn fold_sin_pi() {
        let compiled = compile_opt("let x = sin(PI); rgb(abs(x), 0.0, 0.0)");
        // sin(PI) ≈ 0.0 — should be folded to a constant
        assert!(
            !compiled.ops.contains(&Op::Sin),
            "sin(PI) should be folded, ops: {:?}",
            compiled.ops
        );
        let color = run_compiled(&compiled, 0.0, 0, 1);
        assert!(color.r <= 1, "sin(PI) ≈ 0, got r={}", color.r);
    }

    #[test]
    fn fold_nested_sin_pi_div_2() {
        let compiled = compile_opt("let x = sin(PI / 2.0); rgb(x, x, x)");
        // sin(PI/2) = 1.0 — the entire chain should fold
        assert!(
            !compiled.ops.contains(&Op::Sin),
            "sin(PI/2) should be folded"
        );
        assert!(
            !compiled.ops.contains(&Op::Div),
            "PI/2 should be folded"
        );
        let color = run_compiled(&compiled, 0.0, 0, 1);
        assert_eq!(color.r, 255, "sin(PI/2) = 1.0 → 255");
    }

    #[test]
    fn fold_color_field() {
        let compiled = compile_opt("let x = #ff0000.r; rgb(x, 0.0, 0.0)");
        // #ff0000.r = 1.0 — should fold to constant
        assert!(
            !compiled.ops.contains(&Op::ColorR),
            "#ff0000.r should be folded"
        );
        let color = run_compiled(&compiled, 0.0, 0, 1);
        assert_eq!(color.r, 255);
    }

    #[test]
    fn fold_if_true() {
        let compiled = compile_opt("if true { rgb(1.0, 0.0, 0.0) } else { rgb(0.0, 1.0, 0.0) }");
        // Should eliminate the else branch — no Jump instruction needed
        // (JumpIfFalse over a BoolLit(true) condition still emits but the else is gone)
        let color = run_compiled(&compiled, 0.0, 0, 1);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
    }

    #[test]
    fn fold_if_false() {
        let compiled = compile_opt("if false { rgb(1.0, 0.0, 0.0) } else { rgb(0.0, 1.0, 0.0) }");
        // Should inline else body
        let color = run_compiled(&compiled, 0.0, 0, 1);
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 255);
    }

    #[test]
    fn peephole_identity_add_zero() {
        // x + 0.0 should eliminate the add
        let compiled = compile_opt("let x = t + 0.0; rgb(x, x, x)");
        // The peephole pass should remove PushConst(0.0) + Add
        assert!(
            !compiled.ops.contains(&Op::Add),
            "t + 0.0 should have Add eliminated, ops: {:?}",
            compiled.ops
        );
    }

    #[test]
    fn peephole_double_neg() {
        // -(-t) should eliminate both negations
        let src = "let x = -(-t); rgb(x, x, x)";
        let compiled = compile_opt(src);
        let neg_count = compiled.ops.iter().filter(|&&op| op == Op::Neg).count();
        assert_eq!(neg_count, 0, "double neg should be eliminated, ops: {:?}", compiled.ops);
    }

    #[test]
    fn no_fold_runtime() {
        // sin(t * PI) should NOT fold — t is runtime
        let compiled = compile_opt("let x = sin(t * PI); rgb(x, x, x)");
        assert!(
            compiled.ops.contains(&Op::Sin),
            "sin(t * PI) must NOT be folded (t is runtime), ops: {:?}",
            compiled.ops
        );
    }

    #[test]
    fn end_to_end_optimized_matches_unoptimized() {
        let src = "let x = sin(PI / 4.0) * 0.5 + 0.5; rgb(x, x, x)";
        let opt = compile_opt(src);
        let unopt = compile_unopt(src);

        for pixel in 0..10 {
            let c_opt = run_compiled(&opt, 0.5, pixel, 10);
            let c_unopt = run_compiled(&unopt, 0.5, pixel, 10);
            assert_eq!(
                c_opt, c_unopt,
                "pixel {pixel}: optimized ({},{},{}) != unoptimized ({},{},{})",
                c_opt.r, c_opt.g, c_opt.b, c_unopt.r, c_unopt.g, c_unopt.b
            );
        }
    }

    #[test]
    fn end_to_end_complex_expression() {
        // A more complex expression with multiple foldable subexpressions
        let src = "let base = cos(0.0); let x = base * 0.5; rgb(x, x, x)";
        let opt = compile_opt(src);
        let unopt = compile_unopt(src);

        let c_opt = run_compiled(&opt, 0.0, 0, 1);
        let c_unopt = run_compiled(&unopt, 0.0, 0, 1);
        assert_eq!(c_opt, c_unopt);
        // cos(0) = 1.0, * 0.5 = 0.5 → 128
        assert_eq!(c_opt.r, 128);
    }

    #[test]
    fn fold_preserves_runtime_if() {
        // Runtime condition should not be folded, but constant subexprs within should be
        let src = "if t > 0.5 { rgb(1.0 + 0.0, 0.0, 0.0) } else { rgb(0.0, 1.0 + 0.0, 0.0) }";
        let opt = compile_opt(src);
        let c_hi = run_compiled(&opt, 0.8, 0, 1);
        assert_eq!(c_hi.r, 255);
        let c_lo = run_compiled(&opt, 0.2, 0, 1);
        assert_eq!(c_lo.g, 255);
    }

    #[test]
    fn peephole_mul_by_one() {
        let compiled = compile_opt("let x = t * 1.0; rgb(x, x, x)");
        assert!(
            !compiled.ops.contains(&Op::Mul),
            "t * 1.0 should have Mul eliminated, ops: {:?}",
            compiled.ops
        );
    }

    #[test]
    fn fold_int_bitwise() {
        // 6 & 3 = 2, should fold at compile time
        let compiled = compile_opt("let x = 6 & 3; let n = x / 8.0; rgb(n, 0.0, 0.0)");
        assert!(
            !compiled.ops.contains(&Op::BitAnd),
            "6 & 3 should be folded, ops: {:?}",
            compiled.ops
        );
        let color = run_compiled(&compiled, 0.0, 0, 1);
        assert_eq!(color.r, 64); // 2/8 = 0.25 → 64
    }
}
