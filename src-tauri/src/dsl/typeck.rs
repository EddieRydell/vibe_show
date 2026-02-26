use std::collections::HashMap;

use super::ast::*;
use super::builtins::{self, BuiltinVar};
use super::error::CompileError;

/// Result of type checking: a validated script with type info and resolved names.
#[derive(Debug, Clone)]
pub struct TypedScript {
    pub name: String,
    pub spatial: bool,
    pub params: Vec<TypedParam>,
    pub enums: Vec<TypeDef>,
    pub flags: Vec<TypeDef>,
    pub body: Vec<TypedStmt>,
}

#[derive(Debug, Clone)]
pub struct TypedParam {
    pub name: String,
    pub ty: ParamType,
    pub default: Expr,
    pub index: u16,
}

#[derive(Debug, Clone)]
pub struct TypedStmt {
    pub kind: TypedStmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypedStmtKind {
    Let {
        name: String,
        value: TypedExpr,
        local_index: u16,
    },
    Expr(TypedExpr),
}

#[derive(Debug, Clone)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: TypeName,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypedExprKind {
    FloatLit(f64),
    IntLit(i32),
    BoolLit(bool),
    ColorLit { r: u8, g: u8, b: u8 },
    /// Load a local variable by index.
    LoadLocal(u16),
    /// Load a param by index.
    LoadParam(u16),
    /// Load an implicit builtin variable (t, pixel, pixels, pos, pos2d, PI, TAU).
    LoadBuiltin(BuiltinVar),
    BinOp {
        op: BinOp,
        left: Box<TypedExpr>,
        right: Box<TypedExpr>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<TypedExpr>,
    },
    /// Built-in function call.
    BuiltinCall {
        name: String,
        args: Vec<TypedExpr>,
    },
    /// Evaluate a gradient param at t.
    EvalGradient {
        param_index: u16,
        arg: Box<TypedExpr>,
    },
    /// Evaluate a curve param at t.
    EvalCurve {
        param_index: u16,
        arg: Box<TypedExpr>,
    },
    /// Evaluate a motion path param at explicit time → Vec2.
    EvalPath {
        param_index: u16,
        arg: Box<TypedExpr>,
    },
    /// Evaluate a motion path param at abs_t (implicit) → Vec2.
    EvalPathAtT {
        param_index: u16,
    },
    /// color.scale(f)
    ColorScale {
        color: Box<TypedExpr>,
        factor: Box<TypedExpr>,
    },
    /// Field access: .r, .g, .b, .a, .x, .y
    Field {
        object: Box<TypedExpr>,
        field: String,
    },
    /// MakeVec2(x, y)
    MakeVec2 {
        x: Box<TypedExpr>,
        y: Box<TypedExpr>,
    },
    If {
        condition: Box<TypedExpr>,
        then_body: Vec<TypedStmt>,
        else_body: Option<Vec<TypedStmt>>,
    },
    /// Compare enum param value to variant index.
    EnumEq {
        param_index: u16,
        variant_index: u16,
    },
    /// Test a flag bit on a flags param.
    FlagTest {
        param_index: u16,
        bit_mask: u32,
    },
    /// Load a color param by index.
    LoadColor(u16),
    /// Int to float conversion.
    IntToFloat(Box<TypedExpr>),
}

pub fn type_check(script: &Script) -> Result<TypedScript, Vec<CompileError>> {
    let mut ctx = TypeContext::new();
    ctx.check(script)
}

/// Maximum function inlining depth to prevent stack overflow from recursive functions.
const MAX_INLINE_DEPTH: u16 = 16;

struct TypeContext {
    /// Local variables: name → (type, local_index)
    locals: Vec<HashMap<String, (TypeName, u16)>>,
    next_local: u16,
    /// Param definitions: name → (type, param_index)
    params: HashMap<String, (ParamType, u16)>,
    /// User-defined functions: name → FnDef
    functions: HashMap<String, FnDef>,
    /// Enum definitions: name → variants
    enums: HashMap<String, Vec<String>>,
    /// Flags definitions: name → variants
    flags: HashMap<String, Vec<String>>,
    /// Errors accumulated
    errors: Vec<CompileError>,
    /// Current function inlining depth (guards against recursive functions)
    inline_depth: u16,
}

impl TypeContext {
    fn new() -> Self {
        Self {
            locals: vec![HashMap::new()],
            next_local: 0,
            params: HashMap::new(),
            functions: HashMap::new(),
            enums: HashMap::new(),
            flags: HashMap::new(),
            errors: Vec::new(),
            inline_depth: 0,
        }
    }

    fn check(&mut self, script: &Script) -> Result<TypedScript, Vec<CompileError>> {
        // Extract metadata
        let name = script.metadata.iter()
            .find(|m| m.key == "name")
            .and_then(|m| if let MetaValue::Str(s) = &m.value { Some(s.clone()) } else { None })
            .unwrap_or_default();
        let spatial = script.metadata.iter()
            .find(|m| m.key == "spatial")
            .and_then(|m| if let MetaValue::Bool(b) = &m.value { Some(*b) } else { None })
            .unwrap_or(false);

        // Register type defs
        for td in &script.type_defs {
            match td.kind {
                TypeDefKind::Enum => { self.enums.insert(td.name.clone(), td.variants.clone()); }
                TypeDefKind::Flags => { self.flags.insert(td.name.clone(), td.variants.clone()); }
            }
        }

        // Register params
        let mut typed_params = Vec::new();
        for (i, p) in script.params.iter().enumerate() {
            let idx = i as u16;
            self.params.insert(p.name.clone(), (p.ty.clone(), idx));
            typed_params.push(TypedParam {
                name: p.name.clone(),
                ty: p.ty.clone(),
                default: p.default.clone(),
                index: idx,
            });
        }

        // Register user-defined functions
        for f in &script.functions {
            self.functions.insert(f.name.clone(), f.clone());
        }

        // Type check body
        let mut typed_body = Vec::new();
        for stmt in &script.body {
            match self.check_stmt(stmt) {
                Ok(ts) => typed_body.push(ts),
                Err(e) => self.errors.push(e),
            }
        }

        // Verify last expression is color type
        if let Some(last) = typed_body.last() {
            let last_ty = match &last.kind {
                TypedStmtKind::Let { .. } => None,
                TypedStmtKind::Expr(e) => Some(&e.ty),
            };
            if let Some(ty) = last_ty {
                if *ty != TypeName::Color {
                    self.errors.push(CompileError::type_error(
                        format!("Script must return color, but last expression has type {ty:?}"),
                        last.span,
                    ));
                }
            }
        } else {
            self.errors.push(CompileError::type_error(
                "Script body is empty — must produce a color value",
                Span::new(0, 0),
            ));
        }

        if self.errors.is_empty() {
            Ok(TypedScript {
                name,
                spatial,
                params: typed_params,
                enums: script.type_defs.iter().filter(|td| td.kind == TypeDefKind::Enum).cloned().collect(),
                flags: script.type_defs.iter().filter(|td| td.kind == TypeDefKind::Flags).cloned().collect(),
                body: typed_body,
            })
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<TypedStmt, CompileError> {
        match stmt {
            Stmt::Let { name, value, span } => {
                let typed_value = self.check_expr(value)?;
                let local_idx = self.next_local;
                self.next_local += 1;
                let ty = typed_value.ty.clone();
                if let Some(scope) = self.locals.last_mut() {
                    scope.insert(name.clone(), (ty, local_idx));
                }
                Ok(TypedStmt {
                    kind: TypedStmtKind::Let {
                        name: name.clone(),
                        value: typed_value,
                        local_index: local_idx,
                    },
                    span: *span,
                })
            }
            Stmt::Expr(expr) => {
                let typed_expr = self.check_expr(expr)?;
                Ok(TypedStmt {
                    kind: TypedStmtKind::Expr(typed_expr),
                    span: expr.span,
                })
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<TypedExpr, CompileError> {
        match &expr.kind {
            ExprKind::FloatLit(v) => Ok(TypedExpr {
                kind: TypedExprKind::FloatLit(*v),
                ty: TypeName::Float,
                span: expr.span,
            }),
            ExprKind::IntLit(v) => Ok(TypedExpr {
                kind: TypedExprKind::IntLit(*v),
                ty: TypeName::Int,
                span: expr.span,
            }),
            ExprKind::BoolLit(v) => Ok(TypedExpr {
                kind: TypedExprKind::BoolLit(*v),
                ty: TypeName::Bool,
                span: expr.span,
            }),
            ExprKind::ColorLit { r, g, b } => Ok(TypedExpr {
                kind: TypedExprKind::ColorLit { r: *r, g: *g, b: *b },
                ty: TypeName::Color,
                span: expr.span,
            }),
            ExprKind::Ident(name) => self.resolve_ident(name, expr.span),

            ExprKind::BinOp { op, left, right } => {
                let mut typed_left = self.check_expr(left)?;
                let mut typed_right = self.check_expr(right)?;

                // Auto-promote int to float
                if typed_left.ty == TypeName::Int && typed_right.ty == TypeName::Float {
                    typed_left = Self::coerce_to_float(typed_left);
                } else if typed_left.ty == TypeName::Float && typed_right.ty == TypeName::Int {
                    typed_right = Self::coerce_to_float(typed_right);
                }

                let result_ty = match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod | BinOp::Pow
                    | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                        if typed_left.ty != TypeName::Float && typed_left.ty != TypeName::Int {
                            return Err(CompileError::type_error(
                                format!("Arithmetic on non-numeric type {:?}", typed_left.ty),
                                expr.span,
                            ));
                        }
                        typed_left.ty.clone()
                    }
                    BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
                    | BinOp::Eq | BinOp::Ne => {
                        TypeName::Bool
                    }
                    BinOp::And | BinOp::Or => {
                        if typed_left.ty != TypeName::Bool {
                            return Err(CompileError::type_error(
                                format!("Logical op on non-bool type {:?}", typed_left.ty),
                                expr.span,
                            ));
                        }
                        TypeName::Bool
                    }
                };

                Ok(TypedExpr {
                    kind: TypedExprKind::BinOp {
                        op: *op,
                        left: Box::new(typed_left),
                        right: Box::new(typed_right),
                    },
                    ty: result_ty,
                    span: expr.span,
                })
            }

            ExprKind::UnaryOp { op, operand } => {
                let typed_operand = self.check_expr(operand)?;
                let result_ty = match op {
                    UnaryOp::Neg => {
                        if typed_operand.ty != TypeName::Float && typed_operand.ty != TypeName::Int {
                            return Err(CompileError::type_error(
                                format!("Negation of non-numeric type {:?}", typed_operand.ty),
                                expr.span,
                            ));
                        }
                        typed_operand.ty.clone()
                    }
                    UnaryOp::Not => {
                        if typed_operand.ty != TypeName::Bool {
                            return Err(CompileError::type_error(
                                format!("Logical NOT of non-bool type {:?}", typed_operand.ty),
                                expr.span,
                            ));
                        }
                        TypeName::Bool
                    }
                };
                Ok(TypedExpr {
                    kind: TypedExprKind::UnaryOp {
                        op: *op,
                        operand: Box::new(typed_operand),
                    },
                    ty: result_ty,
                    span: expr.span,
                })
            }

            ExprKind::Call { name, args } => {
                // Check if it's a user-defined function (inline)
                if let Some(fn_def) = self.functions.get(name).cloned() {
                    return self.inline_function_call(&fn_def, args, expr.span);
                }

                // Check if it's a param eval (gradient/curve)
                if let Some((param_ty, param_idx)) = self.params.get(name).cloned() {
                    match param_ty {
                        ParamType::Gradient => {
                            if args.len() != 1 {
                                return Err(CompileError::type_error(
                                    "Gradient evaluation takes exactly 1 argument",
                                    expr.span,
                                ));
                            }
                            let arg = self.check_expr(&args[0])?;
                            return Ok(TypedExpr {
                                kind: TypedExprKind::EvalGradient {
                                    param_index: param_idx,
                                    arg: Box::new(arg),
                                },
                                ty: TypeName::Color,
                                span: expr.span,
                            });
                        }
                        ParamType::Curve => {
                            if args.len() != 1 {
                                return Err(CompileError::type_error(
                                    "Curve evaluation takes exactly 1 argument",
                                    expr.span,
                                ));
                            }
                            let arg = self.check_expr(&args[0])?;
                            return Ok(TypedExpr {
                                kind: TypedExprKind::EvalCurve {
                                    param_index: param_idx,
                                    arg: Box::new(arg),
                                },
                                ty: TypeName::Float,
                                span: expr.span,
                            });
                        }
                        ParamType::Path => {
                            if args.len() != 1 {
                                return Err(CompileError::type_error(
                                    "Path evaluation takes exactly 1 argument (time)",
                                    expr.span,
                                ));
                            }
                            let arg = self.check_expr(&args[0])?;
                            return Ok(TypedExpr {
                                kind: TypedExprKind::EvalPath {
                                    param_index: param_idx,
                                    arg: Box::new(arg),
                                },
                                ty: TypeName::Vec2,
                                span: expr.span,
                            });
                        }
                        _ => {
                            return Err(CompileError::type_error(
                                format!("'{name}' is a param, not a callable function"),
                                expr.span,
                            ));
                        }
                    }
                }

                // Check built-in functions
                let builtin = builtins::lookup_builtin(name).ok_or_else(|| {
                    CompileError::type_error(format!("Unknown function: '{name}'"), expr.span)
                })?;

                if args.len() != builtin.params.len() {
                    return Err(CompileError::type_error(
                        format!(
                            "'{name}' expects {} args, got {}",
                            builtin.params.len(),
                            args.len()
                        ),
                        expr.span,
                    ));
                }

                let mut typed_args = Vec::new();
                for (i, arg) in args.iter().enumerate() {
                    let mut typed_arg = self.check_expr(arg)?;
                    // Auto-promote int to float for builtin params
                    if builtin.params[i].1 == TypeName::Float && typed_arg.ty == TypeName::Int {
                        typed_arg = Self::coerce_to_float(typed_arg);
                    }
                    typed_args.push(typed_arg);
                }

                Ok(TypedExpr {
                    kind: TypedExprKind::BuiltinCall {
                        name: name.clone(),
                        args: typed_args,
                    },
                    ty: builtin.ret.clone(),
                    span: expr.span,
                })
            }

            ExprKind::MethodCall { object, method, args } => {
                let typed_obj = self.check_expr(object)?;
                if typed_obj.ty == TypeName::Color && method == "scale" {
                    if args.len() != 1 {
                        return Err(CompileError::type_error(
                            "color.scale() takes exactly 1 argument",
                            expr.span,
                        ));
                    }
                    let factor = self.check_expr(&args[0])?;
                    return Ok(TypedExpr {
                        kind: TypedExprKind::ColorScale {
                            color: Box::new(typed_obj),
                            factor: Box::new(factor),
                        },
                        ty: TypeName::Color,
                        span: expr.span,
                    });
                }
                Err(CompileError::type_error(
                    format!("Unknown method '{method}' on type {:?}", typed_obj.ty),
                    expr.span,
                ))
            }

            ExprKind::Field { object, field } => {
                let typed_obj = self.check_expr(object)?;
                let result_ty = match (&typed_obj.ty, field.as_str()) {
                    (TypeName::Color, "r" | "g" | "b" | "a")
                    | (TypeName::Vec2, "x" | "y") => TypeName::Float,
                    _ => {
                        // Check if this is a flags field access (opts.Mirror)
                        if let TypedExprKind::LoadParam(idx) = typed_obj.kind {
                            if let Some((ParamType::Named(ref type_name), _)) = self.params.values().find(|(_, pi)| *pi == idx) {
                                if let Some(flag_variants) = self.flags.get(type_name) {
                                    if let Some(bit_pos) = flag_variants.iter().position(|v| v == field) {
                                        return Ok(TypedExpr {
                                            kind: TypedExprKind::FlagTest {
                                                param_index: idx,
                                                bit_mask: 1u32 << bit_pos,
                                            },
                                            ty: TypeName::Bool,
                                            span: expr.span,
                                        });
                                    }
                                }
                            }
                        }
                        return Err(CompileError::type_error(
                            format!("No field '{field}' on type {:?}", typed_obj.ty),
                            expr.span,
                        ));
                    }
                };
                Ok(TypedExpr {
                    kind: TypedExprKind::Field {
                        object: Box::new(typed_obj),
                        field: field.clone(),
                    },
                    ty: result_ty,
                    span: expr.span,
                })
            }

            ExprKind::If { condition, then_body, else_body } => {
                let typed_cond = self.check_expr(condition)?;
                if typed_cond.ty != TypeName::Bool {
                    return Err(CompileError::type_error(
                        format!("If condition must be bool, got {:?}", typed_cond.ty),
                        condition.span,
                    ));
                }

                let typed_then = self.check_block(then_body)?;
                let typed_else = if let Some(eb) = else_body {
                    Some(self.check_block(eb)?)
                } else {
                    None
                };

                // Determine result type from last expression in each branch
                let then_ty = Self::block_result_type(&typed_then);
                let else_ty = typed_else.as_ref().and_then(|eb| Self::block_result_type(eb));

                let result_ty = match (then_ty, else_ty) {
                    (Some(t), Some(e)) if t == e => t,
                    (Some(t), None) if else_body.is_none() => {
                        // if-without-else used as a value — require else branch
                        return Err(CompileError::type_error(
                            format!("'if' expression produces a value ({t:?}) but has no 'else' branch; add an 'else' clause"),
                            expr.span,
                        ));
                    }
                    (Some(_), Some(_)) => {
                        return Err(CompileError::type_error(
                            "if/else branches must have the same type",
                            expr.span,
                        ));
                    }
                    _ => TypeName::Bool, // no result expressions (both branches are let-only)
                };

                Ok(TypedExpr {
                    kind: TypedExprKind::If {
                        condition: Box::new(typed_cond),
                        then_body: typed_then,
                        else_body: typed_else,
                    },
                    ty: result_ty,
                    span: expr.span,
                })
            }

            ExprKind::Switch { scrutinee, cases, default } => {
                let typed_scrutinee = self.check_expr(scrutinee)?;
                // Scrutinee must be comparable (int, float, or bool)
                if !matches!(typed_scrutinee.ty, TypeName::Int | TypeName::Float | TypeName::Bool) {
                    return Err(CompileError::type_error(
                        format!("Switch scrutinee must be int, float, or bool, got {:?}", typed_scrutinee.ty),
                        scrutinee.span,
                    ));
                }

                // Require at least a default branch
                if default.is_none() && cases.is_empty() {
                    return Err(CompileError::type_error(
                        "Switch expression must have at least one case or a default",
                        expr.span,
                    ));
                }

                // Type-check each case
                let mut typed_cases = Vec::new();
                let mut branch_type: Option<TypeName> = None;
                for (pattern, body) in cases {
                    let mut typed_pat = self.check_expr(pattern)?;
                    // Auto-promote int to float if scrutinee is float
                    if typed_scrutinee.ty == TypeName::Float && typed_pat.ty == TypeName::Int {
                        typed_pat = Self::coerce_to_float(typed_pat);
                    }
                    let typed_body = self.check_block(body)?;
                    if let Some(ty) = Self::block_result_type(&typed_body) {
                        if let Some(ref expected) = branch_type {
                            if ty != *expected {
                                return Err(CompileError::type_error(
                                    "All switch branches must have the same type",
                                    expr.span,
                                ));
                            }
                        } else {
                            branch_type = Some(ty);
                        }
                    }
                    typed_cases.push((typed_pat, typed_body));
                }

                // Type-check default branch
                let typed_default = if let Some(def) = default {
                    let typed_body = self.check_block(def)?;
                    if let Some(ty) = Self::block_result_type(&typed_body) {
                        if let Some(ref expected) = branch_type {
                            if ty != *expected {
                                return Err(CompileError::type_error(
                                    "Default branch must match other case types",
                                    expr.span,
                                ));
                            }
                        } else {
                            branch_type = Some(ty);
                        }
                    }
                    Some(typed_body)
                } else {
                    None
                };

                let result_ty = branch_type.unwrap_or(TypeName::Bool);

                // Desugar switch into chained if/else for the typed AST
                // switch x { case A => body_a, case B => body_b, default => body_d }
                // becomes: if x == A { body_a } else { if x == B { body_b } else { body_d } }
                let scrutinee_for_compare = typed_scrutinee;
                let typed_expr = Self::desugar_switch(
                    scrutinee_for_compare,
                    &typed_cases,
                    typed_default.as_deref(),
                    result_ty.clone(),
                    expr.span,
                );

                Ok(typed_expr)
            }

            ExprKind::EnumAccess { enum_name, variant } => {
                // Look up the enum type and resolve variant to its index
                let variants = self.enums.get(enum_name).ok_or_else(|| {
                    CompileError::type_error(format!("Unknown enum type: '{enum_name}'"), expr.span)
                })?;
                let variant_idx = variants.iter().position(|v| v == variant).ok_or_else(|| {
                    CompileError::type_error(
                        format!("Unknown variant '{variant}' in enum '{enum_name}'"),
                        expr.span,
                    )
                })?;

                // Enum variant resolves to its integer index.
                // Comparison happens naturally via BinOp Eq/Ne.
                #[allow(clippy::cast_possible_wrap)]
                Ok(TypedExpr {
                    kind: TypedExprKind::IntLit(variant_idx as i32),
                    ty: TypeName::Int,
                    span: expr.span,
                })
            }

            ExprKind::FlagCombine(_) | ExprKind::GradientLit(_) | ExprKind::CurveLit(_) => {
                Err(CompileError::type_error(
                    "This expression is only valid in param defaults",
                    expr.span,
                ))
            }
        }
    }

    fn resolve_ident(&self, name: &str, span: Span) -> Result<TypedExpr, CompileError> {
        // Check locals (most recently defined first)
        for scope in self.locals.iter().rev() {
            if let Some((ty, idx)) = scope.get(name) {
                return Ok(TypedExpr {
                    kind: TypedExprKind::LoadLocal(*idx),
                    ty: ty.clone(),
                    span,
                });
            }
        }

        // Check params
        if let Some((param_ty, idx)) = self.params.get(name) {
            return match param_ty {
                // Color params use a separate load path (not stored as f64)
                ParamType::Color => Ok(TypedExpr {
                    kind: TypedExprKind::LoadColor(*idx),
                    ty: TypeName::Color,
                    span,
                }),
                // Path params as bare ident → evaluate at abs_t, return Vec2
                ParamType::Path => Ok(TypedExpr {
                    kind: TypedExprKind::EvalPathAtT { param_index: *idx },
                    ty: TypeName::Vec2,
                    span,
                }),
                _ => {
                    #[allow(clippy::unreachable)]
                    let ty = match param_ty {
                        ParamType::Float(_) => TypeName::Float,
                        ParamType::Int(_) | ParamType::Named(_) => TypeName::Int,
                        ParamType::Bool => TypeName::Bool,
                        ParamType::Color | ParamType::Path => unreachable!(), // handled above
                        ParamType::Gradient => TypeName::Gradient,
                        ParamType::Curve => TypeName::Curve,
                    };
                    Ok(TypedExpr {
                        kind: TypedExprKind::LoadParam(*idx),
                        ty,
                        span,
                    })
                }
            };
        }

        // Check implicit builtins (single source of truth in builtins::IMPLICIT_VARS)
        if let Some((ty, builtin_var)) = builtins::lookup_implicit(name) {
            return Ok(TypedExpr {
                kind: TypedExprKind::LoadBuiltin(builtin_var),
                ty: ty.clone(),
                span,
            });
        }

        Err(CompileError::type_error(
            format!("Undefined variable: '{name}'"),
            span,
        ))
    }

    fn coerce_to_float(expr: TypedExpr) -> TypedExpr {
        let span = expr.span;
        TypedExpr {
            kind: TypedExprKind::IntToFloat(Box::new(expr)),
            ty: TypeName::Float,
            span,
        }
    }

    fn check_block(&mut self, stmts: &[Stmt]) -> Result<Vec<TypedStmt>, CompileError> {
        let mut typed = Vec::new();
        for stmt in stmts {
            typed.push(self.check_stmt(stmt)?);
        }
        Ok(typed)
    }

    fn block_result_type(stmts: &[TypedStmt]) -> Option<TypeName> {
        stmts.last().and_then(|s| match &s.kind {
            TypedStmtKind::Expr(e) => Some(e.ty.clone()),
            TypedStmtKind::Let { .. } => None,
        })
    }

    /// Desugar a type-checked switch into chained if/else expressions.
    fn desugar_switch(
        scrutinee: TypedExpr,
        cases: &[(TypedExpr, Vec<TypedStmt>)],
        default: Option<&[TypedStmt]>,
        result_ty: TypeName,
        span: Span,
    ) -> TypedExpr {
        // Build from the inside out: start with the default (or last case)
        let mut result: Option<TypedExpr> = default.map(|body| {
            // If the default is a single expression, use it directly
            if body.len() == 1 {
                if let TypedStmtKind::Expr(ref e) = body[0].kind {
                    return e.clone();
                }
            }
            // Wrap in always-true if block
            TypedExpr {
                kind: TypedExprKind::If {
                    condition: Box::new(TypedExpr {
                        kind: TypedExprKind::BoolLit(true),
                        ty: TypeName::Bool,
                        span,
                    }),
                    then_body: body.to_vec(),
                    else_body: None,
                },
                ty: result_ty.clone(),
                span,
            }
        });

        // Build chained if/else from last case to first
        for (pattern, body) in cases.iter().rev() {
            // Generate: if scrutinee == pattern { body } else { previous }
            let condition = TypedExpr {
                kind: TypedExprKind::BinOp {
                    op: BinOp::Eq,
                    left: Box::new(scrutinee.clone()),
                    right: Box::new(pattern.clone()),
                },
                ty: TypeName::Bool,
                span,
            };

            let else_body = result.map(|prev| {
                vec![TypedStmt {
                    kind: TypedStmtKind::Expr(prev),
                    span,
                }]
            });

            result = Some(TypedExpr {
                kind: TypedExprKind::If {
                    condition: Box::new(condition),
                    then_body: body.clone(),
                    else_body,
                },
                ty: result_ty.clone(),
                span,
            });
        }

        result.unwrap_or(TypedExpr {
            kind: TypedExprKind::BoolLit(false),
            ty: TypeName::Bool,
            span,
        })
    }

    fn inline_function_call(&mut self, fn_def: &FnDef, args: &[Expr], span: Span) -> Result<TypedExpr, CompileError> {
        if args.len() != fn_def.params.len() {
            return Err(CompileError::type_error(
                format!(
                    "'{}' expects {} args, got {}",
                    fn_def.name,
                    fn_def.params.len(),
                    args.len()
                ),
                span,
            ));
        }

        // Guard against recursive (or deeply nested) function calls
        if self.inline_depth >= MAX_INLINE_DEPTH {
            return Err(CompileError::type_error(
                format!(
                    "Function '{}' exceeds maximum inlining depth ({MAX_INLINE_DEPTH}); recursive functions are not supported",
                    fn_def.name
                ),
                span,
            ));
        }
        self.inline_depth += 1;

        // Push a new scope for function parameters
        self.locals.push(HashMap::new());

        // Evaluate args and bind them as locals
        let mut preamble = Vec::new();
        for (param, arg) in fn_def.params.iter().zip(args.iter()) {
            let typed_arg = match self.check_expr(arg) {
                Ok(a) => a,
                Err(e) => {
                    self.locals.pop();
                    self.inline_depth -= 1;
                    return Err(e);
                }
            };
            let local_idx = self.next_local;
            self.next_local += 1;
            if let Some(scope) = self.locals.last_mut() {
                scope.insert(param.name.clone(), (param.ty.clone(), local_idx));
            }
            preamble.push(TypedStmt {
                kind: TypedStmtKind::Let {
                    name: param.name.clone(),
                    value: typed_arg,
                    local_index: local_idx,
                },
                span,
            });
        }

        // Type check the function body
        let mut body = Vec::new();
        let mut body_err = None;
        for stmt in &fn_def.body {
            match self.check_stmt(stmt) {
                Ok(ts) => body.push(ts),
                Err(e) => { body_err = Some(e); break; }
            }
        }

        // Pop the scope and restore inline depth (must happen even on error)
        self.locals.pop();
        self.inline_depth -= 1;

        if let Some(e) = body_err {
            return Err(e);
        }

        // The result is the last expression in the body
        // Wrap everything in a series of lets + final expr
        let result_ty = Self::block_result_type(&body).unwrap_or(TypeName::Float);

        // Build the inlined expression
        let mut all_stmts = preamble;
        all_stmts.extend(body);

        // Return as an if-like construct with just a then branch
        // Actually, for simplicity, we'll return the last expr
        if let Some(last) = all_stmts.pop() {
            if let TypedStmtKind::Expr(last_expr) = last.kind {
                if all_stmts.is_empty() {
                    return Ok(last_expr);
                }
                return Ok(TypedExpr {
                    kind: TypedExprKind::If {
                        condition: Box::new(TypedExpr {
                            kind: TypedExprKind::BoolLit(true),
                            ty: TypeName::Bool,
                            span,
                        }),
                        then_body: {
                            let mut stmts = all_stmts;
                            stmts.push(TypedStmt {
                                kind: TypedStmtKind::Expr(last_expr.clone()),
                                span,
                            });
                            stmts
                        },
                        else_body: None,
                    },
                    ty: result_ty,
                    span,
                });
            }
            all_stmts.push(last);
        }

        Err(CompileError::type_error(
            format!("Function '{}' body must end with an expression", fn_def.name),
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

    use crate::dsl::ast::TypeName;

    fn check(src: &str) -> TypedScript {
        let tokens = lex(src).unwrap();
        let script = parse(tokens).unwrap();
        type_check(&script).unwrap()
    }

    fn check_err(src: &str) -> Vec<CompileError> {
        let tokens = lex(src).unwrap();
        let script = parse(tokens).unwrap();
        type_check(&script).unwrap_err()
    }

    /// Extract the inferred type of the last statement in the body.
    fn last_ty(typed: &TypedScript) -> &TypeName {
        match &typed.body.last().unwrap().kind {
            TypedStmtKind::Expr(e) => &e.ty,
            TypedStmtKind::Let { value, .. } => &value.ty,
        }
    }

    /// Extract the inferred type of the let binding at body[index].
    fn let_ty(typed: &TypedScript, index: usize) -> &TypeName {
        match &typed.body[index].kind {
            TypedStmtKind::Let { value, .. } => &value.ty,
            _ => panic!("expected Let at body[{index}]"),
        }
    }

    #[test]
    fn simple_solid_color() {
        let typed = check("rgb(1.0, 0.0, 0.0)");
        assert_eq!(*last_ty(&typed), TypeName::Color);
    }

    #[test]
    fn let_and_use() {
        let typed = check("let x = t * 2.0; rgb(x, 0.0, 0.0)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Float);
        assert_eq!(*last_ty(&typed), TypeName::Color);
    }

    #[test]
    fn builtin_math() {
        let typed = check("let s = sin(t * 3.14); rgb(s, s, s)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Float);
        assert_eq!(*last_ty(&typed), TypeName::Color);
    }

    #[test]
    fn if_else() {
        let typed = check("if t > 0.5 { rgb(1.0, 0.0, 0.0) } else { rgb(0.0, 0.0, 1.0) }");
        assert_eq!(*last_ty(&typed), TypeName::Color);
    }

    #[test]
    fn type_error_non_bool_condition() {
        let errors = check_err("if 1.0 { rgb(1.0, 0.0, 0.0) }");
        assert!(errors.iter().any(|e| e.message.contains("bool")));
    }

    #[test]
    fn int_auto_promotion() {
        let typed = check("let x = 1 + 2.0; rgb(x, x, x)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Float, "int + float should promote to float");
        assert_eq!(*last_ty(&typed), TypeName::Color);
    }

    #[test]
    fn enum_comparison() {
        let typed = check("enum Mode { A, B }\nparam mode: Mode = A;\nif mode == Mode.A { rgb(1.0, 0.0, 0.0) } else { rgb(0.0, 1.0, 0.0) }");
        assert_eq!(typed.enums.len(), 1);
        assert_eq!(*last_ty(&typed), TypeName::Color);
    }

    #[test]
    fn undefined_var_error() {
        let errors = check_err("rgb(undefined_var, 0.0, 0.0)");
        assert!(errors.iter().any(|e| e.message.contains("Undefined")));
    }

    #[test]
    fn user_function_inline() {
        let typed = check("fn double(x: float) -> float { x * 2.0 }\nlet v = double(t); rgb(v, v, v)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Float, "inlined function returning float");
        assert_eq!(*last_ty(&typed), TypeName::Color);
    }

    #[test]
    fn recursive_function_error() {
        let errors = check_err("fn boom(x: float) -> float { boom(x) }\nlet v = boom(t); rgb(v, v, v)");
        assert!(errors.iter().any(|e| e.message.contains("inlining depth")));
    }

    #[test]
    fn mutual_recursion_error() {
        let errors = check_err("fn a(x: float) -> float { b(x) }\nfn b(x: float) -> float { a(x) }\nlet v = a(t); rgb(v, v, v)");
        assert!(errors.iter().any(|e| e.message.contains("inlining depth")));
    }

    #[test]
    fn if_without_else_error() {
        let errors = check_err("if t > 0.5 { rgb(1.0, 0.0, 0.0) }");
        assert!(errors.iter().any(|e| e.message.contains("no 'else' branch")));
    }

    #[test]
    fn if_with_else_ok() {
        let typed = check("if t > 0.5 { rgb(1.0, 0.0, 0.0) } else { rgb(0.0, 0.0, 1.0) }");
        assert_eq!(*last_ty(&typed), TypeName::Color);
    }

    // ── Issue #72: Bitwise operators ─────────────────────────────

    #[test]
    fn bitwise_and_typechecks() {
        let typed = check("let x = 3 & 1; rgb(0.0, 0.0, 0.0)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Int);
    }

    #[test]
    fn bitwise_xor_typechecks() {
        let typed = check("let x = 3 ^ 1; rgb(0.0, 0.0, 0.0)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Int);
    }

    #[test]
    fn bitwise_or_typechecks() {
        let typed = check("let x = 3 | 1; rgb(0.0, 0.0, 0.0)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Int);
    }

    #[test]
    fn shift_left_typechecks() {
        let typed = check("let x = 1 << 3; rgb(0.0, 0.0, 0.0)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Int);
    }

    #[test]
    fn shift_right_typechecks() {
        let typed = check("let x = 8 >> 2; rgb(0.0, 0.0, 0.0)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Int);
    }

    #[test]
    fn bitwise_on_non_numeric_error() {
        let errors = check_err("let x = true & false; rgb(0.0, 0.0, 0.0)");
        assert!(errors.iter().any(|e| e.message.contains("non-numeric")));
    }

    // ── Issue #72: Power operator ───────────────────────────────

    #[test]
    fn power_operator_typechecks() {
        let typed = check("let x = 2.0 ** 3.0; rgb(x, 0.0, 0.0)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Float);
    }

    #[test]
    fn power_operator_int_promotion() {
        let typed = check("let x = 2 ** 3.0; rgb(x, 0.0, 0.0)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Float, "int ** float should promote to float");
    }

    // ── Issue #73: Ternary operator ─────────────────────────────

    #[test]
    fn ternary_typechecks() {
        let typed = check("t > 0.5 ? rgb(1.0, 0.0, 0.0) : rgb(0.0, 0.0, 1.0)");
        assert_eq!(*last_ty(&typed), TypeName::Color);
    }

    // ── Issue #70: Switch expression ────────────────────────────

    #[test]
    fn switch_enum_typechecks() {
        let typed = check("enum Mode { A, B }\nparam mode: Mode = A;\nswitch mode {\ncase Mode.A => rgb(1.0, 0.0, 0.0)\ncase Mode.B => rgb(0.0, 1.0, 0.0)\ndefault => rgb(0.0, 0.0, 1.0)\n}");
        assert_eq!(*last_ty(&typed), TypeName::Color);
    }

    #[test]
    fn switch_mismatched_branches_error() {
        let errors = check_err("enum Mode { A, B }\nparam mode: Mode = A;\nswitch mode {\ncase Mode.A => rgb(1.0, 0.0, 0.0)\ndefault => 1.0\n}");
        assert!(errors.iter().any(|e| e.message.contains("same type") || e.message.contains("match")),
            "Should error on mismatched branch types, got: {:?}", errors);
    }

    // ── Issue #74: Easing builtins ──────────────────────────────

    #[test]
    fn easing_functions_typecheck() {
        for func in ["ease_in", "ease_out", "ease_in_out", "ease_in_cubic", "ease_out_cubic", "ease_in_out_cubic"] {
            let typed = check(&format!("let x = {func}(t); rgb(x, x, x)"));
            assert_eq!(*let_ty(&typed, 0), TypeName::Float, "{func} should return float");
        }
    }

    // ── Issue #77: Randomness builtins ──────────────────────────

    #[test]
    fn random_functions_typecheck() {
        let typed = check("let x = hash3(1.0, 2.0, 3.0); rgb(x, x, x)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Float, "hash3 should return float");
        let typed = check("let x = random(t); rgb(x, x, x)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Float, "random should return float");
        let typed = check("let x = random_range(0.0, 1.0, t); rgb(x, x, x)");
        assert_eq!(*let_ty(&typed, 0), TypeName::Float, "random_range should return float");
    }

    // ── Issue #78: Noise builtins ───────────────────────────────

    #[test]
    fn noise_functions_typecheck() {
        for (func, src) in [
            ("noise", "let x = abs(noise(t * 5.0)); rgb(x, x, x)"),
            ("noise2", "let x = abs(noise2(t, pos)); rgb(x, x, x)"),
            ("noise3", "let x = abs(noise3(t, pos, 0.0)); rgb(x, x, x)"),
            ("fbm", "let x = abs(fbm(t, pos, 4.0)); rgb(x, x, x)"),
            ("worley2", "let x = worley2(t, pos); rgb(x, x, x)"),
        ] {
            let typed = check(src);
            assert_eq!(*let_ty(&typed, 0), TypeName::Float, "{func} should return float");
        }
    }
}
