# VibeShow Brand Guide

## 1. Core Identity

**VibeShow is a precise creative instrument.**

It is not a toy, not a spectacle, and not a neon-heavy visualizer. It is a professional tool for designing light with control and clarity.

> The UI should never be more expressive than the show being created.

---

## 2. Brand Principles

### 2.1 Controlled, not chaotic
- Avoid visual noise, randomness, or excessive color
- Every element should have a clear purpose

### 2.2 Expressive, but restrained
- Creativity comes from composition and timing, not UI decoration
- Accent color is used sparingly and intentionally

### 2.3 Technical, but approachable
- Feels like a professional tool (DAW / CAD / Figma)
- Not intimidating or cluttered

### 2.4 AI-assisted, not AI-driven
- The user is always in control
- AI enhances workflow, never overrides intent

---

## 3. Visual Philosophy

### 3.1 Silence first, light second
- The UI is primarily neutral (grayscale)
- Color appears only where attention is required

### 3.2 Light is signal, not decoration
Use the primary color ONLY for:
- Active states
- Selection
- Playhead / timeline indicators
- Primary actions

Never use accent color for:
- Backgrounds
- Large surfaces
- Decorative gradients

---

## 4. Color System

### 4.1 Neutral grayscale (R = G = B)

#### Light Mode
| Token       | Value     | CSS Variable   | Tailwind Class        |
|-------------|-----------|----------------|-----------------------|
| Background  | `#FFFFFF` | `--bg`         | `bg-bg`               |
| Surface     | `#F7F7F7` | `--surface`    | `bg-surface`          |
| Surface-2   | `#FAFAFA` | `--surface-2`  | `bg-surface-2`        |
| Border      | `#E5E5E5` | `--border`     | `border-border`       |
| Text        | `#111111` | `--text`       | `text-text`           |
| Text-2      | `#555555` | `--text-2`     | `text-text-2`         |

#### Dark Mode
| Token       | Value     | CSS Variable   | Tailwind Class        |
|-------------|-----------|----------------|-----------------------|
| Background  | `#0E0E0E` | `--bg`         | `bg-bg`               |
| Surface     | `#161616` | `--surface`    | `bg-surface`          |
| Surface-2   | `#1D1D1D` | `--surface-2`  | `bg-surface-2`        |
| Border      | `#2A2A2A` | `--border`     | `border-border`       |
| Text        | `#F5F5F5` | `--text`       | `text-text`           |
| Text-2      | `#AAAAAA` | `--text-2`     | `text-text-2`         |

### 4.2 Accent color (single source of identity)

| Token         | Value     | CSS Variable      | Tailwind Class         |
|---------------|-----------|--------------------|------------------------|
| Primary       | `#3B82F6` | `--primary`        | `bg-primary`, `text-primary`, `border-primary`, `ring-primary` |
| Primary Hover | `#6A4DE0` (light) / `#9178FF` (dark) | `--primary-hover` | `bg-primary-hover` |

Rules:
- Do not introduce additional primary colors
- Do not use rainbow palettes in UI

### 4.3 Semantic colors

| Token   | Value     | CSS Variable | Tailwind Class |
|---------|-----------|--------------|----------------|
| Success | `#22C55E` | `--success`  | `text-success`, `bg-success` |
| Warning | `#F59E0B` | `--warning`  | `text-warning`, `bg-warning` |
| Error   | `#EF4444` | `--error`    | `text-error`, `bg-error`     |

### 4.4 Implementation (SSOT)

All tokens are defined as CSS custom properties in `src/index.css`:
- `:root { ... }` for light mode (default)
- `.dark { ... }` for dark mode
- `@theme inline { ... }` maps variables to Tailwind utilities

Dark mode is toggled via a `dark` class on `<html>`. The init script in `index.html` reads from `localStorage('theme')` with system-preference fallback (defaults to dark).

---

## 5. Typography

### Primary font
- System stack: `-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif`

### Rules
- No decorative text effects
- No glowing or gradient text
- Prioritize readability over style

---

## 6. Layout & Spacing

- Consistent border radius: `rounded` (4px) for small elements, `rounded-lg` (8px) for cards/panels
- Prefer alignment and spacing over visual separators

---

## 7. Components

### 7.1 Surfaces
- Use `bg-surface` and `bg-surface-2` instead of pure white/black
- Prefer subtle borders (`border-border`) over shadows

### 7.2 Borders
- Always 1px
- Use `border-border` (never pure black/white)

### 7.3 Buttons

#### Primary
```
bg-primary hover:bg-primary-hover text-white rounded px-3 py-1.5 text-xs font-medium
```

#### Secondary
```
bg-surface border border-border text-text-2 hover:bg-surface-2 hover:text-text rounded
```

#### Ghost
```
bg-transparent text-text-2 hover:bg-surface hover:text-text
```

### 7.4 Inputs
```
bg-surface border-border text-text placeholder:text-text-2 rounded outline-none focus:border-primary
```

### 7.5 Error banners
```
bg-error/10 border-error/20 text-error
```

---

## 8. Timeline (Core UI)

The timeline is the primary expression of the product.

### Rules
- Background: `bg-bg` (neutral)
- Clips: `border-border` default border
- Selected clip: `border-primary` with optional subtle glow (`box-shadow: 0 0 4px rgba(124, 92, 255, 0.25)`)
- Playhead: `bg-primary` (1-2px line)
- Grid lines: `bg-border/15` (low opacity)

### Avoid
- Neon clip colors
- Rainbow tracks
- Excessive glow effects

---

## 9. Motion & Interaction

- Fast and responsive (no lag)
- Subtle animations (`transition-colors duration-100`)
- No flashy transitions

State clarity is critical:
- Selected = obvious
- Active = obvious
- Playing = obvious

---

## 10. Dark Mode Philosophy

- Not a color inversion of light mode
- Designed independently but consistently

Guidelines:
- Avoid pure black (use `#0E0E0E`)
- Reduce contrast between surfaces
- Increase contrast for text

---

## 11. Anti-Patterns (Do Not Do)

- Neon gradients across large areas
- Multiple competing accent colors
- Glow effects on text or UI elements
- Overuse of shadows
- Decorative UI elements with no function
- Rainbow color systems
- Hardcoded colors in components (use tokens)

---

## 12. Product Feel

The product should feel like:
- Figma (clean, precise)
- Ableton (temporal, layered)
- Linear (focused, minimal)

Not like:
- DJ software
- Gaming UI
- Holiday lighting dashboards

---

## 13. One-Line Definition

> A modern, minimal light sequencing tool that gives powerful creative control without visual or cognitive noise.

---

## 14. Guiding Constraint

If unsure about any design decision:

> Remove visual complexity first. Add only what improves clarity or control.
