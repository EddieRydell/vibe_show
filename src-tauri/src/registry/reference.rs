use std::fmt::Write;

use crate::dsl::builtins::{BUILTINS, IMPLICIT_VARS};

/// Generate a markdown table of built-in functions, grouped by category.
fn builtin_functions_table() -> String {
    let mut out = String::new();
    out.push_str("## Built-in Functions\n\n");

    // Ordered list of (category_key, display_title)
    let categories: &[(&str, &str)] = &[
        ("math", "Math"),
        ("color", "Color Constructors"),
        ("vec2", "Vec2"),
        ("hash", "Hash / Random"),
        ("easing", "Easing"),
        ("noise", "Noise"),
    ];

    for &(cat_key, cat_title) in categories {
        let fns: Vec<_> = BUILTINS.iter().filter(|b| b.category == cat_key).collect();
        if fns.is_empty() {
            continue;
        }

        let _ = writeln!(out, "### {cat_title}");
        out.push_str("| Function | Description |\n");
        out.push_str("|----------|-------------|\n");

        for f in &fns {
            let params_str: Vec<&str> = f.params.iter()
                .map(|(name, _ty)| *name)
                .collect();
            let _ = writeln!(out,
                "| `{}({})` | {} |",
                f.name,
                params_str.join(", "),
                f.description,
            );
        }
        out.push('\n');
    }

    out
}

/// Generate a markdown table of implicit variables.
fn implicit_variables_table() -> String {
    let mut out = String::new();
    out.push_str("## Implicit Variables\n");
    out.push_str("| Variable | Type  | Description |\n");
    out.push_str("|----------|-------|-------------|\n");

    for &(name, ref ty, _, desc) in IMPLICIT_VARS {
        let ty_str = match ty {
            crate::dsl::ast::TypeName::Float => "float",
            crate::dsl::ast::TypeName::Vec2 => "vec2",
            _ => "?",
        };
        let _ = writeln!(out, "| `{name}` | {ty_str} | {desc} |");
    }
    out.push('\n');
    out
}

/// Generate the complete DSL language reference from the actual implementation.
pub fn dsl_reference() -> String {
    let mut out = String::new();

    // ── Hand-written sections ───────────────────────────────────
    out.push_str(r#"# VibeLights DSL Reference

## Script Structure
```
@name "My Effect"       -- metadata: display name
@spatial true           -- enable 2D position data (pos2d variable)

-- Type definitions (before params)
enum Mode { Pulse, Wave, Sparkle }
flags Features { Glow, Trail, Fade }

-- Parameters (user-configurable from the UI)
param speed: float(0.1, 10.0) = 2.0
param color1: color = #ff0000
param grad: gradient = #ff0000, #0000ff
param curve1: curve = 0:0, 0.5:1, 1:0
param mode: Mode = Pulse
param features: Features = Glow | Trail
param count: int(1, 100) = 10
param enabled: bool = true

-- Local variables
let phase = t * speed * TAU
let x = sin(phase) * 0.5 + 0.5

-- Functions
fn pulse(center: float, width: float) -> float {
    let d = abs(pos - center)
    smoothstep(0.0, width, d)
}

-- Conditionals (if / else if / else)
if mode == Mode.Pulse {
    color1.scale(pulse(0.5, 0.3))
} else if mode == Mode.Wave {
    grad(pos)
} else {
    grad(t)
}

-- Last expression is the output color
```

## Comments
```
// C-style line comment
-- Lua-style line comment (also valid)
```

"#);

    // ── Auto-generated from IMPLICIT_VARS ───────────────────────
    out.push_str(&implicit_variables_table());

    // ── Hand-written types/operators ────────────────────────────
    out.push_str(r"## Types
- `float` — 64-bit floating point
- `int` — 32-bit integer
- `bool` — true/false
- `color` — RGBA color (r, g, b, a fields, 0-255)
- `vec2` — 2D vector (x, y fields)
- `gradient` — color gradient (callable: `grad(position)`)
- `curve` — timing curve (callable: `curve1(x)`)
- `path` — motion path (bare ident → Vec2 at abs_t, callable: `orb(time)` → Vec2)

## Operators

### Arithmetic
| Operator | Description |
|----------|-------------|
| `+`  | Addition |
| `-`  | Subtraction (or unary negation) |
| `*`  | Multiplication |
| `/`  | Division (returns 0 on divide by zero) |
| `%`  | Modulo (returns 0 on divide by zero) |

### Comparison (→ bool)
| Operator | Description |
|----------|-------------|
| `==` | Equal |
| `!=` | Not equal |
| `<`  | Less than |
| `>`  | Greater than |
| `<=` | Less than or equal |
| `>=` | Greater than or equal |

### Logical
| Operator | Description |
|----------|-------------|
| `&&` | Logical AND |
| `\|\|` | Logical OR |
| `!`  | Logical NOT (unary) |
| `&`  | Flag test (bitwise AND for flags) |

## Parameter Types
- `float(min, max)` — float slider with range
- `int(min, max)` — integer slider with range
- `bool` — checkbox
- `color` — color picker (default: hex like `#ff0000`)
- `gradient` — gradient editor (default: comma-separated hex `#ff0000, #0000ff`)
- `curve` — curve editor (default: comma-separated x:y pairs `0:0, 0.5:1, 1:0`)
- `path` — motion path selector (no default; bound at runtime via PathRef)
- `EnumName` — dropdown from defined enum
- `FlagsName` — multi-select from defined flags

");

    // ── Auto-generated from BUILTINS ────────────────────────────
    out.push_str(&builtin_functions_table());

    // ── Hand-written color/vec2 operations & rest ───────────────
    out.push_str(r#"### Color Operations
| Operation | Description |
|-----------|-------------|
| `c.r`, `c.g`, `c.b`, `c.a` | Channel access (0.0-1.0) |
| `c.scale(f)` | Multiply RGB by float, returns new color |
| Gradient call: `grad(0.5)` | Evaluate gradient at position [0.0, 1.0] |

### Vec2 Operations
| Operation | Description |
|-----------|-------------|
| `v.x`, `v.y` | Component access |

## Control Flow

### If / Else If / Else
```
if condition {
    expr1
} else if other_condition {
    expr2
} else {
    expr3
}
```
If-expressions return a value (the last expression in the taken branch).

### Boolean Logic
```
-- Combine conditions with && and ||
if speed > 1.0 && enabled {
    color1
} else {
    #000000
}

-- Negate with !
if !enabled {
    #000000
} else {
    color1
}
```

## Enum & Flags Usage
```
enum Direction { Left, Right, Both }
param dir: Direction = Left

if dir == Direction.Left { ... }
```

```
flags Options { Sparkle, Glow, Pulse }
param opts: Options = Sparkle | Glow

-- Test individual flags with &
if opts & Options.Sparkle {
    -- sparkle is enabled
}

-- Combine flag tests with && and ||
if opts & Options.Sparkle && opts & Options.Glow {
    -- both enabled
}
```

## User-Defined Functions
```
fn function_name(param1: type, param2: type) -> return_type {
    -- function body (last expression is the return value)
    expr
}
```
Supported types in signatures: `float`, `int`, `bool`, `color`, `vec2`, `gradient`, `curve`, `path`.

## Examples

### Simple: Pulsing Color
```
@name "Pulse"
param color1: color = #ff0000
param speed: float(0.1, 10.0) = 2.0

let brightness = sin(t * speed * TAU) * 0.5 + 0.5
color1.scale(brightness)
```

### Medium: Gradient Chase
```
@name "Gradient Chase"
param grad: gradient = #ff0000, #00ff00, #0000ff
param speed: float(0.1, 5.0) = 1.0
param width: float(0.1, 1.0) = 0.3

let head = fract(t * speed)
let d = abs(pos - head)
let d2 = min(d, 1.0 - d)
let brightness = smoothstep(0.0, width, d2)
grad(pos).scale(brightness)
```

### Advanced: Rainbow Sparkles
```
@name "Rainbow Sparkles"
param colors: gradient = #ff0000, #ff7f00, #ffff00, #00ff00, #0000ff, #4b0082, #9400d3
param density: float(0.01, 1.0) = 0.15
param speed: float(0.1, 10.0) = 2.0
param sparkle_duration: float(0.05, 0.5) = 0.15
param brightness: float(0.0, 2.0) = 1.0
param sharpness: float(0.0, 1.0) = 0.7

let time_phase = t * speed
let sparkle_phase = hash(pixel, 456.0)
let sparkle_time = fract(time_phase + sparkle_phase)
let time_seed = floor(time_phase * 10.0)
let is_sparkling = hash(pixel, time_seed) < density
let pulse_pos = sparkle_time / sparkle_duration
let pulse = 1.0 - pow(pulse_pos, 2.0)
let active = if sparkle_time < sparkle_duration && is_sparkling { 1.0 } else { 0.0 }
let final_brightness = clamp(pulse * active * brightness, 0.0, 1.0)
let color_position = fract(hash(pixel, 789.0) + t * 0.5)
colors(color_position).scale(final_brightness)
```

### Advanced: Spatial Burst
```
@name "Burst"
@spatial true
param color1: color = #ffffff
param speed: float(0.5, 5.0) = 2.0
param center_x: float(0.0, 1.0) = 0.5
param center_y: float(0.0, 1.0) = 0.5

let center = vec2(center_x, center_y)
let d = distance(pos2d, center)
let wave = sin((d - t * speed) * TAU * 3.0) * 0.5 + 0.5
let falloff = smoothstep(0.0, 0.8, d)
color1.scale(wave * falloff)
```
"#);

    out
}

/// Light show design best practices.
pub fn design_guide() -> String {
    use crate::model::BlendMode;

    let mut out = String::from(
        "# Light Show Design Guide\n\
         \n\
         ## Layering Strategy\n\
         - **Base layer**: Ambient fills (Solid, Gradient, or slow Rainbow) on the bottom track with low opacity\n\
         - **Accent layers**: Movement effects (Chase, Wipe, Fade) on higher tracks\n\
         - **Top layer**: Highlights (Strobe, Twinkle) with short durations or low opacity\n\
         - Use blend modes to combine: Add for glow effects, Multiply for darkening, Screen for brightening, Alpha for overlays\n\
         \n\
         ## Blend Mode Guide\n\
         | Mode | Use Case |\n\
         |------|----------|\n",
    );
    for mode in BlendMode::all() {
        let _ = writeln!(out, "| {mode:?} | {} |", mode.description());
    }
    out.push_str(
        "\n## Beat Sync Techniques\n\
         - Use `get_beats_in_range` to get exact beat timestamps\n\
         - Place Strobe effects on beats for impact\n\
         - Use Fade effects spanning 2-4 beats for rhythmic breathing\n\
         - Chase effects with speed matching BPM: `speed = tempo / 60`\n\
         - For half-time feel, use `speed = tempo / 120`\n\
         \n\
         ## Section-Based Design\n\
         1. Call `get_sections` to understand the song structure\n\
         2. Design each section with a distinct visual identity:\n\
            - **Intro**: Slow builds, dark-to-bright, single colors\n\
            - **Verse**: Moderate movement, muted palette\n\
            - **Chorus**: Full brightness, fast movement, rich gradients\n\
            - **Bridge**: Contrasting colors/patterns from chorus\n\
            - **Outro**: Mirror intro, bright-to-dark fade\n\
         \n\
         ## Color Theory for Lights\n\
         - **Complementary**: High contrast (red+cyan, blue+orange) — great for chorus\n\
         - **Analogous**: Harmonious (blue+purple+pink) — great for verse\n\
         - **Monochromatic**: Single hue, varied brightness — great for builds\n\
         - **Warm vs Cool**: Warm (red/orange/yellow) feels energetic; cool (blue/purple/cyan) feels calm\n\
         - Use the gradient library to define color palettes per section\n\
         \n\
         ## Common Patterns\n\
         - **Energy Build**: Increase brightness, speed, and color saturation from verse to chorus\n\
         - **Drop Impact**: Strobe + full-white flash at drop, then immediate Chase or Wipe\n\
         - **Breathing**: Fade effect with sine wave curve, 2-4 beat period\n\
         - **Sparkle Layer**: Twinkle on top track with Add blend at 30-50% opacity\n\
         - **Color Wash**: Gradient effect spanning all fixtures, slow offset animation\n\
         - **Beat Pulse**: Strobe at tempo BPM, 20-30% duty cycle, on top of ambient layer\n\
         \n\
         ## DSL Script Tips\n\
         - Start simple, test, then add complexity\n\
         - Use `hash(pixel, floor(t * speed))` for per-pixel randomness that changes with time\n\
         - Use `fract()` for repeating patterns\n\
         - Use `smoothstep(e0, e1, x)` for smooth transitions (e0 must be < e1)\n\
         - Combine `pos` with `t` for traveling patterns: `sin((pos - t) * TAU * 3.0)`\n",
    );
    out
}
