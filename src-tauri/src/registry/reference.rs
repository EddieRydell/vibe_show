/// Generate the complete DSL language reference from the actual implementation.
pub fn dsl_reference() -> String {
    r#"# VibeLights DSL Reference

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
param curve1: curve = 0,0 0.5,1 1,0
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
    smoothstep(width, 0.0, d)
}

-- Conditionals
if mode == Mode.Pulse {
    color1.scale(pulse(0.5, 0.3))
} else {
    grad(t)
}

-- Last expression is the output color
```

## Implicit Variables
| Variable | Type  | Description |
|----------|-------|-------------|
| `t`      | float | Normalized time [0.0, 1.0] within the effect duration |
| `pixel`  | int   | Current pixel index (0-based) |
| `pixels` | int   | Total pixel count in the effect's target |
| `pos`    | float | Normalized position: pixel / (pixels - 1), range [0.0, 1.0] |
| `pos2d`  | vec2  | 2D position (requires @spatial true) |
| `PI`     | float | 3.14159... |
| `TAU`    | float | 6.28318... (2π) |

## Types
- `float` — 64-bit floating point
- `int` — 32-bit integer
- `bool` — true/false
- `color` — RGBA color (r, g, b, a fields, 0-255)
- `vec2` — 2D vector (x, y fields)
- `gradient` — color gradient (callable: `grad(position)`)
- `curve` — timing curve (callable: `curve1(x)`)

## Parameter Types
- `float(min, max)` — float slider with range
- `int(min, max)` — integer slider with range
- `bool` — checkbox
- `color` — color picker (default: hex like `#ff0000`)
- `gradient` — gradient editor (default: comma-separated hex `#ff0000, #0000ff`)
- `curve` — curve editor (default: space-separated x,y pairs `0,0 0.5,1 1,0`)
- `EnumName` — dropdown from defined enum
- `FlagsName` — multi-select from defined flags

## Built-in Functions

### Math (1 argument → float)
| Function | Description |
|----------|-------------|
| `sin(x)` | Sine |
| `cos(x)` | Cosine |
| `tan(x)` | Tangent |
| `abs(x)` | Absolute value |
| `floor(x)` | Round down |
| `ceil(x)` | Round up |
| `round(x)` | Round to nearest |
| `fract(x)` | Fractional part (x - floor(x)) |
| `sqrt(x)` | Square root |

### Math (2 arguments → float)
| Function | Description |
|----------|-------------|
| `pow(base, exp)` | Power |
| `min(a, b)` | Minimum |
| `max(a, b)` | Maximum |
| `step(edge, x)` | 0 if x < edge, else 1 |
| `atan2(y, x)` | Arctangent of y/x |

### Math (3 arguments → float)
| Function | Description |
|----------|-------------|
| `clamp(x, min, max)` | Constrain x to [min, max] |
| `mix(a, b, t)` | Linear interpolation: a + (b - a) * t |
| `smoothstep(e0, e1, x)` | Smooth Hermite interpolation |

### Color Constructors → color
| Function | Description |
|----------|-------------|
| `rgb(r, g, b)` | RGB color (0-255 range) |
| `hsv(h, s, v)` | HSV color (h: 0-360, s: 0-1, v: 0-1) |
| `rgba(r, g, b, a)` | RGBA color (0-255 range) |

### Color Operations
| Operation | Description |
|-----------|-------------|
| `c.r`, `c.g`, `c.b`, `c.a` | Channel access (0-255) |
| `c.scale(f)` | Multiply RGB by float |
| Gradient call: `grad(0.5)` | Evaluate gradient at position |

### Vec2
| Function | Description |
|----------|-------------|
| `vec2(x, y)` | Construct vec2 |
| `distance(a, b)` | Euclidean distance between two vec2 |
| `length(v)` | Length of vec2 |
| `v.x`, `v.y` | Component access |

### Noise / Random
| Function | Description |
|----------|-------------|
| `hash(a, b)` | Deterministic pseudo-random [0, 1] — same inputs always produce same output |

## Enum & Flags Usage
```
enum Direction { Left, Right, Both }
param dir: Direction = Left

if dir == Direction.Left { ... }
```

```
flags Options { Sparkle, Glow, Pulse }
param opts: Options = Sparkle | Glow

if opts & Options.Sparkle { ... }
```

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
let brightness = smoothstep(width, 0.0, d2)
grad(pos).scale(brightness)
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
let falloff = smoothstep(0.8, 0.0, d)
color1.scale(wave * falloff)
```
"#
    .to_string()
}

/// Light show design best practices.
pub fn design_guide() -> String {
    r"# Light Show Design Guide

## Layering Strategy
- **Base layer**: Ambient fills (Solid, Gradient, or slow Rainbow) on the bottom track with low opacity
- **Accent layers**: Movement effects (Chase, Wipe, Fade) on higher tracks
- **Top layer**: Highlights (Strobe, Twinkle) with short durations or low opacity
- Use blend modes to combine: Add for glow effects, Multiply for darkening, Screen for brightening, Alpha for overlays

## Blend Mode Guide
| Mode | Use Case |
|------|----------|
| Override | Solo effect, replaces everything below |
| Add | Glow, energy buildup — adds light (never darkens) |
| Multiply | Shadow, dimming — darkens (never brightens) |
| Max | Peak detection — takes brightest channel |
| Alpha | Standard overlay — uses opacity for transparency |
| Screen | Soft brightening — lighter than Add |
| Subtract | Mask out colors from below |
| Min | Minimum of both — creates dark intersections |
| Average | Blend of both — good for smooth transitions |
| Mask | Uses top layer as brightness mask on bottom |
| IntensityOverlay | Uses top layer's brightness to modulate bottom |

## Beat Sync Techniques
- Use `get_beats_in_range` to get exact beat timestamps
- Place Strobe effects on beats for impact
- Use Fade effects spanning 2-4 beats for rhythmic breathing
- Chase effects with speed matching BPM: `speed = tempo / 60`
- For half-time feel, use `speed = tempo / 120`

## Section-Based Design
1. Call `get_sections` to understand the song structure
2. Design each section with a distinct visual identity:
   - **Intro**: Slow builds, dark-to-bright, single colors
   - **Verse**: Moderate movement, muted palette
   - **Chorus**: Full brightness, fast movement, rich gradients
   - **Bridge**: Contrasting colors/patterns from chorus
   - **Outro**: Mirror intro, bright-to-dark fade

## Color Theory for Lights
- **Complementary**: High contrast (red+cyan, blue+orange) — great for chorus
- **Analogous**: Harmonious (blue+purple+pink) — great for verse
- **Monochromatic**: Single hue, varied brightness — great for builds
- **Warm vs Cool**: Warm (red/orange/yellow) feels energetic; cool (blue/purple/cyan) feels calm
- Use the gradient library to define color palettes per section

## Common Patterns
- **Energy Build**: Increase brightness, speed, and color saturation from verse to chorus
- **Drop Impact**: Strobe + full-white flash at drop, then immediate Chase or Wipe
- **Breathing**: Fade effect with sine wave curve, 2-4 beat period
- **Sparkle Layer**: Twinkle on top track with Add blend at 30-50% opacity
- **Color Wash**: Gradient effect spanning all fixtures, slow offset animation
- **Beat Pulse**: Strobe at tempo BPM, 20-30% duty cycle, on top of ambient layer

## DSL Script Tips
- Start simple, test, then add complexity
- Use `hash(pixel, floor(t * speed))` for per-pixel randomness that changes with time
- Use `fract()` for repeating patterns
- Use `smoothstep()` instead of hard if/else for smooth transitions
- Combine `pos` with `t` for traveling patterns: `sin((pos - t) * TAU * 3.0)`
"
    .to_string()
}
