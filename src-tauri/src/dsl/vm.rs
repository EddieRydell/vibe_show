use super::compiler::{CompiledScript, Op};
use crate::model::color::Color;
use crate::model::color_gradient::ColorGradient;
use crate::model::curve::Curve;
use crate::model::motion_path::MotionPath;

/// Maximum stack depth to prevent runaway scripts.
const MAX_STACK: usize = 256;

/// Runtime value on the VM stack.
#[derive(Debug, Clone, Copy)]
enum Value {
    Float(f64),
    Color(Color),
    Vec2(f64, f64),
}

impl Value {
    fn as_float(self) -> f64 {
        match self {
            Self::Float(f) => f,
            Self::Color(_) | Self::Vec2(_, _) => 0.0,
        }
    }

    fn as_color(self) -> Color {
        match self {
            Self::Color(c) => c,
            _ => Color::BLACK,
        }
    }
}

/// Reusable VM working memory. Create once per batch, reuse across pixels
/// to avoid heap allocations in the per-pixel hot path.
#[derive(Default)]
pub struct VmBuffers {
    stack: Vec<Value>,
    locals: Vec<Value>,
}

impl VmBuffers {
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(64),
            locals: Vec::new(),
        }
    }

    /// Clear and resize for a new execution. Reuses existing heap allocations.
    fn reset(&mut self, local_count: usize) {
        self.stack.clear();
        self.locals.clear();
        self.locals.resize(local_count, Value::Float(0.0));
    }
}

/// Runtime context provided per-pixel.
pub struct VmContext<'a> {
    pub t: f64,
    pub pixel: usize,
    pub pixels: usize,
    pub pos: f64,
    pub pos2d: (f64, f64),
    /// Absolute time in seconds (for motion path evaluation).
    pub abs_t: f64,
    pub param_values: &'a [f64],
    pub gradients: &'a [Option<&'a ColorGradient>],
    pub curves: &'a [Option<&'a Curve>],
    pub colors: &'a [Option<Color>],
    pub paths: &'a [Option<&'a MotionPath>],
}

/// Execute a compiled script for one pixel, returning the output color.
///
/// For batch execution, prefer `execute_reuse` with a shared `VmBuffers`
/// to avoid per-pixel heap allocations.
#[allow(clippy::too_many_lines)]
pub fn execute(script: &CompiledScript, ctx: &VmContext<'_>) -> Color {
    let mut buffers = VmBuffers::new();
    execute_reuse(script, ctx, &mut buffers)
}

/// Execute a compiled script reusing pre-allocated buffers.
///
/// This avoids heap allocations on every pixel — call `execute_reuse` in a
/// loop with the same `VmBuffers` for zero-alloc per-pixel evaluation.
#[allow(clippy::too_many_lines)]
pub fn execute_reuse(script: &CompiledScript, ctx: &VmContext<'_>, buffers: &mut VmBuffers) -> Color {
    buffers.reset(script.local_count as usize);
    let stack = &mut buffers.stack;
    let locals = &mut buffers.locals;
    let mut ip: usize = 0;
    let ops = &script.ops;
    let consts = &script.constants;

    while ip < ops.len() {
        if stack.len() >= MAX_STACK {
            return Color::BLACK;
        }

        match ops[ip] {
            Op::PushConst(idx) => {
                stack.push(Value::Float(consts[idx as usize]));
            }
            Op::PushParam(idx) => {
                let val = ctx.param_values.get(idx as usize).copied().unwrap_or(0.0);
                stack.push(Value::Float(val));
            }
            Op::LoadLocal(idx) => {
                let val = locals.get(idx as usize).copied().unwrap_or(Value::Float(0.0));
                stack.push(val);
            }
            Op::StoreLocal(idx) => {
                if let Some(val) = stack.pop() {
                    if (idx as usize) < locals.len() {
                        locals[idx as usize] = val;
                    }
                }
            }
            Op::Pop => {
                stack.pop();
            }

            // Arithmetic
            Op::Add => float_binop(stack, |a, b| a + b),
            Op::Sub => float_binop(stack, |a, b| a - b),
            Op::Mul => float_binop(stack, |a, b| a * b),
            Op::Div => float_binop(stack, |a, b| if b == 0.0 { 0.0 } else { a / b }),
            Op::Mod => float_binop(stack, |a, b| if b == 0.0 { 0.0 } else { a % b }),
            Op::Neg => {
                if let Some(val) = stack.pop() {
                    stack.push(Value::Float(-val.as_float()));
                }
            }

            // Comparison
            Op::Lt => float_cmp(stack, |a, b| a < b),
            Op::Gt => float_cmp(stack, |a, b| a > b),
            Op::Le => float_cmp(stack, |a, b| a <= b),
            Op::Ge => float_cmp(stack, |a, b| a >= b),
            Op::Eq => float_cmp(stack, |a, b| (a - b).abs() < f64::EPSILON),
            Op::Ne => float_cmp(stack, |a, b| (a - b).abs() >= f64::EPSILON),

            // Logic
            Op::And => float_binop(stack, |a, b| {
                if a != 0.0 && b != 0.0 { 1.0 } else { 0.0 }
            }),
            Op::Or => float_binop(stack, |a, b| {
                if a != 0.0 || b != 0.0 { 1.0 } else { 0.0 }
            }),
            Op::Not => {
                if let Some(val) = stack.pop() {
                    stack.push(Value::Float(if val.as_float() == 0.0 { 1.0 } else { 0.0 }));
                }
            }

            // Math (1-arg)
            Op::Sin => float_unary(stack, f64::sin),
            Op::Cos => float_unary(stack, f64::cos),
            Op::Tan => float_unary(stack, f64::tan),
            Op::Abs => float_unary(stack, f64::abs),
            Op::Floor => float_unary(stack, f64::floor),
            Op::Ceil => float_unary(stack, f64::ceil),
            Op::Round => float_unary(stack, f64::round),
            Op::Fract => float_unary(stack, f64::fract),
            Op::Sqrt => float_unary(stack, f64::sqrt),
            Op::Sign => float_unary(stack, f64::signum),
            Op::Exp => float_unary(stack, f64::exp),
            Op::Log => float_unary(stack, f64::ln),

            // Math (2-arg)
            Op::Pow => float_binop(stack, f64::powf),
            Op::Min => float_binop(stack, f64::min),
            Op::Max => float_binop(stack, f64::max),
            Op::Step => float_binop(stack, |edge, x| if x < edge { 0.0 } else { 1.0 }),
            Op::Atan2 => float_binop(stack, f64::atan2),
            Op::Modf => float_binop(stack, |a, b| if b == 0.0 { 0.0 } else { a % b }),

            // Math (3-arg)
            Op::Clamp => {
                if stack.len() >= 3 {
                    let max_val = stack.pop().map_or(0.0, Value::as_float);
                    let min_val = stack.pop().map_or(0.0, Value::as_float);
                    let x = stack.pop().map_or(0.0, Value::as_float);
                    stack.push(Value::Float(x.clamp(min_val, max_val)));
                }
            }
            Op::Mix => {
                if stack.len() >= 3 {
                    let t = stack.pop().map_or(0.0, Value::as_float);
                    let b = stack.pop().map_or(0.0, Value::as_float);
                    let a = stack.pop().map_or(0.0, Value::as_float);
                    stack.push(Value::Float(a + (b - a) * t));
                }
            }
            Op::Smoothstep => {
                if stack.len() >= 3 {
                    let x = stack.pop().map_or(0.0, Value::as_float);
                    let edge1 = stack.pop().map_or(0.0, Value::as_float);
                    let edge0 = stack.pop().map_or(0.0, Value::as_float);
                    let t = if edge0 >= edge1 {
                        0.0
                    } else {
                        ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0)
                    };
                    stack.push(Value::Float(t * t * (3.0 - 2.0 * t)));
                }
            }

            // Color constructors
            Op::Rgb => {
                if stack.len() >= 3 {
                    let b = stack.pop().map_or(0.0, Value::as_float);
                    let g = stack.pop().map_or(0.0, Value::as_float);
                    let r = stack.pop().map_or(0.0, Value::as_float);
                    stack.push(Value::Color(Color::rgb(
                        float_to_u8(r),
                        float_to_u8(g),
                        float_to_u8(b),
                    )));
                }
            }
            Op::Hsv => {
                if stack.len() >= 3 {
                    let v = stack.pop().map_or(0.0, Value::as_float);
                    let s = stack.pop().map_or(0.0, Value::as_float);
                    let h = stack.pop().map_or(0.0, Value::as_float);
                    stack.push(Value::Color(Color::from_hsv(h, s, v)));
                }
            }
            Op::Rgba => {
                if stack.len() >= 4 {
                    let a = stack.pop().map_or(0.0, Value::as_float);
                    let b = stack.pop().map_or(0.0, Value::as_float);
                    let g = stack.pop().map_or(0.0, Value::as_float);
                    let r = stack.pop().map_or(0.0, Value::as_float);
                    stack.push(Value::Color(Color::rgba(
                        float_to_u8(r),
                        float_to_u8(g),
                        float_to_u8(b),
                        float_to_u8(a),
                    )));
                }
            }
            Op::ColorScale => {
                if stack.len() >= 2 {
                    let factor = stack.pop().map_or(0.0, Value::as_float);
                    let color = stack.pop().map_or(Color::BLACK, Value::as_color);
                    stack.push(Value::Color(color.scale(factor)));
                }
            }
            Op::ColorR => {
                if let Some(val) = stack.pop() {
                    let c = val.as_color();
                    stack.push(Value::Float(f64::from(c.r) / 255.0));
                }
            }
            Op::ColorG => {
                if let Some(val) = stack.pop() {
                    let c = val.as_color();
                    stack.push(Value::Float(f64::from(c.g) / 255.0));
                }
            }
            Op::ColorB => {
                if let Some(val) = stack.pop() {
                    let c = val.as_color();
                    stack.push(Value::Float(f64::from(c.b) / 255.0));
                }
            }
            Op::ColorA => {
                if let Some(val) = stack.pop() {
                    let c = val.as_color();
                    stack.push(Value::Float(f64::from(c.a) / 255.0));
                }
            }

            // Vec2
            Op::MakeVec2 => {
                if stack.len() >= 2 {
                    let y = stack.pop().map_or(0.0, Value::as_float);
                    let x = stack.pop().map_or(0.0, Value::as_float);
                    stack.push(Value::Vec2(x, y));
                }
            }
            Op::Vec2X => {
                if let Some(val) = stack.pop() {
                    match val {
                        Value::Vec2(x, _) => stack.push(Value::Float(x)),
                        _ => stack.push(Value::Float(0.0)),
                    }
                }
            }
            Op::Vec2Y => {
                if let Some(val) = stack.pop() {
                    match val {
                        Value::Vec2(_, y) => stack.push(Value::Float(y)),
                        _ => stack.push(Value::Float(0.0)),
                    }
                }
            }
            Op::Distance => {
                if stack.len() >= 2 {
                    let b = stack.pop().unwrap_or(Value::Float(0.0));
                    let a = stack.pop().unwrap_or(Value::Float(0.0));
                    match (a, b) {
                        (Value::Vec2(ax, ay), Value::Vec2(bx, by)) => {
                            let dx = bx - ax;
                            let dy = by - ay;
                            stack.push(Value::Float((dx * dx + dy * dy).sqrt()));
                        }
                        _ => stack.push(Value::Float(0.0)),
                    }
                }
            }
            Op::Length => {
                if let Some(val) = stack.pop() {
                    match val {
                        Value::Vec2(x, y) => stack.push(Value::Float((x * x + y * y).sqrt())),
                        _ => stack.push(Value::Float(0.0)),
                    }
                }
            }
            Op::Dot => {
                if stack.len() >= 2 {
                    let b = stack.pop().unwrap_or(Value::Float(0.0));
                    let a = stack.pop().unwrap_or(Value::Float(0.0));
                    match (a, b) {
                        (Value::Vec2(ax, ay), Value::Vec2(bx, by)) => {
                            stack.push(Value::Float(ax * bx + ay * by));
                        }
                        _ => stack.push(Value::Float(0.0)),
                    }
                }
            }
            Op::Normalize => {
                if let Some(val) = stack.pop() {
                    match val {
                        Value::Vec2(x, y) => {
                            let len = (x * x + y * y).sqrt();
                            if len > 0.0 {
                                stack.push(Value::Vec2(x / len, y / len));
                            } else {
                                stack.push(Value::Vec2(0.0, 0.0));
                            }
                        }
                        _ => stack.push(Value::Vec2(0.0, 0.0)),
                    }
                }
            }

            // Gradient/Curve evaluation
            Op::EvalGradient(param_idx) => {
                if let Some(t_val) = stack.pop() {
                    let t = t_val.as_float();
                    let color = ctx.gradients.get(param_idx as usize)
                        .and_then(|g| g.as_ref())
                        .map_or(Color::BLACK, |g| g.evaluate(t));
                    stack.push(Value::Color(color));
                }
            }
            Op::EvalCurve(param_idx) => {
                if let Some(x_val) = stack.pop() {
                    let x = x_val.as_float();
                    let y = ctx.curves.get(param_idx as usize)
                        .and_then(|c| c.as_ref())
                        .map_or(0.0, |c| c.evaluate(x));
                    stack.push(Value::Float(y));
                }
            }
            Op::LoadColor(param_idx) => {
                let color = ctx.colors.get(param_idx as usize)
                    .and_then(|c| *c)
                    .unwrap_or(Color::BLACK);
                stack.push(Value::Color(color));
            }
            Op::EvalPath(param_idx) => {
                if let Some(t_val) = stack.pop() {
                    let t = t_val.as_float();
                    let (x, y) = ctx.paths.get(param_idx as usize)
                        .and_then(|p| p.as_ref())
                        .map_or((0.0, 0.0), |p| p.evaluate(t));
                    stack.push(Value::Vec2(x, y));
                }
            }
            Op::EvalPathAtT(param_idx) => {
                let (x, y) = ctx.paths.get(param_idx as usize)
                    .and_then(|p| p.as_ref())
                    .map_or((0.0, 0.0), |p| p.evaluate(ctx.abs_t));
                stack.push(Value::Vec2(x, y));
            }

            // Hash (deterministic pseudo-random)
            Op::Hash => {
                if stack.len() >= 2 {
                    let b = stack.pop().map_or(0.0, Value::as_float);
                    let a = stack.pop().map_or(0.0, Value::as_float);
                    stack.push(Value::Float(hash_f64(a, b)));
                }
            }
            Op::Hash3 => {
                if stack.len() >= 3 {
                    let c = stack.pop().map_or(0.0, Value::as_float);
                    let b = stack.pop().map_or(0.0, Value::as_float);
                    let a = stack.pop().map_or(0.0, Value::as_float);
                    stack.push(Value::Float(hash3_f64(a, b, c)));
                }
            }
            Op::Random => {
                if let Some(val) = stack.pop() {
                    let x = val.as_float();
                    stack.push(Value::Float(hash_f64(x, 0.0)));
                }
            }
            Op::RandomRange => {
                if stack.len() >= 3 {
                    let x = stack.pop().map_or(0.0, Value::as_float);
                    let max_val = stack.pop().map_or(1.0, Value::as_float);
                    let min_val = stack.pop().map_or(0.0, Value::as_float);
                    let h = hash_f64(x, 0.0);
                    stack.push(Value::Float(min_val + (max_val - min_val) * h));
                }
            }

            // Easing functions
            Op::EaseIn => float_unary(stack, |t| t * t),
            Op::EaseOut => float_unary(stack, |t| t * (2.0 - t)),
            Op::EaseInOut => float_unary(stack, |t| {
                if t < 0.5 { 2.0 * t * t } else { -1.0 + (4.0 - 2.0 * t) * t }
            }),
            Op::EaseInCubic => float_unary(stack, |t| t * t * t),
            Op::EaseOutCubic => float_unary(stack, |t| {
                let t1 = t - 1.0;
                t1 * t1 * t1 + 1.0
            }),
            Op::EaseInOutCubic => float_unary(stack, |t| {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let t1 = 2.0 * t - 2.0;
                    0.5 * t1 * t1 * t1 + 1.0
                }
            }),

            // Noise functions
            Op::Noise1 => float_unary(stack, noise::perlin1),
            Op::Noise2 => float_binop(stack, noise::perlin2),
            Op::Noise3 => {
                if stack.len() >= 3 {
                    let z = stack.pop().map_or(0.0, Value::as_float);
                    let y = stack.pop().map_or(0.0, Value::as_float);
                    let x = stack.pop().map_or(0.0, Value::as_float);
                    stack.push(Value::Float(noise::perlin3(x, y, z)));
                }
            }
            Op::Fbm => {
                if stack.len() >= 3 {
                    let octaves = stack.pop().map_or(4.0, Value::as_float);
                    let y = stack.pop().map_or(0.0, Value::as_float);
                    let x = stack.pop().map_or(0.0, Value::as_float);
                    stack.push(Value::Float(noise::fbm(x, y, octaves as u32)));
                }
            }
            Op::Worley2 => float_binop(stack, noise::worley2),

            // Enum/Flags
            #[allow(clippy::cast_sign_loss)]
            Op::EnumEq(variant_idx) => {
                if let Some(val) = stack.pop() {
                    let param_val = val.as_float() as u32;
                    stack.push(Value::Float(
                        if param_val == u32::from(variant_idx) { 1.0 } else { 0.0 }
                    ));
                }
            }
            #[allow(clippy::cast_sign_loss)]
            Op::FlagTest(bit_mask) => {
                if let Some(val) = stack.pop() {
                    let flags = val.as_float() as u32;
                    stack.push(Value::Float(
                        if flags & bit_mask != 0 { 1.0 } else { 0.0 }
                    ));
                }
            }

            // Control flow
            Op::JumpIfFalse(target) => {
                if let Some(val) = stack.pop() {
                    if val.as_float() == 0.0 {
                        ip = target as usize;
                        continue;
                    }
                }
            }
            Op::Jump(target) => {
                ip = target as usize;
                continue;
            }

            // Type conversion
            Op::IntToFloat => {
                // Int is already stored as f64, so this is a no-op in our VM
            }

            // Builtin variables
            Op::PushT => stack.push(Value::Float(ctx.t)),
            #[allow(clippy::cast_precision_loss)]
            Op::PushPixel => stack.push(Value::Float(ctx.pixel as f64)),
            #[allow(clippy::cast_precision_loss)]
            Op::PushPixels => stack.push(Value::Float(ctx.pixels as f64)),
            Op::PushPos => stack.push(Value::Float(ctx.pos)),
            Op::PushPos2d => stack.push(Value::Vec2(ctx.pos2d.0, ctx.pos2d.1)),
            Op::PushAbsT => stack.push(Value::Float(ctx.abs_t)),

            Op::Return => break,
        }

        ip += 1;
    }

    // Top of stack is the result color
    stack.pop().map_or(Color::BLACK, Value::as_color)
}

/// Convert a float in [0.0, 1.0] to a u8 in [0, 255], clamped.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn float_to_u8(f: f64) -> u8 {
    (f.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Binary operation on two floats from the stack.
fn float_binop(stack: &mut Vec<Value>, op: impl FnOnce(f64, f64) -> f64) {
    if stack.len() >= 2 {
        let b = stack.pop().map_or(0.0, Value::as_float);
        let a = stack.pop().map_or(0.0, Value::as_float);
        stack.push(Value::Float(op(a, b)));
    }
}

/// Comparison producing a bool (stored as 0.0 or 1.0).
fn float_cmp(stack: &mut Vec<Value>, op: impl FnOnce(f64, f64) -> bool) {
    if stack.len() >= 2 {
        let b = stack.pop().map_or(0.0, Value::as_float);
        let a = stack.pop().map_or(0.0, Value::as_float);
        stack.push(Value::Float(if op(a, b) { 1.0 } else { 0.0 }));
    }
}

/// Unary float operation.
fn float_unary(stack: &mut Vec<Value>, op: impl FnOnce(f64) -> f64) {
    if let Some(val) = stack.pop() {
        stack.push(Value::Float(op(val.as_float())));
    }
}

/// Deterministic hash function: maps two floats to [0, 1].
/// Based on the classic sin-based hash used in GLSL shaders.
fn hash_f64(a: f64, b: f64) -> f64 {
    let dot = a * 12.9898 + b * 78.233;
    let s = (dot.sin() * 43758.5453).fract();
    s.abs()
}

/// Deterministic 3-argument hash function: maps three floats to [0, 1].
fn hash3_f64(a: f64, b: f64, c: f64) -> f64 {
    let dot = a * 12.9898 + b * 78.233 + c * 45.164;
    let s = (dot.sin() * 43758.5453).fract();
    s.abs()
}

/// Deterministic noise algorithms (Perlin, FBM, Worley).
/// All functions are pure — no RNG state, hardcoded permutation table.
mod noise {
    /// Hardcoded permutation table for Perlin noise (doubled for wrapping).
    const PERM: [u8; 512] = {
        const P: [u8; 256] = [
            151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225,
            140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148,
            247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32,
            57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
            74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122,
            60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54,
            65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169,
            200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64,
            52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212,
            207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
            119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
            129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104,
            218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241,
            81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
            184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
            222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
        ];
        let mut table = [0u8; 512];
        let mut i = 0;
        while i < 512 {
            table[i] = P[i & 255];
            i += 1;
        }
        table
    };

    #[inline]
    fn fade(t: f64) -> f64 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    #[inline]
    fn lerp(t: f64, a: f64, b: f64) -> f64 {
        a + t * (b - a)
    }

    /// Gradient function for 1D Perlin noise.
    #[inline]
    fn grad1(hash: u8, x: f64) -> f64 {
        if hash & 1 == 0 { x } else { -x }
    }

    /// Gradient function for 2D Perlin noise.
    #[inline]
    fn grad2(hash: u8, x: f64, y: f64) -> f64 {
        let h = hash & 3;
        match h {
            0 => x + y,
            1 => -x + y,
            2 => x - y,
            _ => -x - y,
        }
    }

    /// Gradient function for 3D Perlin noise.
    #[inline]
    fn grad3(hash: u8, x: f64, y: f64, z: f64) -> f64 {
        let h = hash & 15;
        let u = if h < 8 { x } else { y };
        let v = if h < 4 { y } else if h == 12 || h == 14 { x } else { z };
        (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn perm_idx(v: i32) -> usize {
        (v & 255) as usize
    }

    /// 1D Perlin noise, returns [-1, 1].
    #[allow(clippy::cast_possible_truncation)]
    pub fn perlin1(x: f64) -> f64 {
        let xi = x.floor() as i32;
        let xf = x - x.floor();
        let u = fade(xf);

        let a = PERM[perm_idx(xi)];
        let b = PERM[perm_idx(xi + 1)];

        lerp(u, grad1(a, xf), grad1(b, xf - 1.0))
    }

    /// 2D Perlin noise, returns [-1, 1].
    #[allow(clippy::cast_possible_truncation)]
    pub fn perlin2(x: f64, y: f64) -> f64 {
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let xf = x - x.floor();
        let yf = y - y.floor();
        let u = fade(xf);
        let v = fade(yf);

        let aa = PERM[perm_idx(PERM[perm_idx(xi)] as i32 + yi)] ;
        let ab = PERM[perm_idx(PERM[perm_idx(xi)] as i32 + yi + 1)];
        let ba = PERM[perm_idx(PERM[perm_idx(xi + 1)] as i32 + yi)];
        let bb = PERM[perm_idx(PERM[perm_idx(xi + 1)] as i32 + yi + 1)];

        lerp(v,
            lerp(u, grad2(aa, xf, yf), grad2(ba, xf - 1.0, yf)),
            lerp(u, grad2(ab, xf, yf - 1.0), grad2(bb, xf - 1.0, yf - 1.0)),
        )
    }

    /// 3D Perlin noise, returns [-1, 1].
    #[allow(clippy::cast_possible_truncation)]
    pub fn perlin3(x: f64, y: f64, z: f64) -> f64 {
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let zi = z.floor() as i32;
        let xf = x - x.floor();
        let yf = y - y.floor();
        let zf = z - z.floor();
        let u = fade(xf);
        let v = fade(yf);
        let w = fade(zf);

        let a  = PERM[perm_idx(xi)] as i32 + yi;
        let aa = PERM[perm_idx(a)] as i32 + zi;
        let ab = PERM[perm_idx(a + 1)] as i32 + zi;
        let b  = PERM[perm_idx(xi + 1)] as i32 + yi;
        let ba = PERM[perm_idx(b)] as i32 + zi;
        let bb = PERM[perm_idx(b + 1)] as i32 + zi;

        lerp(w,
            lerp(v,
                lerp(u,
                    grad3(PERM[perm_idx(aa)], xf, yf, zf),
                    grad3(PERM[perm_idx(ba)], xf - 1.0, yf, zf),
                ),
                lerp(u,
                    grad3(PERM[perm_idx(ab)], xf, yf - 1.0, zf),
                    grad3(PERM[perm_idx(bb)], xf - 1.0, yf - 1.0, zf),
                ),
            ),
            lerp(v,
                lerp(u,
                    grad3(PERM[perm_idx(aa + 1)], xf, yf, zf - 1.0),
                    grad3(PERM[perm_idx(ba + 1)], xf - 1.0, yf, zf - 1.0),
                ),
                lerp(u,
                    grad3(PERM[perm_idx(ab + 1)], xf, yf - 1.0, zf - 1.0),
                    grad3(PERM[perm_idx(bb + 1)], xf - 1.0, yf - 1.0, zf - 1.0),
                ),
            ),
        )
    }

    /// Fractal Brownian Motion using 2D Perlin noise.
    /// Lacunarity = 2.0, gain = 0.5.
    pub fn fbm(x: f64, y: f64, octaves: u32) -> f64 {
        let octaves = octaves.clamp(1, 10);
        let mut sum = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = 1.0;
        let mut max_amp = 0.0;

        for _ in 0..octaves {
            sum += amplitude * perlin2(x * frequency, y * frequency);
            max_amp += amplitude;
            amplitude *= 0.5;
            frequency *= 2.0;
        }

        sum / max_amp
    }

    /// 2D Worley (cellular) noise, returns [0, 1].
    /// Returns the distance to the nearest cell point.
    #[allow(clippy::cast_possible_truncation)]
    pub fn worley2(x: f64, y: f64) -> f64 {
        let ix = x.floor() as i32;
        let iy = y.floor() as i32;
        let fx = x - x.floor();
        let fy = y - y.floor();

        let mut min_dist = f64::MAX;

        for dy in -1..=1 {
            for dx in -1..=1 {
                // Deterministic point position within neighbor cell
                let cell_x = ix + dx;
                let cell_y = iy + dy;
                let h = PERM[perm_idx(PERM[perm_idx(cell_x)] as i32 + cell_y)];
                let px = dx as f64 + (h as f64 / 255.0) - fx;
                let py = dy as f64 + (PERM[perm_idx(h as i32 + 1)] as f64 / 255.0) - fy;
                let dist = px * px + py * py;
                if dist < min_dist {
                    min_dist = dist;
                }
            }
        }

        min_dist.sqrt().min(1.0)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::dsl::compiler::compile;
    use crate::dsl::lexer::lex;
    use crate::dsl::parser::parse;
    use crate::dsl::typeck::type_check;

    fn run(src: &str) -> Color {
        run_with_ctx(src, 0.5, 0, 10)
    }

    fn run_with_ctx(src: &str, t: f64, pixel: usize, pixels: usize) -> Color {
        let tokens = lex(src).unwrap();
        let script = parse(tokens).unwrap();
        let typed = type_check(&script).unwrap();
        let compiled = compile(&typed).unwrap();

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

        execute(&compiled, &ctx)
    }

    #[test]
    fn solid_red() {
        let color = run("rgb(1.0, 0.0, 0.0)");
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn solid_white() {
        let color = run("rgb(1.0, 1.0, 1.0)");
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 255);
        assert_eq!(color.b, 255);
    }

    #[test]
    fn color_literal() {
        let color = run("#ff8000");
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn let_binding() {
        let color = run("let v = 0.5\nrgb(v, v, v)");
        assert_eq!(color.r, 128);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 128);
    }

    #[test]
    fn time_variable() {
        // At t=0.5, sin(0.5 * PI) ≈ 1.0
        let color = run("let s = sin(t * PI)\nrgb(s, s, s)");
        assert!(color.r > 250, "Expected near-white, got r={}", color.r);
    }

    #[test]
    fn pixel_variable() {
        // pixel 0 of 10 → pos = 0.0
        let c0 = run_with_ctx("rgb(pos, 0.0, 0.0)", 0.0, 0, 10);
        assert_eq!(c0.r, 0);

        // pixel 9 of 10 → pos = 1.0
        let c9 = run_with_ctx("rgb(pos, 0.0, 0.0)", 0.0, 9, 10);
        assert_eq!(c9.r, 255);
    }

    #[test]
    fn if_else() {
        let color_true = run_with_ctx("if t > 0.3 {\nrgb(1.0, 0.0, 0.0)\n} else {\nrgb(0.0, 0.0, 1.0)\n}", 0.5, 0, 10);
        assert_eq!(color_true.r, 255);
        assert_eq!(color_true.b, 0);

        let color_false = run_with_ctx("if t > 0.3 {\nrgb(1.0, 0.0, 0.0)\n} else {\nrgb(0.0, 0.0, 1.0)\n}", 0.1, 0, 10);
        assert_eq!(color_false.r, 0);
        assert_eq!(color_false.b, 255);
    }

    #[test]
    fn math_operations() {
        // clamp(2.0, 0.0, 1.0) = 1.0
        let color = run("let x = clamp(2.0, 0.0, 1.0)\nrgb(x, x, x)");
        assert_eq!(color.r, 255);

        // abs(-0.5) = 0.5
        let color2 = run("let x = abs(-0.5)\nrgb(x, x, x)");
        assert_eq!(color2.r, 128);
    }

    #[test]
    fn hsv_color() {
        // HSV(0, 1, 1) = pure red
        let color = run("hsv(0.0, 1.0, 1.0)");
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn color_scale() {
        let color = run("rgb(1.0, 1.0, 1.0).scale(0.5)");
        assert_eq!(color.r, 128);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 128);
    }

    #[test]
    fn hash_deterministic() {
        let c1 = run("let h = hash(1.0, 2.0)\nrgb(h, h, h)");
        let c2 = run("let h = hash(1.0, 2.0)\nrgb(h, h, h)");
        assert_eq!(c1.r, c2.r);
        assert_eq!(c1.g, c2.g);
    }

    #[test]
    fn complex_rainbow() {
        // Rainbow effect: hue varies with position
        let c0 = run_with_ctx("hsv(pos * 360.0, 1.0, 1.0)", 0.0, 0, 10);
        let c5 = run_with_ctx("hsv(pos * 360.0, 1.0, 1.0)", 0.0, 5, 10);
        // Different pixels should give different colors
        assert_ne!(c0, c5);
    }

    #[test]
    fn gradient_param() {
        let src = "param palette: gradient = #000000, #ffffff\npalette(t)";
        let tokens = lex(src).unwrap();
        let script = parse(tokens).unwrap();
        let typed = type_check(&script).unwrap();
        let compiled = compile(&typed).unwrap();

        let gradient = ColorGradient::two_color(Color::BLACK, Color::WHITE);
        let gradients: Vec<Option<&ColorGradient>> = vec![Some(&gradient)];

        let ctx = VmContext {
            t: 0.5,
            pixel: 0,
            pixels: 10,
            pos: 0.0,
            pos2d: (0.0, 0.0),
            param_values: &[0.0], // gradient params don't use this slot
            abs_t: 0.0,
            gradients: &gradients,
            curves: &[],
            colors: &[],
            paths: &[],
        };

        let color = execute(&compiled, &ctx);
        // At t=0.5, gradient should be ~mid-gray
        assert!((color.r as i16 - 127).abs() <= 2, "Expected ~127, got r={}", color.r);
    }

    #[test]
    fn user_function() {
        let color = run("fn half(x: float) -> float {\nx * 0.5\n}\nlet v = half(1.0)\nrgb(v, v, v)");
        assert_eq!(color.r, 128);
    }

    #[test]
    fn color_param() {
        let src = "param bg: color = #ff0000\nbg";
        let tokens = lex(src).unwrap();
        let script = parse(tokens).unwrap();
        let typed = type_check(&script).unwrap();
        let compiled = compile(&typed).unwrap();

        let colors = vec![Some(Color::rgb(0, 255, 0))]; // override to green
        let ctx = VmContext {
            t: 0.0,
            pixel: 0,
            pixels: 1,
            pos: 0.0,
            pos2d: (0.0, 0.0),
            param_values: &[0.0],
            abs_t: 0.0,
            gradients: &[],
            curves: &[],
            colors: &colors,
            paths: &[],
        };
        let color = execute(&compiled, &ctx);
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 255);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn enum_param() {
        let src = "enum Mode { Red, Green, Blue }\nparam mode: Mode = Red\nif mode == Mode.Red {\nrgb(1.0, 0.0, 0.0)\n} else {\nrgb(0.0, 1.0, 0.0)\n}";
        let tokens = lex(src).unwrap();
        let script = parse(tokens).unwrap();
        let typed = type_check(&script).unwrap();
        let compiled = compile(&typed).unwrap();

        // mode = 0 (Red)
        let ctx = VmContext {
            t: 0.0,
            pixel: 0,
            pixels: 1,
            pos: 0.0,
            pos2d: (0.0, 0.0),
            param_values: &[0.0],
            abs_t: 0.0,
            gradients: &[],
            curves: &[],
            colors: &[],
            paths: &[],
        };
        let color = execute(&compiled, &ctx);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);

        // mode = 1 (Green)
        let ctx2 = VmContext {
            t: 0.0,
            pixel: 0,
            pixels: 1,
            pos: 0.0,
            pos2d: (0.0, 0.0),
            param_values: &[1.0],
            abs_t: 0.0,
            gradients: &[],
            curves: &[],
            colors: &[],
            paths: &[],
        };
        let color2 = execute(&compiled, &ctx2);
        assert_eq!(color2.r, 0);
        assert_eq!(color2.g, 255);
    }

    // ── Phase 6: Validation tests ────────────────────────────────
    // Compare DSL script output with native Rust effects pixel-for-pixel.

    #[test]
    fn validate_solid_red_matches_native() {
        // DSL solid red using float params for r/g/b
        let src = r#"
param r: float(0.0, 1.0) = 1.0
param g: float(0.0, 1.0) = 0.0
param b: float(0.0, 1.0) = 0.0
rgb(r, g, b)
"#;
        let tokens = lex(src).unwrap();
        let script = parse(tokens).unwrap();
        let typed = type_check(&script).unwrap();
        let compiled = compile(&typed).unwrap();

        // Native solid: Color::rgb(255, 0, 0)
        let native = Color::rgb(255, 0, 0);

        for pixel in 0..10 {
            let pos = if pixel > 0 { pixel as f64 / 9.0 } else { 0.0 };
            let ctx = VmContext {
                t: 0.5,
                pixel,
                pixels: 10,
                pos,
                pos2d: (pos, 0.0),
                param_values: &[1.0, 0.0, 0.0], // r=1.0, g=0.0, b=0.0
                abs_t: 0.0,
                gradients: &[],
                curves: &[],
                colors: &[],
                paths: &[],
            };
            let dsl_color = execute(&compiled, &ctx);
            assert_eq!(dsl_color.r, native.r, "pixel {pixel}: r mismatch");
            assert_eq!(dsl_color.g, native.g, "pixel {pixel}: g mismatch");
            assert_eq!(dsl_color.b, native.b, "pixel {pixel}: b mismatch");
        }
    }

    #[test]
    fn validate_solid_literal_matches_native() {
        // DSL solid using literal color (simpler, no params needed)
        let dsl_src = "rgb(1.0, 0.0, 0.0)";
        let native = Color::rgb(255, 0, 0);

        for pixel in 0..10 {
            let dsl_color = run_with_ctx(dsl_src, 0.5, pixel, 10);
            assert_eq!(dsl_color, native, "pixel {pixel}: color mismatch");
        }
    }

    #[test]
    fn validate_rainbow_matches_native() {
        // Native rainbow: spatial = pixel_index / pixel_count * spread (divides by total, not total-1)
        // hue = ((t * speed + spatial) * 360.0) % 360.0
        //
        // DSL must use `pixel * 1.0 / pixels` (not `pos`, which is pixel/(pixels-1))
        let dsl_src = r#"
param speed: float(0.1, 20.0) = 1.0
param spread: float(0.1, 10.0) = 1.0
let spatial = pixel * 1.0 / pixels * spread
let hue = (t * speed + spatial) * 360.0 % 360.0
hsv(hue, 1.0, 1.0)
"#;
        let tokens = lex(dsl_src).unwrap();
        let script = parse(tokens).unwrap();
        let typed = type_check(&script).unwrap();
        let compiled = compile(&typed).unwrap();

        let pixel_count = 10usize;
        let test_times = [0.0, 0.25, 0.5, 0.75, 1.0];

        for &t in &test_times {
            for pixel in 0..pixel_count {
                // Native calculation
                let spatial_native = if pixel_count > 1 {
                    (pixel as f64) / (pixel_count as f64) * 1.0
                } else {
                    0.0
                };
                let hue_native = ((t * 1.0 + spatial_native) * 360.0) % 360.0;
                let native = Color::from_hsv(hue_native, 1.0, 1.0);

                // DSL calculation
                let pos = if pixel_count > 1 { pixel as f64 / (pixel_count - 1) as f64 } else { 0.0 };
                let ctx = VmContext {
                    t,
                    pixel,
                    pixels: pixel_count,
                    pos,
                    pos2d: (pos, 0.0),
                    param_values: &[1.0, 1.0], // speed=1.0, spread=1.0
                    abs_t: 0.0,
                    gradients: &[],
                    curves: &[],
                    colors: &[],
                    paths: &[],
                };
                let dsl_color = execute(&compiled, &ctx);

                // Allow ±1 tolerance due to floating point → u8 rounding
                assert!(
                    (dsl_color.r as i16 - native.r as i16).abs() <= 1
                    && (dsl_color.g as i16 - native.g as i16).abs() <= 1
                    && (dsl_color.b as i16 - native.b as i16).abs() <= 1,
                    "t={t}, pixel={pixel}: DSL=({},{},{}) native=({},{},{})",
                    dsl_color.r, dsl_color.g, dsl_color.b,
                    native.r, native.g, native.b
                );
            }
        }
    }

    #[test]
    fn validate_strobe_matches_native() {
        // Native strobe: phase = (t * rate).fract(); if phase < duty_cycle { color } else { black }
        // DSL equivalent:
        let dsl_src = r#"
param rate: float(1.0, 50.0) = 10.0
param duty_cycle: float(0.0, 1.0) = 0.5
let phase = fract(t * rate)
if phase < duty_cycle {
    rgb(1.0, 1.0, 1.0)
} else {
    rgb(0.0, 0.0, 0.0)
}
"#;
        let tokens = lex(dsl_src).unwrap();
        let script = parse(tokens).unwrap();
        let typed = type_check(&script).unwrap();
        let compiled = compile(&typed).unwrap();

        let rate = 10.0f64;
        let duty_cycle = 0.5f64;
        let test_times = [0.0, 0.02, 0.05, 0.08, 0.12, 0.25, 0.5, 0.75, 0.99];

        for &t in &test_times {
            // Native
            let phase = (t * rate).fract();
            let native = if phase < duty_cycle { Color::WHITE } else { Color::BLACK };

            // DSL
            let ctx = VmContext {
                t,
                pixel: 0,
                pixels: 1,
                pos: 0.0,
                pos2d: (0.0, 0.0),
                param_values: &[rate, duty_cycle],
                abs_t: 0.0,
                gradients: &[],
                curves: &[],
                colors: &[],
                paths: &[],
            };
            let dsl_color = execute(&compiled, &ctx);

            assert_eq!(
                dsl_color, native,
                "t={t}: DSL=({},{},{}) native=({},{},{})",
                dsl_color.r, dsl_color.g, dsl_color.b,
                native.r, native.g, native.b
            );
        }
    }

    // ── Issue #69: Whitespace-agnostic if/else ──────────────────

    #[test]
    fn if_else_with_newlines_between() {
        let color = run("if t > 0.3 {\nrgb(1.0, 0.0, 0.0)\n}\n\nelse {\nrgb(0.0, 0.0, 1.0)\n}");
        assert_eq!(color.r, 255);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn if_else_with_blank_lines() {
        let color = run_with_ctx("if t > 0.3 {\nrgb(1.0, 0.0, 0.0)\n}\n\n\n\nelse {\nrgb(0.0, 0.0, 1.0)\n}", 0.1, 0, 10);
        assert_eq!(color.r, 0);
        assert_eq!(color.b, 255);
    }

    // ── Issue #73: Ternary operator ─────────────────────────────

    #[test]
    fn ternary_true_branch() {
        let color = run("t > 0.3 ? rgb(1.0, 0.0, 0.0) : rgb(0.0, 0.0, 1.0)");
        assert_eq!(color.r, 255);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn ternary_false_branch() {
        let color = run_with_ctx("t > 0.8 ? rgb(1.0, 0.0, 0.0) : rgb(0.0, 0.0, 1.0)", 0.5, 0, 10);
        assert_eq!(color.r, 0);
        assert_eq!(color.b, 255);
    }

    #[test]
    fn ternary_nested() {
        // t=0.5: first condition true
        let color = run("t > 0.3 ? rgb(1.0, 0.0, 0.0) : t > 0.1 ? rgb(0.0, 1.0, 0.0) : rgb(0.0, 0.0, 1.0)");
        assert_eq!(color.r, 255);
    }

    // ── Issue #72: Power operator ───────────────────────────────

    #[test]
    fn power_operator() {
        // 2.0 ** 3.0 = 8.0, clamped to 1.0 for color
        let color = run("let x = 2.0 ** 3.0\nlet n = x / 8.0\nrgb(n, 0.0, 0.0)");
        assert_eq!(color.r, 255);
    }

    #[test]
    fn power_right_associative() {
        // 2 ** 3 ** 2 = 2 ** 9 = 512, normalized to check it's 512 not 64
        let color = run("let x = 2.0 ** 3.0 ** 2.0\nlet n = x / 512.0\nrgb(n, 0.0, 0.0)");
        assert_eq!(color.r, 255);
    }

    // ── Issue #70: Switch/case ──────────────────────────────────

    #[test]
    fn switch_enum_first_case() {
        let src = "enum Mode { Red, Green, Blue }\nparam mode: Mode = Red\nswitch mode {\ncase Mode.Red => rgb(1.0, 0.0, 0.0)\ncase Mode.Green => rgb(0.0, 1.0, 0.0)\ndefault => rgb(0.0, 0.0, 1.0)\n}";
        let tokens = lex(src).unwrap();
        let script = parse(tokens).unwrap();
        let typed = type_check(&script).unwrap();
        let compiled = compile(&typed).unwrap();

        let ctx = VmContext {
            t: 0.0, pixel: 0, pixels: 1, pos: 0.0, pos2d: (0.0, 0.0),
            param_values: &[0.0], // Red = 0
            abs_t: 0.0, gradients: &[], curves: &[], colors: &[], paths: &[],
        };
        let color = execute(&compiled, &ctx);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
    }

    #[test]
    fn switch_enum_second_case() {
        let src = "enum Mode { Red, Green, Blue }\nparam mode: Mode = Red\nswitch mode {\ncase Mode.Red => rgb(1.0, 0.0, 0.0)\ncase Mode.Green => rgb(0.0, 1.0, 0.0)\ndefault => rgb(0.0, 0.0, 1.0)\n}";
        let tokens = lex(src).unwrap();
        let script = parse(tokens).unwrap();
        let typed = type_check(&script).unwrap();
        let compiled = compile(&typed).unwrap();

        let ctx = VmContext {
            t: 0.0, pixel: 0, pixels: 1, pos: 0.0, pos2d: (0.0, 0.0),
            param_values: &[1.0], // Green = 1
            abs_t: 0.0, gradients: &[], curves: &[], colors: &[], paths: &[],
        };
        let color = execute(&compiled, &ctx);
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 255);
    }

    #[test]
    fn switch_default_fallthrough() {
        let src = "enum Mode { Red, Green, Blue }\nparam mode: Mode = Red\nswitch mode {\ncase Mode.Red => rgb(1.0, 0.0, 0.0)\ncase Mode.Green => rgb(0.0, 1.0, 0.0)\ndefault => rgb(0.0, 0.0, 1.0)\n}";
        let tokens = lex(src).unwrap();
        let script = parse(tokens).unwrap();
        let typed = type_check(&script).unwrap();
        let compiled = compile(&typed).unwrap();

        let ctx = VmContext {
            t: 0.0, pixel: 0, pixels: 1, pos: 0.0, pos2d: (0.0, 0.0),
            param_values: &[2.0], // Blue = 2 (falls to default)
            abs_t: 0.0, gradients: &[], curves: &[], colors: &[], paths: &[],
        };
        let color = execute(&compiled, &ctx);
        assert_eq!(color.r, 0);
        assert_eq!(color.b, 255);
    }

    // ── Issue #74: Easing functions ─────────────────────────────

    #[test]
    fn ease_in_endpoints() {
        // ease_in(0) = 0, ease_in(1) = 1
        let c0 = run_with_ctx("let x = ease_in(t)\nrgb(x, x, x)", 0.0, 0, 1);
        assert_eq!(c0.r, 0);
        let c1 = run_with_ctx("let x = ease_in(t)\nrgb(x, x, x)", 1.0, 0, 1);
        assert_eq!(c1.r, 255);
    }

    #[test]
    fn ease_out_endpoints() {
        let c0 = run_with_ctx("let x = ease_out(t)\nrgb(x, x, x)", 0.0, 0, 1);
        assert_eq!(c0.r, 0);
        let c1 = run_with_ctx("let x = ease_out(t)\nrgb(x, x, x)", 1.0, 0, 1);
        assert_eq!(c1.r, 255);
    }

    #[test]
    fn ease_in_out_endpoints() {
        let c0 = run_with_ctx("let x = ease_in_out(t)\nrgb(x, x, x)", 0.0, 0, 1);
        assert_eq!(c0.r, 0);
        let c1 = run_with_ctx("let x = ease_in_out(t)\nrgb(x, x, x)", 1.0, 0, 1);
        assert_eq!(c1.r, 255);
    }

    #[test]
    fn ease_in_cubic_midpoint() {
        // ease_in_cubic(0.5) = 0.125
        let c = run_with_ctx("let x = ease_in_cubic(t)\nrgb(x, x, x)", 0.5, 0, 1);
        assert_eq!(c.r, 32, "ease_in_cubic(0.5) ≈ 0.125 → 32, got {}", c.r);
    }

    #[test]
    fn ease_out_cubic_midpoint() {
        // ease_out_cubic(0.5) = (0.5-1)^3 + 1 = -0.125 + 1 = 0.875
        let c = run_with_ctx("let x = ease_out_cubic(t)\nrgb(x, x, x)", 0.5, 0, 1);
        assert_eq!(c.r, 223, "ease_out_cubic(0.5) ≈ 0.875 → 223, got {}", c.r);
    }

    #[test]
    fn ease_in_out_cubic_symmetry() {
        // ease_in_out_cubic should be symmetric: f(0.25) + f(0.75) ≈ 1.0
        let c_lo = run_with_ctx("let x = ease_in_out_cubic(t)\nrgb(x, x, x)", 0.25, 0, 1);
        let c_hi = run_with_ctx("let x = ease_in_out_cubic(t)\nrgb(x, x, x)", 0.75, 0, 1);
        let sum = c_lo.r as u16 + c_hi.r as u16;
        assert!((sum as i16 - 255).abs() <= 1, "symmetry: {} + {} should ≈ 255", c_lo.r, c_hi.r);
    }

    // ── Issue #77: Deterministic randomness ─────────────────────

    #[test]
    fn hash3_deterministic() {
        let c1 = run("let h = hash3(1.0, 2.0, 3.0)\nrgb(h, h, h)");
        let c2 = run("let h = hash3(1.0, 2.0, 3.0)\nrgb(h, h, h)");
        assert_eq!(c1.r, c2.r);
        // Different inputs should give different output
        let c3 = run("let h = hash3(1.0, 2.0, 4.0)\nrgb(h, h, h)");
        assert_ne!(c1.r, c3.r, "Different seed should give different value");
    }

    #[test]
    fn random_in_unit_range() {
        // random returns hash(x, 0) which is in [0, 1]
        let c = run("let r = random(42.0)\nrgb(r, r, r)");
        assert!(c.r > 0 && c.r < 255, "random should produce value in (0, 1), got {}", c.r);
    }

    #[test]
    fn random_range_within_bounds() {
        // random_range(0.2, 0.8, x) should be in [0.2, 0.8] → pixel [51, 204]
        let c = run("let r = random_range(0.2, 0.8, 42.0)\nrgb(r, r, r)");
        assert!(c.r >= 51 && c.r <= 204, "random_range(0.2, 0.8, x) should be in [51, 204], got {}", c.r);
    }

    // ── Issue #78: Noise functions ──────────────────────────────

    #[test]
    fn noise1_deterministic() {
        let c1 = run("let n = abs(noise(5.5))\nrgb(n, n, n)");
        let c2 = run("let n = abs(noise(5.5))\nrgb(n, n, n)");
        assert_eq!(c1.r, c2.r);
    }

    #[test]
    fn noise2_varies_with_input() {
        // Use non-integer coordinates to avoid zero crossings
        let c1 = run("let n = abs(noise2(1.3, 2.7))\nrgb(n, n, n)");
        let c2 = run("let n = abs(noise2(4.6, 8.1))\nrgb(n, n, n)");
        // Different inputs should produce different outputs
        assert_ne!(c1.r, c2.r, "noise2 with different inputs should differ");
    }

    #[test]
    fn noise3_deterministic() {
        let c1 = run("let n = abs(noise3(1.0, 2.0, 3.0))\nrgb(n, n, n)");
        let c2 = run("let n = abs(noise3(1.0, 2.0, 3.0))\nrgb(n, n, n)");
        assert_eq!(c1.r, c2.r);
    }

    #[test]
    fn fbm_more_detail_than_single_octave() {
        // FBM with 1 octave is just perlin2; more octaves add detail
        let c1 = run("let n = abs(fbm(3.5, 7.2, 1.0))\nrgb(n, n, n)");
        let c4 = run("let n = abs(fbm(3.5, 7.2, 4.0))\nrgb(n, n, n)");
        // With different octave counts, results should differ
        assert_ne!(c1.r, c4.r, "fbm with 1 vs 4 octaves should differ");
    }

    #[test]
    fn worley2_in_unit_range() {
        // worley2 returns [0, 1], so the color channel should be a valid value
        let c = run("let n = worley2(5.5, 3.2)\nrgb(n, n, n)");
        // Value should be non-zero (not at a cell center) and less than 1.0
        assert!(c.r > 0, "worley2 should return non-zero for most inputs");
    }

    #[test]
    fn worley2_deterministic() {
        let c1 = run("let n = worley2(5.5, 3.2)\nrgb(n, n, n)");
        let c2 = run("let n = worley2(5.5, 3.2)\nrgb(n, n, n)");
        assert_eq!(c1.r, c2.r);
    }

    #[test]
    fn noise_at_integer_boundaries() {
        // Perlin noise at integer coordinates should be 0 (or very close)
        let c = run("let n = noise(0.0)\nlet v = abs(n)\nrgb(v, v, v)");
        assert!(c.r <= 1, "noise at integer boundary should be ~0, got {}", c.r);
    }
}
